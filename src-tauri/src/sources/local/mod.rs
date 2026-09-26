//! Local Torrent Source: librqbit downloads only the file of the Episode, with piece
//! priority near the play position, and the stream-only server hands it to mpv.
//! All data sits in one Lokii folder, which the app empties when it starts and quits.

mod server;
pub mod settings;

use std::collections::HashSet;
use std::net::{Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use librqbit::api::TorrentIdOrHash;
use librqbit::dht::Id20;
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ByteBufOwned, ListenerOptions, ManagedTorrent, Session,
    SessionOptions, ValidatedTorrentMetaV1Info,
};
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OnceCell};

use crate::sources::files::ReleaseFile;
use server::StreamServer;
use settings::LocalSettings;

#[derive(Default)]
pub struct LocalTorrentState {
    engine: OnceCell<Engine>,
    started_with: OnceLock<LocalSettings>,
    /// Held while the data folder is emptied, so a start and a cleanup never overlap.
    disk: Mutex<()>,
}

impl LocalTorrentState {
    /// Starts the engine on first use with the saved settings.
    pub async fn engine(&self, app: &AppHandle, settings: LocalSettings) -> Result<&Engine, String> {
        self.engine
            .get_or_try_init(|| async {
                let _disk = self.disk.lock().await;
                let engine = Engine::start(settings.data_dir(app)?, settings.port).await?;
                let _ = self.started_with.set(settings);
                Ok::<_, String>(engine)
            })
            .await
    }

    pub fn running(&self) -> Option<&Engine> {
        self.engine.get()
    }

    pub fn started_with(&self) -> Option<&LocalSettings> {
        self.started_with.get()
    }

    /// Deletes data that a crash or a failed quit left behind. Runs when the app starts.
    pub async fn clear_leftovers(&self, app: &AppHandle, settings: &LocalSettings) {
        let _disk = self.disk.lock().await;
        if self.engine.get().is_some() {
            return;
        }
        if let Ok(dir) = settings.data_dir(app) {
            let _ = tokio::fs::remove_dir_all(dir).await;
        }
    }

    /// Deletes every torrent with its data, then the data folder. Runs when the app quits.
    pub async fn shutdown(&self) {
        let Some(engine) = self.engine.get() else { return };
        let ids: Vec<usize> = engine.session.with_torrents(|torrents| torrents.map(|(id, _)| id).collect());
        for id in ids {
            // Deleting the torrent closes its files, so Windows can delete the folder.
            let _ = engine.session.delete(TorrentIdOrHash::Id(id), true).await;
        }
        engine.session.stop().await;
        let _ = tokio::fs::remove_dir_all(&engine.data_dir).await;
    }
}

/// Empties the Local Torrent data folder in the background when the app starts.
pub fn spawn_startup_cleanup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let store = app.state::<crate::store::StoreState>();
        let Ok(store) = store.get(&app).await else { return };
        let Ok(settings) = LocalSettings::load(store).await else { return };
        app.state::<LocalTorrentState>().clear_leftovers(&app, &settings).await;
    });
}

/// One file of a torrent that the server can stream.
pub struct Opened {
    pub torrent_id: usize,
    pub file: ReleaseFile,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalProgress {
    pub downloaded: u64,
    /// Bytes per second.
    pub speed: u64,
    pub peers: u32,
}

pub struct Engine {
    session: Arc<Session>,
    server: StreamServer,
    data_dir: PathBuf,
}

impl Engine {
    async fn start(data_dir: PathBuf, port: Option<u16>) -> Result<Self, String> {
        // Data left by a crash is never finished Episodes the user wants to keep.
        let _ = tokio::fs::remove_dir_all(&data_dir).await;
        tokio::fs::create_dir_all(&data_dir).await.map_err(|e| format!("cannot create {}: {e}", data_dir.display()))?;
        let listen = ListenerOptions {
            listen_addr: SocketAddr::from((Ipv6Addr::UNSPECIFIED, port.unwrap_or(0))),
            ..Default::default()
        };
        let options = SessionOptions { listen: Some(listen), ..Default::default() };
        let session = Session::new_with_opts(data_dir.clone(), options).await.map_err(|e| match port {
            Some(port) => format!("cannot start torrents on port {port}. Change the port in Settings. ({e:#})"),
            None => format!("cannot start torrents: {e:#}"),
        })?;
        Ok(Self { session, server: StreamServer::start().await?, data_dir })
    }

    /// Reads the torrent's file list, lets `choose` pick one file, and starts to
    /// download only that file (plus the files other Streams already play).
    pub async fn open(
        &self,
        info_hash: &str,
        magnet: &str,
        list_timeout: Duration,
        choose: impl FnOnce(&[ReleaseFile]) -> Result<u64, String>,
    ) -> Result<Opened, String> {
        if let Some(torrent) = self.managed(info_hash) {
            let files = torrent.with_metadata(|m| files_of(&m.info)).map_err(|e| format!("{e:#}"))?;
            let file = chosen(files, choose)?;
            self.add_file(&torrent, file.id as usize).await?;
            return self.opened(torrent, file).await;
        }

        let listing_options = AddTorrentOptions { list_only: true, ..Default::default() };
        let listing = tokio::time::timeout(
            list_timeout,
            self.session.add_torrent(AddTorrent::from_url(magnet), Some(listing_options)),
        )
        .await
        .map_err(|_| "No peers sent this Release. Try another release.".to_string())?;
        let AddTorrentResponse::ListOnly(listing) = listing.map_err(|e| format!("cannot read the torrent: {e:#}"))?
        else {
            return Err("cannot read the torrent's file list".to_string());
        };
        let file = chosen(files_of(&listing.info), choose)?;
        let options =
            AddTorrentOptions { only_files: Some(vec![file.id as usize]), overwrite: true, ..Default::default() };
        let added = self.session.add_torrent(AddTorrent::from_bytes(listing.torrent_bytes), Some(options)).await;
        let torrent = match added.map_err(|e| format!("cannot start the torrent: {e:#}"))? {
            AddTorrentResponse::Added(_, handle) => handle,
            AddTorrentResponse::AlreadyManaged(_, handle) => {
                self.add_file(&handle, file.id as usize).await?;
                handle
            }
            AddTorrentResponse::ListOnly(_) => return Err("the torrent was not added".to_string()),
        };
        self.opened(torrent, file).await
    }

