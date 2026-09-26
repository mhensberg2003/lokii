//! Watch Progress: the last position the user reached in each Episode, saved by the
//! player every few seconds. An Episode is Watched once its Watch Progress passes 90%,
//! and it stays Watched when the user plays it again.

use rusqlite::Row;
use serde::Serialize;

use crate::store::{now_ms, Store};

/// An Episode is Watched after 90% of its length (CONTEXT.md).
pub const WATCHED_AT: f64 = 0.9;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WatchProgress {
    pub show_id: i64,
    pub episode: i64,
    /// Seconds.
    pub position: f64,
    /// Seconds.
    pub duration: f64,
    pub watched: bool,
    /// Unix milliseconds.
    pub updated_at: i64,
}

pub fn is_watched(position: f64, duration: f64) -> bool {
    duration > 0.0 && position / duration >= WATCHED_AT
}

/// Rejects values that mpv never reports for a playing file.
fn validate(position: f64, duration: f64) -> Result<(), String> {
    if !position.is_finite() || !duration.is_finite() || position < 0.0 || duration <= 0.0 {
        return Err(format!("Watch Progress {position} of {duration} is not valid."));
    }
    Ok(())
}

/// Saves the position and returns true when the Episode is Watched.
pub async fn save(store: &Store, show_id: i64, episode: i64, position: f64, duration: f64) -> Result<bool, String> {
    validate(position, duration)?;
    let position = position.min(duration);
    let watched = is_watched(position, duration);
    let updated_at = now_ms();
    store
        .with_conn(move |conn| {
            conn.execute(
                "INSERT INTO watch_progress (show_id, episode, position, duration, watched, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (show_id, episode) DO UPDATE SET
                    position = excluded.position,
                    duration = excluded.duration,
                    watched = watched OR excluded.watched,
                    updated_at = excluded.updated_at",
                rusqlite::params![show_id, episode, position, duration, watched, updated_at],
            )?;
            conn.query_row(
                "SELECT watched FROM watch_progress WHERE show_id = ?1 AND episode = ?2",
                [show_id, episode],
                |row| row.get(0),
            )
        })
        .await
}

fn from_row(row: &Row) -> rusqlite::Result<WatchProgress> {
    Ok(WatchProgress {
        show_id: row.get(0)?,
        episode: row.get(1)?,
        position: row.get(2)?,
        duration: row.get(3)?,
        watched: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

/// Every saved Episode of the Show, by Episode number.
pub async fn for_show(store: &Store, show_id: i64) -> Result<Vec<WatchProgress>, String> {
    store
        .with_conn(move |conn| {
            let mut query = conn.prepare(
                "SELECT show_id, episode, position, duration, watched, updated_at
                 FROM watch_progress WHERE show_id = ?1 ORDER BY episode",
            )?;
            let rows = query.query_map([show_id], from_row)?;
            rows.collect()
        })
        .await
}

/// The Shows with Watch Progress, the most recently played first.
pub async fn recent_shows(store: &Store, limit: i64) -> Result<Vec<i64>, String> {
    store
        .with_conn(move |conn| {
            let mut query = conn.prepare(
                "SELECT show_id FROM watch_progress
                 GROUP BY show_id ORDER BY MAX(updated_at) DESC LIMIT ?1",
            )?;
            let rows = query.query_map([limit], |row| row.get(0))?;
            rows.collect()
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_episode_is_watched_after_90_percent_and_stays_watched() {
        let store = Store::in_memory().unwrap();
        assert!(!save(&store, 1, 3, 100.0, 1440.0).await.unwrap());
        assert!(save(&store, 1, 3, 1300.0, 1440.0).await.unwrap());
        // Playing it again from the start keeps it Watched.
        assert!(save(&store, 1, 3, 20.0, 1440.0).await.unwrap());
        let saved = for_show(&store, 1).await.unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!((saved[0].position, saved[0].watched), (20.0, true));
    }

    #[tokio::test]
    async fn invalid_positions_are_rejected() {
        let store = Store::in_memory().unwrap();
        assert!(save(&store, 1, 1, f64::NAN, 1440.0).await.is_err());
        assert!(save(&store, 1, 1, 10.0, 0.0).await.is_err());
        assert!(save(&store, 1, 1, -1.0, 1440.0).await.is_err());
        assert!(for_show(&store, 1).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn recent_shows_puts_the_last_played_show_first() {
        let store = Store::in_memory().unwrap();
        save(&store, 1, 1, 60.0, 1440.0).await.unwrap();
        save(&store, 2, 1, 60.0, 1440.0).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        save(&store, 1, 2, 60.0, 1440.0).await.unwrap();
        assert_eq!(recent_shows(&store, 10).await.unwrap(), vec![1, 2]);
        assert_eq!(recent_shows(&store, 1).await.unwrap(), vec![1]);
    }
}
