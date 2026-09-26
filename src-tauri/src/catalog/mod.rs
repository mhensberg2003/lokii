//! Catalog: Tauri commands backed by the AniList client (`anilist.rs`), a SQLite
//! response cache (`cache.rs`), and a 30-request-per-minute throttle (`throttle.rs`).
//! Output types mirror `src/lib/catalog.ts` exactly (see `model.rs`).

mod anilist;
mod cache;
mod franchise;
mod model;
mod queries;
mod raw;
mod season;
mod text;
mod throttle;

use std::collections::HashSet;

use tauri::{AppHandle, Manager, State};
use tokio::sync::OnceCell;

use anilist::{AniListClient, CachingFetcher};
use cache::Cache;
use model::{BrowseFeed, HomeFeed, NextAiring, ShowCard, ShowDetails, ShowRow};
use raw::{RawFranchiseNode, RawMedia, RawStudioConnection};

/// Genres the Browse page supports. Mirrors `BROWSE_GENRES` in `src/lib/catalog.ts`.
pub const BROWSE_GENRES: &[&str] = &[
    "Action",
    "Adventure",
    "Comedy",
    "Drama",
    "Fantasy",
    "Romance",
    "Sci-Fi",
    "Slice of Life",
    "Mystery",
    "Psychological",
    "Sports",
    "Supernatural",
    "Horror",
    "Mecha",
    "Music",
    "Thriller",
];

pub struct CatalogState {
    client: AniListClient,
    cache: OnceCell<Cache>,
}

impl Default for CatalogState {
    fn default() -> Self {
        Self { client: AniListClient::new(), cache: OnceCell::new() }
    }
}

impl CatalogState {
    /// Lazily opens the SQLite cache at `<app data dir>/lokii.db` on first use.
    async fn cache(&self, app: &AppHandle) -> Result<&Cache, String> {
        self.cache
            .get_or_try_init(|| async {
                let dir = app.path().app_data_dir().map_err(|e| format!("cannot find the app data folder: {e}"))?;
                Cache::open(dir.join("lokii.db")).await
            })
            .await
    }
}

#[tauri::command]
pub async fn catalog_home(app: AppHandle, state: State<'_, CatalogState>) -> Result<HomeFeed, String> {
    let cache = state.cache(&app).await?;
    home(&state.client, cache).await
}

async fn home(client: &AniListClient, cache: &Cache) -> Result<HomeFeed, String> {
    let (season, year) = season::current_season();
    let data = client.fetch_home(cache, season, year).await?;

    let hero_raw = data
        .trending
        .media
        .iter()
        .find(|m| m.banner_image.is_some() && m.description.is_some())
        .or_else(|| data.trending.media.first())
        .ok_or_else(|| "AniList returned no Shows for Trending now.".to_string())?;

    let rows = vec![
        row("airing", "Airing this season", &data.airing.media),
        row("trending", "Trending now", &data.trending.media),
        row("popular", "All-time popular", &data.popular.media),
        row("top", "Top rated", &data.top.media),
    ];
    Ok(HomeFeed { hero: model::to_show_details_lite(hero_raw), rows })
}

#[tauri::command]
pub async fn catalog_browse(app: AppHandle, state: State<'_, CatalogState>, genre: String) -> Result<BrowseFeed, String> {
    if !BROWSE_GENRES.contains(&genre.as_str()) {
        return Err(format!("\"{genre}\" is not a Browse genre."));
    }
    let cache = state.cache(&app).await?;
    browse(&state.client, cache, genre).await
}

async fn browse(client: &AniListClient, cache: &Cache, genre: String) -> Result<BrowseFeed, String> {
    let (season, year) = season::current_season();
    let data = client.fetch_browse(cache, &genre, season, year).await?;

    let rows = vec![
        row("airing", "Airing now", &data.airing.media),
        row("popular", "Most popular", &data.popular.media),
        row("top", "Top rated", &data.top.media),
        row("season", "This season", &data.season.media),
    ];
    Ok(BrowseFeed { genre, rows })
}

