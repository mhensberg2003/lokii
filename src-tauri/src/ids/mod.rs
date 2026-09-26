//! AniList ↔ AniDB ↔ MAL ID mapping from the Fribb anime-lists project. AnimeTosho
//! needs AniDB IDs; AniSkip needs MAL IDs. The app ships a snapshot (`anime-ids.tsv`,
//! rebuilt by `scripts/update-anime-ids.mjs`), seeds SQLite from it on first use, and
//! replaces it with fresh data once a week.

use std::collections::HashSet;
use std::time::Duration;

use rusqlite::OptionalExtension;
use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::store::{now, Store, StoreState};

const BUNDLED: &str = include_str!("anime-ids.tsv");
const SOURCE_URL: &str = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-mini.json";
const REFRESH_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);
const REFRESHED_AT_KEY: &str = "anime_ids_refreshed_at";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimeIds {
    pub anidb: Option<i64>,
    pub mal: Option<i64>,
}

/// One mapping row: AniList ID, AniDB ID, MAL ID.
type Row = (i64, Option<i64>, Option<i64>);

/// The IDs for one AniList Show, or `None` when the mapping does not know it.
pub async fn lookup(store: &Store, anilist_id: i64) -> Result<Option<AnimeIds>, String> {
    ensure_seeded(store).await?;
    store
        .with_conn(move |conn| {
            conn.query_row("SELECT anidb_id, mal_id FROM anime_ids WHERE anilist_id = ?1", [anilist_id], |row| {
                Ok(AnimeIds { anidb: row.get(0)?, mal: row.get(1)? })
            })
            .optional()
        })
        .await
}

/// Refreshes the mapping in the background when it is older than a week. A failure
/// keeps the current data; the next app start tries again.
pub fn spawn_refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<StoreState>();
        let Ok(store) = state.get(&app).await else {
            return;
        };
        let http = crate::index::http_client();
        let _ = refresh_if_stale(store, &http).await;
    });
}

async fn refresh_if_stale(store: &Store, http: &reqwest::Client) -> Result<(), String> {
    ensure_seeded(store).await?;
    let refreshed_at = read_refreshed_at(store).await?;
    if now().saturating_sub(refreshed_at) < REFRESH_AFTER.as_secs() as i64 {
        return Ok(());
    }
    let response = http.get(SOURCE_URL).send().await.map_err(|e| format!("cannot download the ID mapping: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("the ID mapping download returned HTTP {}", response.status()));
    }
    let body = response.text().await.map_err(|e| format!("cannot read the ID mapping: {e}"))?;
    let rows = parse_fribb(&body)?;
    replace_all(store, rows, now()).await
}

/// Seeds the table from the bundled snapshot when it is empty. The snapshot counts as
/// stale, so the first app start downloads fresh data.
async fn ensure_seeded(store: &Store) -> Result<(), String> {
    let count: i64 =
        store.with_conn(|conn| conn.query_row("SELECT COUNT(*) FROM anime_ids", [], |row| row.get(0))).await?;
    if count > 0 {
        return Ok(());
    }
    replace_all(store, parse_tsv(BUNDLED), 0).await
}

async fn read_refreshed_at(store: &Store) -> Result<i64, String> {
    let value: Option<String> = store
        .with_conn(|conn| {
            conn.query_row("SELECT value FROM meta WHERE key = ?1", [REFRESHED_AT_KEY], |row| row.get(0)).optional()
        })
        .await?;
    Ok(value.and_then(|v| v.parse().ok()).unwrap_or(0))
}

async fn replace_all(store: &Store, rows: Vec<Row>, refreshed_at: i64) -> Result<(), String> {
    if rows.is_empty() {
        return Err("the ID mapping has no rows".to_string());
    }
    store
        .with_conn(move |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute("DELETE FROM anime_ids", [])?;
            {
                let mut insert =
                    tx.prepare("INSERT INTO anime_ids (anilist_id, anidb_id, mal_id) VALUES (?1, ?2, ?3)")?;
                for (anilist, anidb, mal) in rows {
                    insert.execute(rusqlite::params![anilist, anidb, mal])?;
                }
            }
            tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![REFRESHED_AT_KEY, refreshed_at.to_string()],
            )?;
            tx.commit()
        })
        .await
}

