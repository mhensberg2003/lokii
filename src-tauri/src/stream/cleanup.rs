//! Delete-after-Watched rules for TorBox. Lokii remembers each TorBox torrent it plays,
//! per Show, and which of its Episodes are Watched. It removes a torrent it added when
//! every Episode in it is Watched for every Show that plays it, so a Batch stays until
//! its last Episode. It never removes a torrent that was already in the user's list.

use crate::index::model::Coverage;
use crate::store::{now, Store};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorBoxItem {
    pub info_hash: String,
    /// 0 while Lokii adds the torrent, so an interrupted add still counts as Lokii's.
    pub torrent_id: i64,
    pub show_id: i64,
    pub first_episode: i64,
    pub last_episode: i64,
    pub watched: Vec<i64>,
    pub added_by_lokii: bool,
}

impl TorBoxItem {
    pub fn new(info_hash: &str, torrent_id: i64, show_id: i64, episodes: (i64, i64), added_by_lokii: bool) -> Self {
        Self {
            info_hash: info_hash.to_string(),
            torrent_id,
            show_id,
            first_episode: episodes.0,
            last_episode: episodes.1,
            watched: Vec::new(),
            added_by_lokii,
        }
    }

    /// True when Lokii added the torrent and every Episode in it is Watched.
    pub fn can_remove(&self) -> bool {
        self.added_by_lokii && (self.first_episode..=self.last_episode).all(|e| self.watched.contains(&e))
    }
}

/// True when Lokii can remove the torrent: every Show that plays it has Watched all
/// of its Episodes.
pub fn can_remove_torrent(items: &[TorBoxItem]) -> bool {
    !items.is_empty() && items.iter().all(TorBoxItem::can_remove)
}

/// The Episodes a Release holds, as a closed range. `show_episodes` is the Show's
/// Episode count, for a Batch of the full Show.
pub fn episode_range(coverage: Coverage, show_episodes: i64) -> (i64, i64) {
    match coverage {
        Coverage::Episode { episode } => (episode, episode),
        Coverage::Range { first, last } => (first, last),
        Coverage::Show => (1, show_episodes.max(1)),
    }
}

/// Every Show's row for the torrent.
pub async fn find_all(store: &Store, info_hash: &str) -> Result<Vec<TorBoxItem>, String> {
    let hash = info_hash.to_string();
    store
        .with_conn(move |conn| {
            let mut query = conn.prepare(
                "SELECT info_hash, torrent_id, show_id, first_episode, last_episode, watched, added_by_lokii
                 FROM torbox_item WHERE info_hash = ?1",
            )?;
            let rows = query.query_map([hash], |row| {
                let watched: String = row.get(5)?;
                Ok(TorBoxItem {
                    info_hash: row.get(0)?,
                    torrent_id: row.get(1)?,
                    show_id: row.get(2)?,
                    first_episode: row.get(3)?,
                    last_episode: row.get(4)?,
                    watched: serde_json::from_str(&watched).unwrap_or_default(),
                    added_by_lokii: row.get(6)?,
                })
            })?;
            rows.collect()
        })
        .await
}

pub async fn find(store: &Store, info_hash: &str, show_id: i64) -> Result<Option<TorBoxItem>, String> {
    Ok(find_all(store, info_hash).await?.into_iter().find(|item| item.show_id == show_id))
}

pub async fn save(store: &Store, item: TorBoxItem) -> Result<(), String> {
    let watched = serde_json::to_string(&item.watched).unwrap_or_else(|_| "[]".into());
    store
        .with_conn(move |conn| {
            conn.execute(
                "INSERT INTO torbox_item
                   (info_hash, torrent_id, show_id, first_episode, last_episode, watched, added_by_lokii, added_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(info_hash, show_id) DO UPDATE SET torrent_id = excluded.torrent_id,
                   watched = excluded.watched, first_episode = excluded.first_episode,
                   last_episode = excluded.last_episode, added_by_lokii = excluded.added_by_lokii",
                rusqlite::params![
                    item.info_hash,
                    item.torrent_id,
                    item.show_id,
                    item.first_episode,
                    item.last_episode,
                    watched,
                    item.added_by_lokii,
                    now()
                ],
            )?;
            Ok(())
        })
        .await
}

/// Removes every Show's row for the torrent.
pub async fn forget(store: &Store, info_hash: &str) -> Result<(), String> {
    let hash = info_hash.to_string();
    store.with_conn(move |conn| conn.execute("DELETE FROM torbox_item WHERE info_hash = ?1", [hash]).map(|_| ())).await
}

/// Records the Episode of the Show as Watched and returns the updated item.
pub async fn mark_watched(
    store: &Store,
    info_hash: &str,
    show_id: i64,
    episode: i64,
) -> Result<Option<TorBoxItem>, String> {
    let Some(mut item) = find(store, info_hash, show_id).await? else {
        return Ok(None);
    };
    if !item.watched.contains(&episode) {
        item.watched.push(episode);
        item.watched.sort_unstable();
        save(store, item.clone()).await?;
    }
    Ok(Some(item))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "fcd7dbc76cfeca08d3888ccc446e8859fc7c90db";

    #[test]
    fn a_batch_stays_until_its_last_episode_is_watched() {
        let mut item = TorBoxItem::new(HASH, 1, 16498, (1, 3), true);
        item.watched = vec![1, 3];
        assert!(!item.can_remove());
        item.watched.push(2);
        assert!(item.can_remove());
    }

    #[test]
    fn a_torrent_the_user_added_is_never_removed() {
        let mut item = TorBoxItem::new(HASH, 1, 16498, (5, 5), false);
        item.watched = vec![5];
        assert!(!item.can_remove());
    }

    #[test]
    fn a_full_show_batch_covers_every_episode() {
        assert_eq!(episode_range(Coverage::Show, 25), (1, 25));
        assert_eq!(episode_range(Coverage::Episode { episode: 7 }, 25), (7, 7));
        assert_eq!(episode_range(Coverage::Range { first: 13, last: 24 }, 25), (13, 24));
    }

    #[tokio::test]
    async fn watched_episodes_persist_once_each() {
        let store = Store::in_memory().unwrap();
        assert_eq!(mark_watched(&store, HASH, 16498, 1).await.unwrap(), None);

        save(&store, TorBoxItem::new(HASH, 42, 16498, (1, 2), true)).await.unwrap();
        mark_watched(&store, HASH, 16498, 2).await.unwrap();
        let item = mark_watched(&store, HASH, 16498, 2).await.unwrap().unwrap();
        assert_eq!(item.watched, vec![2]);
        assert_eq!(find(&store, HASH, 16498).await.unwrap(), Some(item));

        forget(&store, HASH).await.unwrap();
        assert_eq!(find(&store, HASH, 16498).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_release_of_two_shows_stays_until_both_are_watched() {
        let store = Store::in_memory().unwrap();
        save(&store, TorBoxItem::new(HASH, 42, 1, (1, 1), true)).await.unwrap();
        save(&store, TorBoxItem::new(HASH, 42, 2, (1, 1), true)).await.unwrap();
        mark_watched(&store, HASH, 2, 1).await.unwrap();
        assert!(!can_remove_torrent(&find_all(&store, HASH).await.unwrap()));
        mark_watched(&store, HASH, 1, 1).await.unwrap();
        assert!(can_remove_torrent(&find_all(&store, HASH).await.unwrap()));
    }
}
