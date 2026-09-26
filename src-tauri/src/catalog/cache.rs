//! SQLite-backed response cache for the AniList client. Every GraphQL response is
//! stored under a key derived from its query text and variables, with a per-call TTL.
//! A network failure falls back to a stale cached copy when one exists.
//!
//! Blocking `rusqlite` calls run through `tokio::task::spawn_blocking`, holding the
//! connection lock only for the duration of one query.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};
use tokio::sync::Mutex;

/// The schema version this build knows how to migrate to. Later milestones add more
/// `if version < N` steps in `migrate` and bump this constant.
const SCHEMA_VERSION: i32 = 1;

pub struct Cache {
    conn: Arc<Mutex<Connection>>,
}

impl Cache {
    /// Opens (creating if needed) the SQLite file at `path` and runs migrations.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, String> {
        let path = path.as_ref().to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection, String> {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
            }
            let conn = Connection::open(&path).map_err(|e| format!("cannot open cache database: {e}"))?;
            migrate(&conn)?;
            Ok(conn)
        })
        .await
        .map_err(|e| format!("cache init task panicked: {e}"))??;
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
            conn.query_row("SELECT body, fetched_at FROM anilist_cache WHERE key = ?1", [&key], |row| {
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
                "INSERT INTO anilist_cache (key, body, fetched_at) VALUES (?1, ?2, ?3)
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

    async fn with_conn<T, F>(&self, f: F) -> Result<T, String>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let guard = self.conn.clone().lock_owned().await;
        tokio::task::spawn_blocking(move || f(&guard).map_err(|e| e.to_string()))
            .await
            .map_err(|e| format!("cache task panicked: {e}"))?
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
    // Future milestones: `if version < 2 { ... }`, etc.
    conn.pragma_update(None, "user_version", SCHEMA_VERSION).map_err(|e| e.to_string())?;
    Ok(())
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn is_fresh(fetched_at: i64, ttl: Duration) -> bool {
    now().saturating_sub(fetched_at) < ttl.as_secs() as i64
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
    async fn fresh_entry_is_returned_without_calling_fetch() {
        let cache = Cache::in_memory().unwrap();
        cache.put_at("k", "cached", now()).await.unwrap();

        let result = cache
            .get_or_fetch("k", Duration::from_secs(3600), || async { panic!("fetch should not run") })
            .await
            .unwrap();
        assert_eq!(result, "cached");
    }

    #[tokio::test]
    async fn expired_entry_triggers_a_fetch_and_is_replaced() {
        let cache = Cache::in_memory().unwrap();
        cache.put_at("k", "old", now() - 1000).await.unwrap();

        let result = cache
            .get_or_fetch("k", Duration::from_secs(10), || async { Ok("fresh".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "fresh");

        // The replacement was persisted.
        let again = cache.get_or_fetch("k", Duration::from_secs(10), || async { panic!("should be cached") }).await.unwrap();
        assert_eq!(again, "fresh");
    }

    #[tokio::test]
    async fn network_error_falls_back_to_stale_cache() {
        let cache = Cache::in_memory().unwrap();
        cache.put_at("k", "stale", now() - 1_000_000).await.unwrap();

        let result = cache
            .get_or_fetch("k", Duration::from_secs(10), || async { Err("network down".to_string()) })
            .await
            .unwrap();
        assert_eq!(result, "stale");
    }

    #[tokio::test]
    async fn network_error_with_no_cache_propagates_the_error() {
        let cache = Cache::in_memory().unwrap();
        let result =
            cache.get_or_fetch("missing", Duration::from_secs(10), || async { Err("network down".to_string()) }).await;
        assert_eq!(result, Err("network down".to_string()));
    }

    #[tokio::test]
    async fn a_successful_fetch_with_no_prior_cache_is_stored() {
        let cache = Cache::in_memory().unwrap();
        let result =
            cache.get_or_fetch("k", Duration::from_secs(10), || async { Ok("first".to_string()) }).await.unwrap();
        assert_eq!(result, "first");

        let again = cache.get_or_fetch("k", Duration::from_secs(10), || async { panic!("should be cached") }).await.unwrap();
        assert_eq!(again, "first");
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
