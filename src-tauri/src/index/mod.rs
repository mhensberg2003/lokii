//! Finding video: the Releases of a Show from the Index (AnimeTosho), the Best Release
//! from SeaDex, and the Chosen Release for each Episode.

mod animetosho;
mod choose;
pub mod matching;
pub mod model;
pub mod name;
mod picks;
mod seadex;

use std::collections::HashSet;
use std::time::Duration;

use tauri::{AppHandle, State};

use crate::catalog::{self, model::ShowDetails, CatalogState};
use crate::ids;
use crate::onepace;
use crate::store::{Store, StoreState};
use animetosho::{AnimeTosho, ToshoItem};
use choose::Pick;
use matching::ShowContext;
use model::{EpisodeReleases, Release};
use seadex::SeaDex;

pub const USER_AGENT: &str = concat!("Lokii/", env!("CARGO_PKG_VERSION"), " (+https://github.com/mhensberg2003/lokii)");

/// The HTTP client for every service except AniList. AnimeTosho refuses requests
/// without a User-Agent.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder().user_agent(USER_AGENT).timeout(Duration::from_secs(20)).build().unwrap_or_default()
}

pub struct IndexState {
    http: reqwest::Client,
}

impl Default for IndexState {
    fn default() -> Self {
        Self { http: http_client() }
    }
}

impl IndexState {
    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }
}

/// The Chosen Release of one Episode, with what a Source needs to find its file.
pub struct Resolved {
    pub show: ShowDetails,
    pub release: Release,
    pub context: ShowContext,
}

/// Finds the Chosen Release for the Episode.
pub async fn resolve(
    catalog: &CatalogState,
    store: &Store,
    http: &reqwest::Client,
    show_id: i64,
    episode: i64,
) -> Result<Resolved, String> {
    let deps = Deps { catalog, store, http };
    let mut list = episode_releases(&deps, show_id, episode).await?;
    let chosen = list.chosen.take().ok_or_else(|| format!("No release found for Episode {episode}."))?;
    let release = list.releases.into_iter().find(|r| r.info_hash == chosen).ok_or("the Chosen Release is missing")?;
    let show = catalog::show(catalog, store, show_id).await?;
    let context = ShowContext::from_show(&show);
    Ok(Resolved { show, release, context })
}

/// Everything one Index call needs, borrowed from Tauri state.
struct Deps<'a> {
    catalog: &'a CatalogState,
    store: &'a Store,
    http: &'a reqwest::Client,
}

#[tauri::command]
pub async fn index_releases(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
    index: State<'_, IndexState>,
    show_id: i64,
    episode: i64,
) -> Result<EpisodeReleases, String> {
    let deps = Deps { catalog: &catalog, store: store.get(&app).await?, http: &index.http };
    episode_releases(&deps, show_id, episode).await
}

/// Saves the user's pick for the Show (`None` returns to the automatic rule), then
/// returns the Episode's Releases with the new Chosen Release.
#[tauri::command]
pub async fn index_pick(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
    index: State<'_, IndexState>,
    show_id: i64,
    episode: i64,
    info_hash: Option<String>,
) -> Result<EpisodeReleases, String> {
    let deps = Deps { catalog: &catalog, store: store.get(&app).await?, http: &index.http };
    match info_hash {
        None => picks::save(deps.store, show_id, None).await?,
        Some(hash) => {
            let current = episode_releases(&deps, show_id, episode).await?;
            let release = current
                .releases
                .iter()
                .find(|r| r.info_hash == hash)
                .ok_or_else(|| "This Release is no longer in the list.".to_string())?;
            picks::save(deps.store, show_id, Some(Pick::from_release(release))).await?;
        }
    }
    episode_releases(&deps, show_id, episode).await
}

async fn episode_releases(deps: &Deps<'_>, show_id: i64, episode: i64) -> Result<EpisodeReleases, String> {
    let mut releases: Vec<Release> = if onepace::is_arc(show_id) {
        deps.catalog.onepace.releases(deps.store, show_id, episode).await?
    } else {
        let show = catalog::show(deps.catalog, deps.store, show_id).await?;
        show_releases(deps, &show).await?.into_iter().filter(|r| r.coverage.contains(episode)).collect()
    };
    choose::sort(&mut releases);
    let pick = picks::load(deps.store, show_id).await?;
    let (chosen, picked_by_user) = choose::choose(&releases, pick.as_ref());
    Ok(EpisodeReleases { show_id, episode, releases, chosen, picked_by_user })
}

