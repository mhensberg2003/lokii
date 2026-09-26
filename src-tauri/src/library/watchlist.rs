//! The Watchlist: Shows the user saved to watch later. Each row keeps a copy of the
//! Show's card, so the Library page shows the Watchlist without a request per Show.

use crate::catalog::model::ShowCard;
use crate::store::{now_ms, Store};

pub async fn add(store: &Store, card: &ShowCard) -> Result<(), String> {
    let json = serde_json::to_string(card).map_err(|e| e.to_string())?;
    let show_id = card.id;
    store
        .with_conn(move |conn| {
            conn.execute(
                "INSERT INTO watchlist (show_id, card, added_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT (show_id) DO UPDATE SET card = excluded.card",
                rusqlite::params![show_id, json, now_ms()],
            )?;
            Ok(())
        })
        .await
}

pub async fn remove(store: &Store, show_id: i64) -> Result<(), String> {
    store
        .with_conn(move |conn| {
            conn.execute("DELETE FROM watchlist WHERE show_id = ?1", [show_id])?;
            Ok(())
        })
        .await
}

pub async fn contains(store: &Store, show_id: i64) -> Result<bool, String> {
    store
        .with_conn(move |conn| {
            conn.query_row("SELECT EXISTS (SELECT 1 FROM watchlist WHERE show_id = ?1)", [show_id], |row| row.get(0))
        })
        .await
}

/// The Watchlist, the most recently added Show first. Rows that do not parse are skipped.
pub async fn list(store: &Store) -> Result<Vec<ShowCard>, String> {
    let rows: Vec<String> = store
        .with_conn(|conn| {
            let mut query = conn.prepare("SELECT card FROM watchlist ORDER BY added_at DESC")?;
            let rows = query.query_map([], |row| row.get(0))?;
            rows.collect()
        })
        .await?;
    Ok(rows.iter().filter_map(|json| serde_json::from_str(json).ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::up_next::tests::show;

    #[tokio::test]
    async fn shows_can_be_added_listed_and_removed() {
        let store = Store::in_memory().unwrap();
        let first = show(1, 12, vec![]).lite.card;
        let second = show(2, 12, vec![]).lite.card;
        add(&store, &first).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        add(&store, &second).await.unwrap();
        // Adding a Show again keeps one row.
        add(&store, &first).await.unwrap();
        assert_eq!(list(&store).await.unwrap(), vec![second.clone(), first.clone()]);
        assert!(contains(&store, 1).await.unwrap());

        remove(&store, 1).await.unwrap();
        assert!(!contains(&store, 1).await.unwrap());
        assert_eq!(list(&store).await.unwrap(), vec![second]);
    }
}
