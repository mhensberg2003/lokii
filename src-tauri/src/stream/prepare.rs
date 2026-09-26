//! Prepares one Stream: finds the Chosen Release, asks the Source for it, picks the
//! Episode's file, and ends with a URL that mpv can play.

use std::time::Duration;

use tauri::{AppHandle, Manager};

use super::cleanup::{self, TorBoxItem};
use super::model::Phase;
use super::{set_handle, update, Handle};
use crate::catalog::model::ShowDetails;
use crate::catalog::CatalogState;
use crate::index::{self, IndexState, Resolved};
use crate::sources::files::{self, ReleaseFile};
use crate::sources::local::settings::LocalSettings;
use crate::sources::local::LocalTorrentState;
use crate::sources::torbox::client::{TorBox, Torrent};
use crate::sources::torbox::TorBoxState;
use crate::sources::SourceKind;
use crate::store::{Store, StoreState};

/// TorBox asks for polls every 2 to 3 seconds while a Stream prepares.
const POLL_INTERVAL: Duration = Duration::from_millis(2500);
/// Polls that may fail in a row before the Stream fails.
const POLL_RETRIES: u32 = 3;
/// How long peers have to send the torrent's file list.
const PEER_TIMEOUT: Duration = Duration::from_secs(90);

pub async fn run(app: AppHandle, id: String, show_id: i64, episode: i64, source: SourceKind) {
    if let Err(message) = prepare(&app, &id, show_id, episode, source).await {
        update(&app, &id, |view| view.phase = Phase::Failed { message });
    }
}

async fn prepare(app: &AppHandle, id: &str, show_id: i64, episode: i64, source: SourceKind) -> Result<(), String> {
    let store_state = app.state::<StoreState>();
    let store = store_state.get(app).await?;
    let (catalog, index) = (app.state::<CatalogState>(), app.state::<IndexState>());
    let resolved = index::resolve(&catalog, store, index.http(), show_id, episode).await?;
    update(app, id, |view| {
        view.show_title = resolved.show.lite.card.title.clone();
        view.release_title = Some(resolved.release.title.clone());
        view.release_group = resolved.release.group.clone();
        view.size = resolved.release.size_bytes;
    });
    match source {
        SourceKind::TorBox => torbox(app, id, store, &resolved, episode).await,
        SourceKind::LocalTorrent => local(app, id, store, &resolved, episode).await,
    }
}

fn step(app: &AppHandle, id: &str, step: &str) {
    update(app, id, |view| view.phase = Phase::Preparing { step: step.to_string() });
}

fn ready(app: &AppHandle, id: &str, file: &ReleaseFile, url: String, downloaded: u64) {
    update(app, id, |view| {
        view.phase = Phase::Ready;
        view.url = Some(url);
        view.file_name = Some(file.path.rsplit('/').next().unwrap_or(&file.path).to_string());
        view.size = file.size;
        view.downloaded = downloaded;
    });
}

async fn local(app: &AppHandle, id: &str, store: &Store, r: &Resolved, episode: i64) -> Result<(), String> {
    let settings = LocalSettings::load(store).await?;
    let state = app.state::<LocalTorrentState>();
    let engine = state.engine(app, settings).await?;
    let hash = r.release.info_hash.clone();
    // Marks the torrent as in use before it exists, so another Stream's cleanup keeps it.
    if !set_handle(app, id, Handle::Opening { info_hash: hash.clone() }) {
        return Ok(());
    }
    step(app, id, "Finding peers");
    let (release, context) = (r.release.clone(), r.context.clone());
    let choose = move |list: &[ReleaseFile]| files::pick_release(list, &release, &context, episode);
    let opened = match engine.open(&hash, &r.release.magnet, PEER_TIMEOUT, choose).await {
        Ok(opened) => opened,
        Err(err) => {
            set_handle(app, id, Handle::Pending);
            super::release_local(app, &hash).await;
            return Err(err);
        }
    };
    let handle = Handle::Local { info_hash: hash.clone(), torrent_id: opened.torrent_id, file_id: opened.file.id };
    if !set_handle(app, id, handle) {
        // The Stream was removed while the torrent opened.
        super::release_local(app, &hash).await;
        return Ok(());
    }
    ready(app, id, &opened.file, opened.url, 0);
    super::ensure_ticker(app);
    Ok(())
}

