//! Stream lifecycle. One Stream is one Episode from one Source: TorBox when connected,
//! else Local Torrent. Every Stream ends as one HTTP URL for mpv. The UI follows each
//! Stream through `stream://update` events and `stream://removed` when it goes away.

mod cleanup;
pub mod model;
mod prepare;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::sources::local::LocalTorrentState;
use crate::sources::torbox::TorBoxState;
use crate::sources::SourceKind;
use crate::store::now;
use model::StreamView;

/// TorBox links last 3 hours. A Stream older than this asks for a new one.
const LINK_REFRESH_AFTER: Duration = Duration::from_secs(2 * 3600);
const TICK: Duration = Duration::from_secs(1);

/// What the Source holds for a Stream, so the Delete-after-Watched rules can find it.
#[derive(Debug, Clone, PartialEq)]
pub enum Handle {
    Pending,
    TorBox {
        info_hash: String,
        torrent_id: i64,
    },
    /// Local Torrent reads the file list and adds the torrent.
    Opening {
        info_hash: String,
    },
    Local {
        info_hash: String,
        torrent_id: usize,
        file_id: u64,
    },
}

impl Handle {
    /// The local torrent this Stream holds or is about to hold.
    fn local_hash(&self) -> Option<&str> {
        match self {
            Handle::Opening { info_hash } | Handle::Local { info_hash, .. } => Some(info_hash),
            _ => None,
        }
    }
}

struct Entry {
    view: StreamView,
    handle: Handle,
    task: Option<JoinHandle<()>>,
    created: Instant,
}

#[derive(Default)]
pub struct StreamState {
    streams: Mutex<HashMap<String, Entry>>,
    ticking: AtomicBool,
}

impl StreamState {
    fn with<T>(&self, f: impl FnOnce(&mut HashMap<String, Entry>) -> T) -> T {
        let mut streams = self.streams.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f(&mut streams)
    }
}

/// Changes one Stream and sends the new state to the UI. The event goes out under the
/// lock, so it can never arrive after the Stream's `stream://removed` event.
pub fn update(app: &AppHandle, id: &str, change: impl FnOnce(&mut StreamView)) {
    app.state::<StreamState>().with(|streams| {
        if let Some(entry) = streams.get_mut(id) {
            change(&mut entry.view);
            let _ = app.emit("stream://update", &entry.view);
        }
    });
}

/// Records what the Source holds for the Stream. False when the Stream was removed.
pub fn set_handle(app: &AppHandle, id: &str, handle: Handle) -> bool {
    app.state::<StreamState>().with(|streams| match streams.get_mut(id) {
        Some(entry) => {
            entry.handle = handle;
            true
        }
        None => false,
    })
}

/// Starts a Stream for the Episode, or returns the one that already plays it.
#[tauri::command]
pub async fn stream_start(
    app: AppHandle,
    state: State<'_, StreamState>,
    torbox: State<'_, TorBoxState>,
    show_id: i64,
    episode: i64,
) -> Result<StreamView, String> {
    let source = if torbox.key()?.is_some() { SourceKind::TorBox } else { SourceKind::LocalTorrent };
    let reusable = |entry: &Entry| {
        entry.view.show_id == show_id
            && entry.view.episode == episode
            && entry.view.source == source
            && !entry.view.is_failed()
            && (source == SourceKind::LocalTorrent || entry.created.elapsed() < LINK_REFRESH_AFTER)
    };
    let id = uuid::Uuid::new_v4().simple().to_string();
    let view = StreamView::new(id.clone(), show_id, episode, source, now());
    // One lock for the check and the insert, so two calls cannot start two Streams.
    let stale = state.with(|streams| {
        if let Some(existing) = streams.values().find(|e| reusable(e)) {
            return Err(Box::new(existing.view.clone()));
        }
        let stale: Vec<String> = streams
            .iter()
            .filter(|(_, e)| e.view.show_id == show_id && e.view.episode == episode)
            .map(|(id, _)| id.clone())
            .collect();
        let entry = Entry { view: view.clone(), handle: Handle::Pending, task: None, created: Instant::now() };
        streams.insert(id.clone(), entry);
        Ok(stale)
    });
    let stale = match stale {
        Ok(stale) => stale,
        Err(existing) => return Ok(*existing),
    };
    for old in stale {
        remove(&app, &old, false).await;
    }
    let task = tauri::async_runtime::spawn(prepare::run(app.clone(), id.clone(), show_id, episode, source));
    state.with(|streams| streams.get_mut(&id).map(|entry| entry.task = Some(task)));
    let _ = app.emit("stream://update", view.clone());
    Ok(view)
}

