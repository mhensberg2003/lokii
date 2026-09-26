//! The stream-only HTTP server on 127.0.0.1. It serves one route, `/stream/<token>`,
//! with Range support, so mpv can seek while librqbit downloads. Each token is a random
//! ID for one file of one torrent; without a token the server shows nothing.

use std::collections::HashMap;
use std::io::SeekFrom;
use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use librqbit::ManagedTorrent;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

#[derive(Clone)]
struct Target {
    torrent: Arc<ManagedTorrent>,
    file_id: usize,
    mime: &'static str,
}

type Targets = Arc<RwLock<HashMap<String, Target>>>;

pub struct StreamServer {
    port: u16,
    targets: Targets,
}

impl StreamServer {
    pub async fn start() -> Result<Self, String> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| format!("cannot open the local stream port: {e}"))?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let targets = Targets::default();
        let router = Router::new().route("/stream/{token}", get(stream)).with_state(targets.clone());
        tauri::async_runtime::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(Self { port, targets })
    }

    /// Makes one file playable and returns its URL.
    pub fn register(&self, torrent: Arc<ManagedTorrent>, file_id: usize, path: &str) -> String {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let target = Target { torrent, file_id, mime: mime_type(path) };
        if let Ok(mut targets) = self.targets.write() {
            targets.insert(token.clone(), target);
        }
        format!("http://127.0.0.1:{}/stream/{token}", self.port)
    }

    /// Stops serving every file of the torrent.
    pub fn unregister(&self, torrent_id: usize) {
        if let Ok(mut targets) = self.targets.write() {
            targets.retain(|_, target| target.torrent.id() != torrent_id);
        }
    }
}

async fn stream(State(targets): State<Targets>, Path(token): Path<String>, headers: HeaderMap) -> Response {
    let target = targets.read().ok().and_then(|targets| targets.get(&token).cloned());
    let Some(target) = target else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match respond(target, &headers).await {
        Ok(response) => response,
        Err(status) => status.into_response(),
    }
}

async fn respond(target: Target, headers: &HeaderMap) -> Result<Response, StatusCode> {
    let mut file = target.torrent.clone().stream(target.file_id).await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let len = file.len();
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (status, start, end) = match parse_range(range, len)? {
        Some((start, end)) => (StatusCode::PARTIAL_CONTENT, start, end),
        None => (StatusCode::OK, 0, len),
    };
    file.seek(SeekFrom::Start(start)).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut out = HeaderMap::new();
    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static(target.mime));
    out.insert(header::CONTENT_LENGTH, HeaderValue::from(end - start));
    if status == StatusCode::PARTIAL_CONTENT {
        let value = format!("bytes {start}-{}/{len}", end - 1);
        out.insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&value).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        );
    }
    let body = tokio_util::io::ReaderStream::with_capacity(file.take(end - start), 64 * 1024);
    Ok((status, out, Body::from_stream(body)).into_response())
}

/// Reads a `bytes=start-end` header into a half-open byte range. `None` means the whole
/// file; an error means the range is outside the file.
fn parse_range(header: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, StatusCode> {
    let Some((start, end)) = header.and_then(|v| v.strip_prefix("bytes=")).and_then(|v| v.split_once('-')) else {
        return Ok(None);
    };
    let bad = StatusCode::RANGE_NOT_SATISFIABLE;
    let range = match (start.trim(), end.trim()) {
        ("", suffix) => {
            let suffix: u64 = suffix.parse().map_err(|_| bad)?;
            (len.saturating_sub(suffix), len)
        }
        (start, "") => (start.parse().map_err(|_| bad)?, len),
        (start, end) => {
            (start.parse().map_err(|_| bad)?, end.parse::<u64>().map_err(|_| bad)?.saturating_add(1).min(len))
        }
    };
    if range.0 >= range.1 || range.0 >= len {
        return Err(bad);
    }
    Ok(Some(range))
}

fn mime_type(path: &str) -> &'static str {
    match path.rsplit('.').next().map(str::to_lowercase).as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("avi") => "video/x-msvideo",
        Some("ts" | "m2ts") => "video/mp2t",
        _ => "video/x-matroska",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_range_header_means_the_whole_file() {
        assert_eq!(parse_range(None, 100), Ok(None));
        assert_eq!(parse_range(Some("items=1-2"), 100), Ok(None));
    }

    #[test]
    fn reads_open_closed_and_suffix_ranges() {
        assert_eq!(parse_range(Some("bytes=0-"), 100), Ok(Some((0, 100))));
        assert_eq!(parse_range(Some("bytes=10-19"), 100), Ok(Some((10, 20))));
        assert_eq!(parse_range(Some("bytes=90-500"), 100), Ok(Some((90, 100))));
        assert_eq!(parse_range(Some("bytes=-10"), 100), Ok(Some((90, 100))));
    }

    #[test]
    fn a_range_outside_the_file_is_refused() {
        assert_eq!(parse_range(Some("bytes=100-"), 100), Err(StatusCode::RANGE_NOT_SATISFIABLE));
        assert_eq!(parse_range(Some("bytes=20-10"), 100), Err(StatusCode::RANGE_NOT_SATISFIABLE));
        assert_eq!(parse_range(Some("bytes=a-b"), 100), Err(StatusCode::RANGE_NOT_SATISFIABLE));
    }

    #[test]
    fn the_mime_type_follows_the_extension() {
        assert_eq!(mime_type("a/b.MP4"), "video/mp4");
        assert_eq!(mime_type("a/b.mkv"), "video/x-matroska");
    }
}
