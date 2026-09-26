//! The app's one SQLite database (`<app data dir>/lokii.db`): an HTTP response cache
//! shared by every online service, the ID mapping, and user state such as Release picks.
//! A cached response has a per-call TTL; a network failure falls back to a stale copy
//! when one exists.
//!
//! Blocking `rusqlite` calls run through `tokio::task::spawn_blocking`, holding the
//! connection lock only for the duration of one call.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OnceCell};

/// The schema version this build knows how to migrate to. Later milestones add more
/// `if version < N` steps in `migrate` and bump this constant.
const SCHEMA_VERSION: i32 = 3;

pub struct Store {
    conn: Arc<Mutex<Connection>>,
}

impl Store {
    /// Opens (creating if needed) the SQLite file at `path` and runs migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection, String> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            let conn = Connection::open(&path).map_err(|e| format!("cannot open the database: {e}"))?;
            migrate(&conn)?;
            Ok(conn)
        })
        .await
        .map_err(|e| format!("database init task panicked: {e}"))??;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// An in-memory database, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        migrate(&conn)?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    /// Returns the cached body for `key` if younger than `ttl`. Otherwise calls
    /// `fetch`, caches a successful result, and returns it. If `fetch` fails and a
    /// cached body exists (even a stale one), that stale body is returned instead.
    pub async fn get_or_fetch<F, Fut>(&self, key: &str, ttl: Duration, fetch: F) -> Result<String, String>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<String, String>>,
    {
        let existing = self.read(key).await?;
        if let Some((body, fetched_at)) = &existing {
            if is_fresh(*fetched_at, ttl) {
                return Ok(body.clone());
            }
        }

        match fetch().await {
            Ok(body) => {
                self.write(key, &body, now()).await?;
                Ok(body)
            }
            Err(err) => existing.map(|(body, _)| body).ok_or(err),
        }
    }

    async fn read(&self, key: &str) -> Result<Option<(String, i64)>, String> {
        let key = key.to_string();
        self.with_conn(move |conn| {
            conn.query_row("SELECT body, fetched_at FROM http_cache WHERE key = ?1", [&key], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .optional()
        })
        .await
    }

    async fn write(&self, key: &str, body: &str, fetched_at: i64) -> Result<(), String> {
        let key = key.to_string();
        let body = body.to_string();
        self.with_conn(move |conn| {
            conn.execute(
                "INSERT INTO http_cache (key, body, fetched_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET body = excluded.body, fetched_at = excluded.fetched_at",
                rusqlite::params![key, body, fetched_at],
            )?;
            Ok(())
        })
        .await
    }

    /// Seeds a row with an explicit `fetched_at`, so tests can create stale entries.
    #[cfg(test)]
    pub async fn put_at(&self, key: &str, body: &str, fetched_at: i64) -> Result<(), String> {
        self.write(key, body, fetched_at).await
    }

    /// A value from the `meta` table.
    pub async fn meta(&self, key: &str) -> Result<Option<String>, String> {
        let key = key.to_string();
        self.with_conn(move |conn| {
            conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get(0)).optional()
        })
        .await
    }

    /// Stores a value in the `meta` table, or removes it when `value` is `None`.
    pub async fn set_meta(&self, key: &str, value: Option<String>) -> Result<(), String> {
        let key = key.to_string();
        self.with_conn(move |conn| {
            match value {
                Some(value) => conn.execute(
                    "INSERT INTO meta (key, value) VALUES (?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [key, value],
                )?,
                None => conn.execute("DELETE FROM meta WHERE key = ?1", [key])?,
            };
            Ok(())
        })
        .await
    }

    /// Runs `f` with the connection on a blocking thread.
    pub async fn with_conn<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let guard = self.conn.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || f(&guard).map_err(|e| e.to_string()))
            .await
            .map_err(|e| format!("database task panicked: {e}"))?
    }
}