    fn managed(&self, info_hash: &str) -> Option<Arc<ManagedTorrent>> {
        let hash = Id20::from_str(info_hash).ok()?;
        self.session.get(TorrentIdOrHash::Hash(hash))
    }

    /// Waits until librqbit can read the file, then makes it playable.
    async fn opened(&self, torrent: Arc<ManagedTorrent>, file: ReleaseFile) -> Result<Opened, String> {
        torrent.wait_until_initialized().await.map_err(|e| format!("cannot start the torrent: {e:#}"))?;
        let url = self.server.register(torrent.clone(), file.id as usize, &file.path);
        Ok(Opened { torrent_id: torrent.id(), file, url })
    }

    async fn add_file(&self, torrent: &Arc<ManagedTorrent>, index: usize) -> Result<(), String> {
        let mut only: HashSet<usize> = torrent.only_files().unwrap_or_default().into_iter().collect();
        if only.insert(index) {
            self.session.update_only_files(torrent, &only).await.map_err(|e| format!("cannot add the file: {e:#}"))?;
        }
        Ok(())
    }

    pub fn progress(&self, torrent_id: usize, file_id: u64) -> Option<LocalProgress> {
        let stats = self.session.get(TorrentIdOrHash::Id(torrent_id))?.stats();
        let live = stats.live.as_ref();
        Some(LocalProgress {
            downloaded: stats.file_progress.get(file_id as usize).copied().unwrap_or(0),
            speed: live.map_or(0, |l| (l.download_speed.mbps * 1024.0 * 1024.0) as u64),
            peers: live.map_or(0, |l| l.snapshot.peer_stats.live),
        })
    }

    /// Stops the torrent, stops seeding and deletes its data.
    pub async fn remove(&self, info_hash: &str) -> Result<(), String> {
        let Some(torrent) = self.managed(info_hash) else { return Ok(()) };
        self.server.unregister(torrent.id());
        self.session.delete(TorrentIdOrHash::Id(torrent.id()), true).await.map_err(|e| format!("{e:#}"))
    }
}

fn chosen(
    files: Vec<ReleaseFile>,
    choose: impl FnOnce(&[ReleaseFile]) -> Result<u64, String>,
) -> Result<ReleaseFile, String> {
    let id = choose(&files)?;
    files.into_iter().find(|f| f.id == id).ok_or_else(|| "the chosen file is not in the torrent".to_string())
}

fn files_of(info: &ValidatedTorrentMetaV1Info<ByteBufOwned>) -> Vec<ReleaseFile> {
    info.iter_file_details()
        .enumerate()
        .filter(|(_, f)| !f.attrs().padding)
        .map(|(index, f)| ReleaseFile { id: index as u64, path: f.filename.to_vec().join("/"), size: f.len })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Streams the first bytes of Big Buck Bunny (a Creative Commons film) through the
    /// stream-only server. Run with `cargo test live_local_stream -- --ignored --nocapture`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "downloads from BitTorrent peers"]
    async fn live_local_stream() {
        const MAGNET: &str = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c&dn=Big+Buck+Bunny\
            &tr=udp%3A%2F%2Fexplodie.org%3A6969&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337\
            &tr=wss%3A%2F%2Ftracker.webtorrent.dev&ws=https%3A%2F%2Fwebtorrent.io%2Ftorrents%2F";
        let dir = std::env::temp_dir().join("lokii-live-local-stream");
        let engine = Engine::start(dir.clone(), None).await.unwrap();
        let largest = |files: &[ReleaseFile]| Ok(files.iter().max_by_key(|f| f.size).unwrap().id);
        let hash = "dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c";
        let opened = engine.open(hash, MAGNET, Duration::from_secs(90), largest).await.unwrap();
        println!("file {:?} at {}", opened.file, opened.url);

        let response = reqwest::Client::new().get(&opened.url).header("Range", "bytes=0-1023").send().await.unwrap();
        assert_eq!(response.status(), 206);
        let range = response.headers()["content-range"].to_str().unwrap().to_string();
        assert_eq!(response.bytes().await.unwrap().len(), 1024);
        println!("content-range {range}, progress {:?}", engine.progress(opened.torrent_id, opened.file.id));

        let root = opened.url.rsplit_once("/stream/").unwrap().0.to_string();
        assert_eq!(reqwest::get(format!("{root}/torrents")).await.unwrap().status(), 404);
        assert_eq!(reqwest::get(format!("{root}/stream/wrong")).await.unwrap().status(), 404);

        engine.remove(hash).await.unwrap();
        assert_eq!(reqwest::get(&opened.url).await.unwrap().status(), 404);
        let _ = std::fs::remove_dir_all(dir);
    }
}