#[tauri::command]
pub fn stream_list(state: State<'_, StreamState>) -> Vec<StreamView> {
    let mut views: Vec<StreamView> = state.with(|streams| streams.values().map(|e| e.view.clone()).collect());
    views.sort_by_key(|v| std::cmp::Reverse(v.started_at));
    views
}

/// The player passed 90% of the Episode. Its data goes away when the Stream closes.
#[tauri::command]
pub async fn stream_watched(app: AppHandle, state: State<'_, StreamState>, id: String) -> Result<(), String> {
    update(&app, &id, |view| view.watched = true);
    let handle = state.with(|streams| streams.get(&id).map(|e| (e.handle.clone(), e.view.show_id, e.view.episode)));
    if let Some((Handle::TorBox { info_hash, .. }, show_id, episode)) = handle {
        let store = app.state::<crate::store::StoreState>();
        cleanup::mark_watched(store.get(&app).await?, &info_hash, show_id, episode).await?;
    }
    Ok(())
}

/// The player left the Stream. A Watched Stream ends here and its data is deleted.
#[tauri::command]
pub async fn stream_close(app: AppHandle, state: State<'_, StreamState>, id: String) -> Result<(), String> {
    let watched = state.with(|streams| streams.get(&id).is_some_and(|e| e.view.watched));
    if watched {
        remove(&app, &id, true).await;
    }
    Ok(())
}

/// Stops the Stream and deletes its local data (the Remove button on the Downloads page).
#[tauri::command]
pub async fn stream_remove(app: AppHandle, id: String) -> Result<(), String> {
    remove(&app, &id, false).await;
    Ok(())
}

/// Removes the Stream from the list, stops its task, and deletes what the rules allow.
async fn remove(app: &AppHandle, id: &str, watched: bool) {
    let Some(entry) = app.state::<StreamState>().with(|streams| streams.remove(id)) else {
        return;
    };
    if let Some(task) = entry.task {
        task.abort();
    }
    match entry.handle {
        Handle::TorBox { info_hash, torrent_id } if watched => {
            if let Err(err) = prepare::finish_torbox(app, &info_hash, torrent_id).await {
                let _ = app.emit("stream://error", err);
            }
        }
        handle => {
            if let Some(hash) = handle.local_hash() {
                release_local(app, hash).await;
            }
        }
    }
    let _ = app.emit("stream://removed", id);
}

/// Deletes a local torrent when no other Stream plays or opens a file from it.
pub async fn release_local(app: &AppHandle, info_hash: &str) {
    let in_use =
        app.state::<StreamState>().with(|streams| streams.values().any(|e| e.handle.local_hash() == Some(info_hash)));
    if in_use {
        return;
    }
    if let Some(engine) = app.state::<LocalTorrentState>().running() {
        if let Err(err) = engine.remove(info_hash).await {
            let _ = app.emit("stream://error", err);
        }
    }
}

/// Sends download progress of Local Torrent Streams to the UI once per second.
pub fn ensure_ticker(app: &AppHandle) {
    if app.state::<StreamState>().ticking.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK);
        loop {
            interval.tick().await;
            tick(&app);
        }
    });
}

fn tick(app: &AppHandle) {
    let local = app.state::<LocalTorrentState>();
    let Some(engine) = local.running() else { return };
    app.state::<StreamState>().with(|streams| {
        for entry in streams.values_mut() {
            let Handle::Local { torrent_id, file_id, .. } = entry.handle else { continue };
            let Some(progress) = engine.progress(torrent_id, file_id) else { continue };
            let view = &mut entry.view;
            let next = (progress.downloaded, progress.speed, progress.peers);
            if next != (view.downloaded, view.speed, view.peers) {
                (view.downloaded, view.speed, view.peers) = next;
                let _ = app.emit("stream://update", &*view);
            }
        }
    });
}

/// Runs when the app quits: removes Watched TorBox torrents, then stops Local Torrent
/// and deletes all its data, finished or not.
pub async fn shutdown(app: &AppHandle) {
    let watched: Vec<String> = app
        .state::<StreamState>()
        .with(|streams| streams.iter().filter(|(_, e)| e.view.watched).map(|(id, _)| id.clone()).collect());
    for id in watched {
        let _ = tokio::time::timeout(Duration::from_secs(3), remove(app, &id, true)).await;
    }
    app.state::<LocalTorrentState>().shutdown().await;
}