/// Parses the bundled `anilist\tanidb\tmal` snapshot; skips the header and bad lines.
fn parse_tsv(text: &str) -> Vec<Row> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let mut fields = line.split('\t');
            let anilist = fields.next()?.parse().ok()?;
            let anidb = fields.next().and_then(|v| v.parse().ok());
            let mal = fields.next().and_then(|v| v.parse().ok());
            Some((anilist, anidb, mal))
        })
        .collect()
}

#[derive(Deserialize)]
struct FribbEntry {
    anilist_id: Option<i64>,
    anidb_id: Option<i64>,
    mal_id: Option<i64>,
}

/// Parses the Fribb `anime-list-mini.json`. Keeps the first row per AniList ID and
/// drops rows that map to nothing.
fn parse_fribb(json: &str) -> Result<Vec<Row>, String> {
    let entries: Vec<FribbEntry> =
        serde_json::from_str(json).map_err(|e| format!("the ID mapping has an unexpected shape: {e}"))?;
    let mut seen = HashSet::new();
    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let anilist = entry.anilist_id?;
            if entry.anidb_id.is_none() && entry.mal_id.is_none() {
                return None;
            }
            seen.insert(anilist).then_some((anilist, entry.anidb_id, entry.mal_id))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_snapshot_parses_and_knows_attack_on_titan() {
        let rows = parse_tsv(BUNDLED);
        assert!(rows.len() > 10_000, "only {} rows", rows.len());
        assert!(rows.contains(&(16498, Some(9541), Some(16498))));
    }

    #[test]
    fn tsv_rows_may_have_empty_ids() {
        assert_eq!(parse_tsv("header\n1\t\t5\n2\t7\t\nbad\n"), vec![(1, None, Some(5)), (2, Some(7), None)]);
    }

    #[test]
    fn fribb_parsing_keeps_the_first_row_and_drops_empty_ones() {
        let json = r#"[
            {"anilist_id": 1, "anidb_id": 10, "mal_id": 100, "type": "TV"},
            {"anilist_id": 1, "anidb_id": 11},
            {"anilist_id": 2},
            {"anidb_id": 30, "mal_id": 300}
        ]"#;
        assert_eq!(parse_fribb(json).unwrap(), vec![(1, Some(10), Some(100))]);
    }

    #[tokio::test]
    async fn lookup_seeds_from_the_snapshot_on_first_use() {
        let store = Store::in_memory().unwrap();
        let ids = lookup(&store, 16498).await.unwrap();
        assert_eq!(ids, Some(AnimeIds { anidb: Some(9541), mal: Some(16498) }));
        assert_eq!(lookup(&store, -1).await.unwrap(), None);
        assert_eq!(read_refreshed_at(&store).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn replace_all_swaps_the_rows_and_records_the_time() {
        let store = Store::in_memory().unwrap();
        replace_all(&store, vec![(5, Some(50), None)], 1234).await.unwrap();
        assert_eq!(lookup(&store, 5).await.unwrap(), Some(AnimeIds { anidb: Some(50), mal: None }));
        assert_eq!(lookup(&store, 16498).await.unwrap(), None);
        assert_eq!(read_refreshed_at(&store).await.unwrap(), 1234);
    }

    #[tokio::test]
    async fn an_empty_download_does_not_wipe_the_mapping() {
        let store = Store::in_memory().unwrap();
        ensure_seeded(&store).await.unwrap();
        assert!(replace_all(&store, Vec::new(), now()).await.is_err());
        assert!(lookup(&store, 16498).await.unwrap().is_some());
    }
}
