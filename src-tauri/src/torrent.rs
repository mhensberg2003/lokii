//! Local Torrent source: librqbit downloads one file of a Release with piece priority
//! and serves it on 127.0.0.1 with HTTP Range support, so mpv can play while it downloads.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use librqbit::http_api::{HttpApi, HttpApiOptions};
use librqbit::{AddTorrent, AddTorrentOptions, AddTorrentResponse, Api, Session};
use librqbit_dualstack_sockets::socket::MaybeDualstackSocket;
use librqbit_dualstack_sockets::BindOpts;
use tauri::State;
use tokio::sync::OnceCell;

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "webm", "avi"];
const STREAM_USER: &str = "lokii";

#[derive(Default)]
pub struct TorrentEngine {
    inner: OnceCell<Engine>,
}

struct Engine {
    session: Arc<Session>,
    port: u16,
    token: String,
}

impl TorrentEngine {
    async fn get(&self, data_dir: PathBuf) -> anyhow::Result<&Engine> {
        self.inner.get_or_try_init(|| Engine::start(data_dir)).await
    }
}

impl Engine {
    async fn start(data_dir: PathBuf) -> anyhow::Result<Self> {
        let session = Session::new(data_dir).await.context("cannot start torrent session")?;
        let token = uuid::Uuid::new_v4().simple().to_string();
        let listener = MaybeDualstackSocket::bind_tcp(
            SocketAddr::from(([127, 0, 0, 1], 0)),
            BindOpts { request_dualstack: false, ..Default::default() },
        )
        .context("cannot open local stream port")?;
        let port = listener.bind_addr().port();

        // Read-only: the local server can stream files but cannot add or delete torrents.
        let opts = HttpApiOptions {
            read_only: true,
            basic_auth: Some((STREAM_USER.into(), token.clone())),
            ..Default::default()
        };
        let api = Api::new(session.clone(), None, None);
        tauri::async_runtime::spawn(HttpApi::new(api, Some(opts)).make_http_api_and_run(listener, None));

        Ok(Self { session, port, token })
    }

    /// Adds the torrent, downloads only its largest video file, and returns a URL mpv can play.
    async fn stream_url(&self, magnet: &str) -> anyhow::Result<String> {
        let listing = self
            .session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions { list_only: true, ..Default::default() }),
            )
            .await?;
        let AddTorrentResponse::ListOnly(listing) = listing else {
            return Err(anyhow!("torrent listing failed"));
        };
        let file_idx = largest_video_file(&listing)?;

        let added = self
            .session
            .add_torrent(
                AddTorrent::from_bytes(listing.torrent_bytes),
                Some(AddTorrentOptions { only_files: Some(vec![file_idx]), overwrite: true, ..Default::default() }),
            )
            .await?;
        let id = match added {
            AddTorrentResponse::Added(id, _) | AddTorrentResponse::AlreadyManaged(id, _) => id,
            AddTorrentResponse::ListOnly(_) => return Err(anyhow!("torrent was not added")),
        };

        Ok(format!("http://{STREAM_USER}:{}@127.0.0.1:{}/torrents/{id}/stream/{file_idx}", self.token, self.port))
    }
}

fn largest_video_file(listing: &librqbit::ListOnlyResponse) -> anyhow::Result<usize> {
    listing
        .info
        .iter_file_details()
        .enumerate()
        .filter(|(_, f)| !f.attrs().padding)
        .filter(|(_, f)| {
            let name = f.filename.to_vec().join("/").to_lowercase();
            VIDEO_EXTENSIONS.iter().any(|ext| name.ends_with(&format!(".{ext}")))
        })
        .max_by_key(|(_, f)| f.len)
        .map(|(idx, _)| idx)
        .ok_or_else(|| anyhow!("the Release has no video file"))
}

/// SPIKE-PROBE: prints a stream URL for LOKII_SPIKE_MAGNET so the stream can be checked with curl.
pub fn spike_probe(app: &tauri::AppHandle) {
    let Ok(magnet) = std::env::var("LOKII_SPIKE_MAGNET") else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;
        let engine = app.state::<TorrentEngine>();
        let result = torrent_stream(app.clone(), engine, magnet).await;
        eprintln!("SPIKE stream={result:?}");
    });
}

#[tauri::command]
pub async fn torrent_stream(
    app: tauri::AppHandle,
    engine: State<'_, TorrentEngine>,
    magnet: String,
) -> Result<String, String> {
    use tauri::Manager;
    let data_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?.join("torrents");
    let engine = engine.get(data_dir).await.map_err(|e| format!("{e:#}"))?;
    engine.stream_url(&magnet).await.map_err(|e| format!("{e:#}"))
}
