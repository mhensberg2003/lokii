//! One Pace: a fan recut of the One Piece anime. It is not on AniList, so Lokii builds
//! it here: each Arc is one Show, and all Arcs form one Franchise. The Releases come
//! from Nyaa, matched to Episodes by the CRC32 in the Episode Guide sheet.
//!
//! The app ships a snapshot (`onepace.json`, rebuilt by `cargo test
//! dump_bundled_mapping -- --ignored`) and rebuilds it once a week in the background.

mod build;
pub mod model;
mod sources;

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;

use crate::catalog::model::{ShowCard, ShowDetails};
use crate::catalog::CatalogState;
use crate::index::model::Release;
use crate::store::{now, Store, StoreState};
use model::Mapping;

pub use model::{is_arc, names_one_pace};

const BUNDLED: &str = include_str!("onepace.json");
const MAPPING_KEY: &str = "onepace_mapping";
const REFRESH_AFTER: Duration = Duration::from_secs(7 * 24 * 3600);
/// A rebuild that loses more Episodes than this share is a broken source, not news.
const MIN_KEPT_SHARE: f64 = 0.9;

/// The current mapping, loaded once from SQLite or the bundled snapshot.
#[derive(Default)]
pub struct OnePace {
    current: RwLock<Option<Arc<Mapping>>>,
}

impl OnePace {
    pub async fn mapping(&self, store: &Store) -> Result<Arc<Mapping>, String> {
        if let Some(mapping) = self.current.read().await.as_ref() {
            return Ok(mapping.clone());
        }
        let mut current = self.current.write().await;
        if let Some(mapping) = current.as_ref() {
            return Ok(mapping.clone());
        }
        let saved = store.meta(MAPPING_KEY).await?.and_then(|json| serde_json::from_str::<Mapping>(&json).ok());
        let bundled = bundled()?;
        // A new app version can ship a newer snapshot than the last refresh.
        let mapping = match saved {
            Some(saved) if saved.built_at >= bundled.built_at => saved,
            _ => bundled,
        };
        let mapping = Arc::new(mapping);
        *current = Some(mapping.clone());
        Ok(mapping)
    }

    async fn replace(&self, store: &Store, mapping: Mapping) -> Result<(), String> {
        let json = serde_json::to_string(&mapping).map_err(|e| e.to_string())?;
        store.set_meta(MAPPING_KEY, Some(json)).await?;
        *self.current.write().await = Some(Arc::new(mapping));
        Ok(())
    }

    pub async fn show(&self, store: &Store, id: i64) -> Result<ShowDetails, String> {
        self.mapping(store).await?.show(id).ok_or_else(|| "This One Pace Arc does not exist.".to_string())
    }

    pub async fn releases(&self, store: &Store, id: i64, episode: i64) -> Result<Vec<Release>, String> {
        Ok(self.mapping(store).await?.releases(id, episode))
    }

    pub async fn search(&self, store: &Store, query: &str) -> Result<Vec<ShowCard>, String> {
        Ok(self.mapping(store).await?.search(query))
    }
}

fn bundled() -> Result<Mapping, String> {
    serde_json::from_str(BUNDLED).map_err(|e| format!("the bundled One Pace mapping is broken: {e}"))
}

/// Rebuilds the mapping in the background when it is older than a week. A failure
/// keeps the current mapping; the next app start tries again.
pub fn spawn_refresh(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<StoreState>();
        let Ok(store) = state.get(&app).await else { return };
        let onepace = &app.state::<CatalogState>().onepace;
        let Ok(current) = onepace.mapping(store).await else { return };
        if now().saturating_sub(current.built_at) < REFRESH_AFTER.as_secs() as i64 {
            return;
        }
        let http = crate::index::http_client();
        if let Ok(fresh) = rebuild(&http, &current, now()).await {
            let _ = onepace.replace(store, fresh).await;
        }
    });
}