#[tauri::command]
pub async fn catalog_show(app: AppHandle, state: State<'_, CatalogState>, id: i64) -> Result<ShowDetails, String> {
    let cache = state.cache(&app).await?;
    show(&state.client, cache, id).await
}

async fn show(client: &AniListClient, cache: &Cache, id: i64) -> Result<ShowDetails, String> {
    let media = client.fetch_show(cache, id).await?;

    let start = RawFranchiseNode::from_media(&media);
    let fetcher = CachingFetcher { client, cache };
    let franchise = franchise::walk(start, &fetcher).await;
    let franchise_ids: HashSet<i64> = franchise.iter().map(|entry| entry.id).collect();

    let related = media
        .relations
        .as_ref()
        .map(|relations| model::build_related(&relations.edges, &franchise_ids))
        .unwrap_or_default();

    Ok(ShowDetails {
        lite: model::to_show_details_lite(&media),
        id_mal: media.id_mal,
        title_romaji: media.title.romaji.clone(),
        title_native: media.title.native.clone(),
        status: media.status.clone(),
        duration: media.duration,
        studios: studio_names(media.studios.as_ref()),
        next_airing: media.next_airing_episode.as_ref().map(|next| NextAiring { episode: next.episode, airing_at: next.airing_at }),
        episode_list: model::build_episode_list(&media),
        franchise,
        related,
    })
}

#[tauri::command]
pub async fn catalog_search(app: AppHandle, state: State<'_, CatalogState>, query: String) -> Result<Vec<ShowCard>, String> {
    let trimmed = query.trim();
    if trimmed.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let cache = state.cache(&app).await?;
    search(&state.client, cache, trimmed).await
}

async fn search(client: &AniListClient, cache: &Cache, query: &str) -> Result<Vec<ShowCard>, String> {
    let results = client.fetch_search(cache, query).await?;
    Ok(results.iter().map(model::to_show_card).collect())
}

fn row(id: &str, title: &str, media: &[RawMedia]) -> ShowRow {
    ShowRow { id: id.to_string(), title: title.to_string(), shows: media.iter().map(model::to_show_card).collect() }
}

fn studio_names(studios: Option<&RawStudioConnection>) -> Vec<String> {
    studios.map(|s| s.nodes.iter().map(|node| node.name.clone()).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writes real AniList responses to `preview/fixtures/` for the browser preview
    /// (`pnpm preview:ui`). Run with `cargo test dump_preview_fixtures -- --ignored`.
    #[tokio::test]
    #[ignore = "calls the live AniList API"]
    async fn dump_preview_fixtures() {
        let client = AniListClient::new();
        let cache = Cache::in_memory().unwrap();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../preview/fixtures");
        std::fs::create_dir_all(&dir).unwrap();
        let write = |name: &str, value: &dyn erased::Json| {
            std::fs::write(dir.join(name), value.json()).unwrap();
        };

        let home_feed = home(&client, &cache).await.unwrap();
        write("home.json", &home_feed);
        write("browse-Action.json", &browse(&client, &cache, "Action".into()).await.unwrap());
        write("search.json", &search(&client, &cache, "frieren").await.unwrap());
        // The hero, plus Attack on Titan (16498) for a long Franchise season strip.
        for id in [home_feed.hero.card.id, 16498] {
            write(&format!("show-{id}.json"), &show(&client, &cache, id).await.unwrap());
        }
    }

    mod erased {
        pub trait Json {
            fn json(&self) -> String;
        }
        impl<T: serde::Serialize> Json for T {
            fn json(&self) -> String {
                serde_json::to_string_pretty(self).unwrap()
            }
        }
    }

    #[test]
    fn browse_genres_match_the_ui_contract() {
        // Kept in sync by hand with BROWSE_GENRES in src/lib/catalog.ts.
        assert_eq!(BROWSE_GENRES.len(), 16);
        assert!(BROWSE_GENRES.contains(&"Sci-Fi"));
        assert!(BROWSE_GENRES.contains(&"Slice of Life"));
    }
}