fn migrate(conn: &Connection) -> Result<(), String> {
    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS anilist_cache (
                key TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                fetched_at INTEGER NOT NULL
            );",
        )
        .map_err(|e| e.to_string())?;
    }
    if version < 2 {
        conn.execute_batch(
            "ALTER TABLE anilist_cache RENAME TO http_cache;
             CREATE TABLE IF NOT EXISTS meta (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS anime_ids (
                anilist_id INTEGER PRIMARY KEY,
                anidb_id INTEGER,
                mal_id INTEGER
             );
             CREATE TABLE IF NOT EXISTS release_pick (
                show_id INTEGER PRIMARY KEY,
                info_hash TEXT NOT NULL,
                release_group TEXT,
                resolution INTEGER,
                picked_at INTEGER NOT NULL
             );",
        )
        .map_err(|e| e.to_string())?;
    }
    if version < 3 {
        conn.execute_batch(
            "DROP TABLE IF EXISTS torbox_item;
             CREATE TABLE torbox_item (
                info_hash TEXT NOT NULL,
                show_id INTEGER NOT NULL,
                torrent_id INTEGER NOT NULL,
                first_episode INTEGER NOT NULL,
                last_episode INTEGER NOT NULL,
                watched TEXT NOT NULL DEFAULT '[]',
                added_by_lokii INTEGER NOT NULL,
                added_at INTEGER NOT NULL,
                PRIMARY KEY (info_hash, show_id)
             );",
        )
        .map_err(|e| e.to_string())?;
    }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn is_fresh(fetched_at: i64, ttl: Duration) -> bool {
    now().saturating_sub(fetched_at) < ttl.as_secs() as i64
}

/// Opens the database lazily, on first use, and shares it between every command.
#[derive(Default)]
pub struct StoreState {
    store: OnceCell<Store>,
}

impl StoreState {
    pub async fn get(&self, app: &AppHandle) -> Result<&Store, String> {
        self.store
            .get_or_try_init(|| async {
                let dir = app.path().app_data_dir().map_err(|e| format!("cannot find the app data folder: {e}"))?;
                Store::open(dir.join("lokii.db")).await
            })
            .await
    }
}

/// A cache key derived from a GraphQL query's text and variables, tagged with a
/// human-readable namespace (e.g. `"home"`, `"show"`) for easier debugging.
pub fn cache_key(namespace: &str, query: &str, variables: &serde_json::Value) -> String {
    let mut hasher = DefaultHasher::new();
    query.hash(&mut hasher);
    variables.to_string().hash(&mut hasher);
    format!("{namespace}:{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn meta_values_can_be_set_replaced_and_removed() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.meta("k").await.unwrap(), None);
        store.set_meta("k", Some("1".into())).await.unwrap();
        store.set_meta("k", Some("2".into())).await.unwrap();
        assert_eq!(store.meta("k").await.unwrap(), Some("2".into()));
        store.set_meta("k", None).await.unwrap();
        assert_eq!(store.meta("k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn fresh_entry_is_returned_without_calling_fetch() {
        let cache = Store::in_memory().unwrap();
        cache.put_at("k", "cached", now()).await.unwrap();

        let result = cache
            .get_or_fetch("k", Duration::from_secs(3600), || async { panic!("fetch should not run") })
            .await
            .unwrap();
        assert_eq!(result, "cached");
    }

    #[tokio::test]
    async fn expired_entry_triggers_a_fetch_and_is_replaced() {
        let cache = Store::in_memory().unwrap();
        cache.put_at("k", "old", now() - 1000).await.unwrap();

        let result =
            cache.get_or_fetch("k", Duration::from_secs(10), || async { Ok("fresh".to_string()) }).await.unwrap();
        assert_eq!(result, "fresh");

        // The replacement was persisted.
        let again =
            cache.get_or_fetch("k", Duration::from_secs(10), || async { panic!("should be cached") }).await.unwrap();
        assert_eq!(again, "fresh");
    }

    #[tokio::test]
    async fn network_error_falls_back_to_stale_cache() {
        let cache = Store::in_memory().unwrap();
        cache.put_at("k", "stale", now() - 1_000_000).await.unwrap();

        let result = cache
            .get_or_fetch("k", Duration::from_secs(10), || async { Err("network down".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "stale");
    }

    #[tokio::test]
    async fn network_error_with_no_cache_propagates_the_error() {
        let cache = Store::in_memory().unwrap();
        let result =
            cache.get_or_fetch("missing", Duration::from_secs(10), || async { Err("network down".to_string()) }).await;
        assert_eq!(result, Err("network down".to_string()));
    }

    #[tokio::test]
    async fn a_successful_fetch_with_no_prior_cache_is_stored() {
        let cache = Store::in_memory().unwrap();
        let result =
            cache.get_or_fetch("k", Duration::from_secs(10), || async { Ok("first".to_string()) }).await.unwrap();
        assert_eq!(result, "first");

        let again =
            cache.get_or_fetch("k", Duration::from_secs(10), || async { panic!("should be cached") }).await.unwrap();
        assert_eq!(again, "first");
    }

    #[test]
    fn version_1_databases_keep_their_cache_after_migrating() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE anilist_cache (key TEXT PRIMARY KEY, body TEXT NOT NULL, fetched_at INTEGER NOT NULL);
             INSERT INTO anilist_cache VALUES ('k', 'kept', 1);
             PRAGMA user_version = 1;",
        )
        .unwrap();
        migrate(&conn).unwrap();
        let body: String = conn.query_row("SELECT body FROM http_cache WHERE key = 'k'", [], |row| row.get(0)).unwrap();
        assert_eq!(body, "kept");
        for table in ["meta", "anime_ids", "release_pick"] {
            let count: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0)).unwrap();
            assert_eq!(count, 0, "{table} should exist and be empty");
        }
        let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn cache_key_is_stable_and_distinguishes_variables() {
        let a = cache_key("show", "query {}", &serde_json::json!({"id": 1}));
        let b = cache_key("show", "query {}", &serde_json::json!({"id": 1}));
        let c = cache_key("show", "query {}", &serde_json::json!({"id": 2}));
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