async fn rebuild(http: &reqwest::Client, current: &Mapping, built_at: i64) -> Result<Mapping, String> {
    let (guide_tabs, descriptions, posters, stills, releases) = tokio::join!(
        sources::guide(http),
        sources::descriptions(http),
        sources::posters(http),
        sources::stills(http),
        sources::nyaa_releases(http, &current.file_lists),
    );
    let (episode_descriptions, arc_descriptions) = descriptions?;
    // Posters are decoration: without the poster source, keep the ones we have.
    let posters = posters.unwrap_or_else(|_| {
        current.arcs.iter().filter_map(|arc| Some((arc.title.clone(), arc.poster_url.clone()?))).collect()
    });
    // Stills are decoration too: without ani.zip, keep the ones we have.
    let stills_failed = stills.is_err();
    let mut fresh = build::build(build::Inputs {
        guide_tabs: guide_tabs?,
        episode_descriptions,
        arc_descriptions,
        posters,
        stills: stills.unwrap_or_default(),
        releases: releases?,
        built_at,
    })?;
    if stills_failed {
        fresh.keep_thumbnails(current);
    }
    let (kept, before) = (fresh.playable_episodes(), current.playable_episodes());
    if (kept as f64) < before as f64 * MIN_KEPT_SHARE {
        return Err(format!("the rebuilt One Pace mapping has {kept} playable Episodes, down from {before}"));
    }
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_mapping_parses_and_every_episode_plays() {
        let mapping = bundled().unwrap();
        let episodes: usize = mapping.arcs.iter().map(|arc| arc.episodes.len()).sum();
        assert!(mapping.arcs.len() >= 36, "only {} Arcs", mapping.arcs.len());
        assert_eq!(mapping.playable_episodes(), episodes);
        let ids: std::collections::HashSet<i64> = mapping.arcs.iter().map(|arc| arc.id).collect();
        assert_eq!(ids.len(), mapping.arcs.len(), "two Arcs share an ID");
        for arc in &mapping.arcs {
            for episode in &arc.episodes {
                let chosen = episode.chosen.as_ref().unwrap();
                assert!(mapping.releases.contains_key(chosen), "{} plays an unknown Release", episode.label);
            }
        }
    }

    async fn store_with(built_at: i64) -> Store {
        let store = Store::in_memory().unwrap();
        let saved = Mapping { built_at, ..Mapping::default() };
        store.set_meta(MAPPING_KEY, Some(serde_json::to_string(&saved).unwrap())).await.unwrap();
        store
    }

    #[tokio::test]
    async fn the_newer_of_the_saved_and_bundled_mappings_wins() {
        let bundled_at = bundled().unwrap().built_at;
        let newer = store_with(bundled_at + 1).await;
        assert_eq!(OnePace::default().mapping(&newer).await.unwrap().built_at, bundled_at + 1);

        let older = store_with(bundled_at - 1).await;
        assert_eq!(OnePace::default().mapping(&older).await.unwrap().built_at, bundled_at);

        let empty = Store::in_memory().unwrap();
        assert!(OnePace::default().mapping(&empty).await.unwrap().arcs.len() >= 36);
    }

    /// Rebuilds `onepace.json` from the live sources. Run with
    /// `cargo test dump_bundled_mapping -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "calls Google Sheets, GitHub, ani.zip and Nyaa"]
    async fn dump_bundled_mapping() {
        let current = bundled().unwrap_or_default();
        let mapping = rebuild(&crate::index::http_client(), &current, now()).await.unwrap();
        for arc in &mapping.arcs {
            let playable = arc.episodes.iter().filter(|e| e.chosen.is_some()).count();
            let stills = arc.episodes.iter().filter(|e| e.thumbnail_url.is_some()).count();
            println!(
                "{:<34} {playable:>3}/{:<3} stills={stills:<3} poster={}",
                arc.title,
                arc.episodes.len(),
                arc.poster_url.is_some()
            );
        }
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/onepace/onepace.json");
        std::fs::write(path, serde_json::to_string(&mapping).unwrap()).unwrap();
    }
}