async fn torbox(app: &AppHandle, id: &str, store: &Store, r: &Resolved, episode: i64) -> Result<(), String> {
    let state = app.state::<TorBoxState>();
    let key = state.key()?.ok_or("TorBox is not connected.")?;
    let client = TorBox { http: state.http(), key: &key };
    let torrent_id = torbox_torrent(app, id, &client, store, r).await?;
    set_handle(app, id, Handle::TorBox { info_hash: r.release.info_hash.clone(), torrent_id });

    let torrent = wait_until_present(app, id, &client, torrent_id).await?;
    let list = torrent.release_files();
    let file_id = files::pick_release(&list, &r.release, &r.context, episode)?;
    let file = list.iter().find(|f| f.id == file_id).ok_or("the chosen file is not in the torrent")?;
    step(app, id, "Getting the link");
    let url = client.download_link(torrent_id, file_id).await?;
    ready(app, id, file, url, file.size);
    Ok(())
}

/// The torrent ID in the user's TorBox list: one Lokii added before, one the user
/// already has, or a new one.
async fn torbox_torrent(
    app: &AppHandle,
    id: &str,
    client: &TorBox<'_>,
    store: &Store,
    r: &Resolved,
) -> Result<i64, String> {
    let hash = r.release.info_hash.as_str();
    let row = TorBoxItem::new(
        hash,
        0,
        r.show.lite.card.id,
        cleanup::episode_range(r.release.coverage, episode_count(&r.show)),
        true,
    );
    step(app, id, "Checking TorBox");
    let rows = cleanup::find_all(store, hash).await?;
    // A row with torrent ID 0 is an add that stopped before TorBox answered: still Lokii's.
    let owned = rows.iter().any(|r| r.added_by_lokii);
    if let Some(known) = rows.iter().map(|r| r.torrent_id).find(|t| *t > 0) {
        if client.torrent(known).await?.is_some() {
            save_row(store, &rows, row, known, owned).await?;
            return Ok(known);
        }
    }
    if let Some(existing) = client.torrents().await?.into_iter().find(|t| t.hash.eq_ignore_ascii_case(hash)) {
        save_row(store, &rows, row, existing.id, owned).await?;
        return Ok(existing.id);
    }

    cleanup::forget(store, hash).await?;
    cleanup::save(store, row.clone()).await?;
    let cached = client.is_cached(hash).await.unwrap_or(false);
    step(app, id, if cached { "Adding to TorBox" } else { "Sending to TorBox" });
    let torrent_id = client.add(&r.release.magnet).await?;
    cleanup::save(store, TorBoxItem { torrent_id, ..row }).await?;
    Ok(torrent_id)
}

/// Saves this Show's row for the torrent, and keeps its Watched Episodes.
async fn save_row(
    store: &Store,
    rows: &[TorBoxItem],
    new: TorBoxItem,
    torrent_id: i64,
    owned: bool,
) -> Result<(), String> {
    let mut item = rows.iter().find(|r| r.show_id == new.show_id).cloned().unwrap_or(new);
    item.torrent_id = torrent_id;
    item.added_by_lokii = owned;
    cleanup::save(store, item).await
}

/// Polls TorBox until the Release's files are on its servers.
async fn wait_until_present(
    app: &AppHandle,
    id: &str,
    client: &TorBox<'_>,
    torrent_id: i64,
) -> Result<Torrent, String> {
    let mut failures = 0;
    loop {
        let torrent = match client.torrent(torrent_id).await {
            Ok(Some(torrent)) => torrent,
            Ok(None) => return Err("The Release was removed from TorBox.".to_string()),
            Err(err) if failures >= POLL_RETRIES => return Err(err),
            Err(_) => {
                failures += 1;
                tokio::time::sleep(POLL_INTERVAL).await;
                continue;
            }
        };
        failures = 0;
        if torrent.failed() {
            return Err("TorBox could not download this Release. Try another release.".to_string());
        }
        if torrent.download_present && !torrent.files.is_empty() {
            return Ok(torrent);
        }
        update(app, id, |view| {
            view.phase = Phase::Downloading { progress: torrent.progress.clamp(0.0, 1.0) };
            view.speed = torrent.download_speed;
            view.peers = u32::try_from(torrent.seeds.max(0)).unwrap_or(0);
            view.downloaded = (torrent.size as f64 * torrent.progress.clamp(0.0, 1.0)) as u64;
        });
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Removes a TorBox torrent once Lokii added it and every Show that plays it has
/// Watched all of its Episodes.
pub async fn finish_torbox(app: &AppHandle, info_hash: &str, torrent_id: i64) -> Result<(), String> {
    let store_state = app.state::<StoreState>();
    let store = store_state.get(app).await?;
    if !cleanup::can_remove_torrent(&cleanup::find_all(store, info_hash).await?) {
        return Ok(());
    }
    let state = app.state::<TorBoxState>();
    let Some(key) = state.key()? else { return Ok(()) };
    TorBox { http: state.http(), key: &key }.delete(torrent_id).await?;
    cleanup::forget(store, info_hash).await
}

fn episode_count(show: &ShowDetails) -> i64 {
    show.lite.card.episodes.unwrap_or(show.episode_list.len() as i64)
}