/// Every Release of the Show that the matching rules accept, plus the Best Releases.
async fn show_releases(deps: &Deps<'_>, show: &ShowDetails) -> Result<Vec<Release>, String> {
    let tosho = AnimeTosho { http: deps.http, store: deps.store };
    let ttl =
        if show.status.as_deref() == Some("RELEASING") { animetosho::TTL_AIRING } else { animetosho::TTL_FINISHED };
    let aid = ids::lookup(deps.store, show.lite.card.id).await?.and_then(|ids| ids.anidb);
    let seadex = SeaDex { http: deps.http, store: deps.store };
    let (items, best) = tokio::join!(
        async {
            match aid {
                Some(aid) => tosho.by_anidb(aid, ttl).await,
                None => Ok(Vec::new()),
            }
        },
        seadex.best_hashes(show.lite.card.id),
    );
    // SeaDex only marks Releases, so the list still works while it is down.
    let (items, best) = (items?, best.unwrap_or_default());

    let ctx = ShowContext::from_show(show);
    let mut releases: Vec<Release> = items.iter().filter_map(|item| to_release(&ctx, item, &best)).collect();
    let found: HashSet<String> = releases.iter().map(|r| r.info_hash.clone()).collect();
    let missing = best.iter().filter(|hash| !found.contains(*hash)).map(|hash| tosho.by_hash(hash));
    for item in futures::future::join_all(missing).await.into_iter().flatten().flatten() {
        releases.extend(to_release(&ctx, &item, &best));
    }
    Ok(releases)
}

fn to_release(ctx: &ShowContext, item: &ToshoItem, best: &[String]) -> Option<Release> {
    let info_hash = item.info_hash.clone()?;
    let magnet = item.magnet_uri.clone()?;
    let parsed = name::parse(&item.title);
    let file_count = item.num_files.unwrap_or(1);
    let is_best = best.contains(&info_hash);
    let coverage = if is_best {
        matching::trusted_coverage(ctx, &parsed, file_count)
    } else {
        matching::coverage(ctx, &parsed, file_count)?
    };
    Some(Release {
        info_hash,
        title: item.title.clone(),
        group: parsed.group,
        resolution: parsed.resolution,
        size_bytes: item.total_size.unwrap_or(0),
        seeders: item.seeders.unwrap_or(0),
        leechers: item.leechers.unwrap_or(0),
        file_count,
        coverage,
        is_best,
        published_at: item.timestamp.unwrap_or(0),
        magnet,
        link: item.link.clone().unwrap_or_default(),
        file_path: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the full Index pipeline against the live services, prints a summary, and
    /// writes `preview/fixtures/releases-<show>-<episode>.json` for `pnpm preview:ui`.
    /// Run with `cargo test live_episode_releases -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore = "calls AniList, AnimeTosho and SeaDex"]
    async fn live_episode_releases() {
        let catalog = CatalogState::default();
        let store = Store::in_memory().unwrap();
        let http = http_client();
        let deps = Deps { catalog: &catalog, store: &store, http: &http };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../preview/fixtures");
        // Attack on Titan S1, its Final Season, and Frieren.
        for (show_id, episode) in [(16498, 1), (110277, 3), (154587, 5)] {
            let started = std::time::Instant::now();
            let result = episode_releases(&deps, show_id, episode).await.unwrap();
            println!("show {show_id} episode {episode}: {} Releases in {:?}", result.releases.len(), started.elapsed());
            for r in result.releases.iter().take(8) {
                let mark = if Some(&r.info_hash) == result.chosen.as_ref() { "*" } else { " " };
                println!(
                    "  {mark} best={} {:?} {:?}p s={} {:?} {}",
                    r.is_best, r.group, r.resolution, r.seeders, r.coverage, r.title
                );
            }
            let json = serde_json::to_string_pretty(&result).unwrap();
            std::fs::write(dir.join(format!("releases-{show_id}-{episode}.json")), json).unwrap();
        }
    }
}
