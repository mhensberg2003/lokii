//! Library: Watch Progress, Up Next, Continue watching and the Watchlist. All of it
//! stays in the local SQLite database. Output types mirror `src/lib/library.ts`.

pub mod progress;
pub mod up_next;
pub mod watchlist;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::catalog::anilist::AniListClient;
use crate::catalog::model::{ShowCard, ShowDetails};
use crate::catalog::{self, CatalogState};
use crate::store::{now, Store, StoreState};
use progress::WatchProgress;
use up_next::{up_next, UpNext};

/// Continue watching looks at this many of the most recently played Shows.
const CONTINUE_LIMIT: i64 = 20;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShowLibrary {
    pub progress: Vec<WatchProgress>,
    pub up_next: Option<UpNext>,
    pub on_watchlist: bool,
}

/// One card of the Continue watching row: the Up Next of a Show the user plays.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContinueItem {
    pub show: ShowCard,
    pub episode: i64,
    pub episode_title: Option<String>,
    pub thumbnail_url: Option<String>,
    /// Seconds to resume from; 0 starts the Episode.
    pub position: f64,
    pub duration: f64,
}

/// Saves the player's position. Returns true when the Episode is Watched.
#[tauri::command]
pub async fn library_save_progress(
    app: AppHandle,
    store: State<'_, StoreState>,
    show_id: i64,
    episode: i64,
    position: f64,
    duration: f64,
) -> Result<bool, String> {
    progress::save(store.get(&app).await?, show_id, episode, position, duration).await
}

/// The Watch Progress, Up Next and Watchlist state of one Show.
#[tauri::command]
pub async fn library_show(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
    show_id: i64,
) -> Result<ShowLibrary, String> {
    let store = store.get(&app).await?;
    let progress = progress::for_show(store, show_id).await?;
    let on_watchlist = watchlist::contains(store, show_id).await?;
    let up_next = if progress.is_empty() {
        None
    } else {
        let details = catalog::cached_show(catalog.client(), store, show_id).await?;
        up_next(&details, &progress, now())
    };
    Ok(ShowLibrary { progress, up_next, on_watchlist })
}

/// The Continue watching row, the most recently played Show first.
#[tauri::command]
pub async fn library_continue(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
) -> Result<Vec<ContinueItem>, String> {
    let store = store.get(&app).await?;
    let ids = progress::recent_shows(store, CONTINUE_LIMIT).await?;
    let items = futures::future::join_all(ids.into_iter().map(|id| continue_for(catalog.client(), store, id))).await;
    Ok(unique_shows(items.into_iter().flatten().collect()))
}

/// The Continue watching card of a Show, or None when it has no Up Next or cannot load.
async fn continue_for(client: &AniListClient, store: &Store, show_id: i64) -> Option<ContinueItem> {
    let details = catalog::cached_show(client, store, show_id).await.ok()?;
    let progress = progress::for_show(store, show_id).await.ok()?;
    let next = up_next(&details, &progress, now())?;
    if next.show_id == show_id {
        return Some(continue_item(&details, &next));
    }
    let sequel = catalog::cached_show(client, store, next.show_id).await.ok()?;
    Some(continue_item(&sequel, &next))
}

fn continue_item(show: &ShowDetails, next: &UpNext) -> ContinueItem {
    let info = show.episode_list.iter().find(|e| e.number == next.episode);
    ContinueItem {
        show: show.lite.card.clone(),
        episode: next.episode,
        episode_title: info.and_then(|e| e.title.clone()),
        thumbnail_url: info.and_then(|e| e.thumbnail_url.clone()),
        position: next.position,
        duration: next.duration,
    }
}

/// Keeps the first card of each Show. A finished Show and its sequel can both lead to
/// the sequel; the more recently played one wins.
fn unique_shows(items: Vec<ContinueItem>) -> Vec<ContinueItem> {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().filter(|item| seen.insert(item.show.id)).collect()
}

#[tauri::command]
pub async fn library_watchlist(app: AppHandle, store: State<'_, StoreState>) -> Result<Vec<ShowCard>, String> {
    watchlist::list(store.get(&app).await?).await
}

/// Adds the Show to the Watchlist (`on`) or removes it.
#[tauri::command]
pub async fn library_set_watchlist(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
    show_id: i64,
    on: bool,
) -> Result<(), String> {
    let store = store.get(&app).await?;
    if !on {
        return watchlist::remove(store, show_id).await;
    }
    let details = catalog::cached_show(catalog.client(), store, show_id).await?;
    watchlist::add(store, &details.lite.card).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use up_next::tests::show;

    #[test]
    fn a_continue_card_names_the_episode_and_keeps_the_position() {
        let next = UpNext { show_id: 1, episode: 2, position: 300.0, duration: 1440.0 };
        let item = continue_item(&show(1, 12, vec![]), &next);
        assert_eq!((item.show.id, item.episode, item.position), (1, 2, 300.0));
        assert_eq!(item.episode_title.as_deref(), Some("Title 2"));
    }

    #[test]
    fn each_show_is_in_the_row_once() {
        let card = |id: i64, episode: i64| {
            continue_item(&show(id, 12, vec![]), &UpNext { show_id: id, episode, position: 0.0, duration: 0.0 })
        };
        let items = unique_shows(vec![card(9, 1), card(2, 4), card(9, 3)]);
        assert_eq!(items.iter().map(|i| (i.show.id, i.episode)).collect::<Vec<_>>(), vec![(9, 1), (2, 4)]);
    }
}
