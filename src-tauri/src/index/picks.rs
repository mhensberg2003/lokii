//! The user's Release pick per Show, stored in SQLite.

use rusqlite::OptionalExtension;

use crate::index::choose::Pick;
use crate::store::{now, Store};

pub async fn load(store: &Store, show_id: i64) -> Result<Option<Pick>, String> {
    store
        .with_conn(move |conn| {
            conn.query_row(
                "SELECT info_hash, release_group, resolution FROM release_pick WHERE show_id = ?1",
                [show_id],
                |row| Ok(Pick { info_hash: row.get(0)?, group: row.get(1)?, resolution: row.get(2)? }),
            )
            .optional()
        })
        .await
}

/// Stores the pick, or removes it when `pick` is `None`.
pub async fn save(store: &Store, show_id: i64, pick: Option<Pick>) -> Result<(), String> {
    store
        .with_conn(move |conn| {
            match pick {
                None => conn.execute("DELETE FROM release_pick WHERE show_id = ?1", [show_id])?,
                Some(pick) => conn.execute(
                    "INSERT INTO release_pick (show_id, info_hash, release_group, resolution, picked_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(show_id) DO UPDATE SET info_hash = excluded.info_hash,
                        release_group = excluded.release_group, resolution = excluded.resolution,
                        picked_at = excluded.picked_at",
                    rusqlite::params![show_id, pick.info_hash, pick.group, pick.resolution, now()],
                )?,
            };
            Ok(())
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_pick_round_trips_and_can_be_replaced_and_removed() {
        let store = Store::in_memory().unwrap();
        assert_eq!(load(&store, 1).await.unwrap(), None);

        let first = Pick { info_hash: "a".into(), group: Some("MTBB".into()), resolution: Some(1080) };
        save(&store, 1, Some(first.clone())).await.unwrap();
        assert_eq!(load(&store, 1).await.unwrap(), Some(first));

        let second = Pick { info_hash: "b".into(), group: None, resolution: None };
        save(&store, 1, Some(second.clone())).await.unwrap();
        assert_eq!(load(&store, 1).await.unwrap(), Some(second));

        save(&store, 1, None).await.unwrap();
        assert_eq!(load(&store, 1).await.unwrap(), None);
    }
}
