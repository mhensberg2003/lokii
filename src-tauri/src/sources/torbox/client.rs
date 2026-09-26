//! TorBox REST client (`https://api.torbox.app/v1/api`). Every call uses the user's API
//! key as a Bearer token. Limits: 300 requests per minute, 60 uncached adds per hour.

use reqwest::{RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::sources::files::ReleaseFile;

const API_BASE: &str = "https://api.torbox.app/v1/api";

/// The standard TorBox response: `data` holds the result when `success` is true.
#[derive(Deserialize)]
struct Envelope<T> {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    detail: Option<String>,
    data: Option<T>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub plan: String,
    pub email: Option<String>,
    /// ISO date, UTC.
    pub premium_expires_at: Option<String>,
}

#[derive(Deserialize)]
struct RawAccount {
    plan: Option<i64>,
    email: Option<String>,
    premium_expires_at: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Torrent {
    pub id: i64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub download_state: String,
    /// The files are on TorBox's servers and can be downloaded.
    #[serde(default)]
    pub download_present: bool,
    /// 0 to 1.
    #[serde(default)]
    pub progress: f64,
    /// Bytes per second.
    #[serde(default)]
    pub download_speed: u64,
    #[serde(default)]
    pub seeds: i64,
    #[serde(default)]
    pub peers: i64,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub files: Vec<TorrentFile>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct TorrentFile {
    pub id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: u64,
}

impl Torrent {
    pub fn release_files(&self) -> Vec<ReleaseFile> {
        self.files.iter().map(|f| ReleaseFile { id: f.id, path: f.name.clone(), size: f.size }).collect()
    }

    pub fn failed(&self) -> bool {
        let state = self.download_state.to_lowercase();
        state == "error" || state.starts_with("failed")
    }
}

#[derive(Deserialize)]
struct Created {
    torrent_id: Option<i64>,
}

pub struct TorBox<'a> {
    pub http: &'a reqwest::Client,
    pub key: &'a str,
}

impl TorBox<'_> {
    pub async fn account(&self) -> Result<Account, String> {
        let raw: RawAccount = self.send(self.http.get(format!("{API_BASE}/user/me"))).await?;
        Ok(Account { plan: plan_name(raw.plan), email: raw.email, premium_expires_at: raw.premium_expires_at })
    }

    /// True when TorBox already has the Release, so adding it is instant.
    pub async fn is_cached(&self, hash: &str) -> Result<bool, String> {
        let url = format!("{API_BASE}/torrents/checkcached");
        let request = self.http.get(url).query(&[("hash", hash), ("format", "object")]);
        let data: Option<Value> = self.send_optional(request).await?;
        Ok(data.is_some_and(|data| is_cached_in(&data, hash)))
    }

    /// Adds the Release to the user's TorBox list and returns its torrent ID.
    pub async fn add(&self, magnet: &str) -> Result<i64, String> {
        let form = reqwest::multipart::Form::new().text("magnet", magnet.to_string());
        let created: Created =
            self.send(self.http.post(format!("{API_BASE}/torrents/createtorrent")).multipart(form)).await?;
        created.torrent_id.ok_or_else(|| "TorBox queued the Release. Try again when it starts.".to_string())
    }

    /// The torrent with this ID, or `None` when it is no longer in the user's list.
    pub async fn torrent(&self, id: i64) -> Result<Option<Torrent>, String> {
        let url = format!("{API_BASE}/torrents/mylist");
        let request = self.http.get(url).query(&[("id", id.to_string()), ("bypass_cache", "true".into())]);
        match self.send::<Torrent>(request).await {
            Ok(torrent) => Ok(Some(torrent)),
            Err(err) if err == ITEM_NOT_FOUND => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Every torrent in the user's list.
    pub async fn torrents(&self) -> Result<Vec<Torrent>, String> {
        let url = format!("{API_BASE}/torrents/mylist");
        self.send(self.http.get(url).query(&[("bypass_cache", "true")])).await
    }

    /// A direct HTTPS link to one file. TorBox keeps it valid for 3 hours.
    pub async fn download_link(&self, torrent_id: i64, file_id: u64) -> Result<String, String> {
        let url = format!("{API_BASE}/torrents/requestdl");
        let query = [
            ("token", self.key.to_string()),
            ("torrent_id", torrent_id.to_string()),
            ("file_id", file_id.to_string()),
            ("redirect", "false".into()),
        ];
        self.send(self.http.get(url).query(&query)).await
    }

    pub async fn delete(&self, torrent_id: i64) -> Result<(), String> {
        let body = serde_json::json!({ "torrent_id": torrent_id, "operation": "delete" });
        let url = format!("{API_BASE}/torrents/controltorrent");
        match self.send::<Value>(self.http.post(url).json(&body)).await {
            Ok(_) => Ok(()),
            Err(err) if err == ITEM_NOT_FOUND => Ok(()),
            Err(err) => Err(err),
        }
    }

    async fn send<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T, String> {
        self.send_optional(request).await?.ok_or_else(|| "TorBox sent an empty response.".to_string())
    }

    /// Like `send`, but a successful response without `data` is `None`.
    async fn send_optional<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<Option<T>, String> {
        let response =
            request.bearer_auth(self.key).send().await.map_err(|_| "TorBox is not reachable.".to_string())?;
        let status = response.status();
        let body = response.text().await.map_err(|_| "TorBox sent an unreadable response.".to_string())?;
        parse_envelope(status, &body)
    }
}

/// The error text for a torrent that is not in the user's list.
const ITEM_NOT_FOUND: &str = "ITEM_NOT_FOUND";

#[cfg(test)]
fn parse_response<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<T, String> {
    parse_envelope(status, body)?.ok_or_else(|| "TorBox sent an empty response.".to_string())
}

fn parse_envelope<T: DeserializeOwned>(status: StatusCode, body: &str) -> Result<Option<T>, String> {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err("TorBox did not accept the API key.".to_string());
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err("TorBox limits how often Lokii can ask. Try again in a minute.".to_string());
    }
    let envelope: Envelope<T> =
        serde_json::from_str(body).map_err(|e| format!("TorBox sent data we could not read (HTTP {status}): {e}"))?;
    if envelope.error.as_deref() == Some(ITEM_NOT_FOUND) {
        return Err(ITEM_NOT_FOUND.to_string());
    }
    match envelope.success {
        true => Ok(envelope.data),
        false => Err(envelope
            .detail
            .filter(|d| !d.is_empty())
            .or(envelope.error)
            .unwrap_or_else(|| format!("TorBox returned an error (HTTP {status})."))),
    }
}

/// `checkcached` returns `{ "<hash>": {...} }` for cached Releases and an empty object,
/// an empty list or `null` for the others.
fn is_cached_in(data: &Value, hash: &str) -> bool {
    match data {
        Value::Object(map) => map.keys().any(|k| k.eq_ignore_ascii_case(hash)),
        Value::Array(list) => !list.is_empty(),
        _ => false,
    }
}

fn plan_name(plan: Option<i64>) -> String {
    match plan {
        Some(1) => "Essential",
        Some(2) => "Pro",
        Some(3) => "Standard",
        _ => "Free",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TORRENT: &str = r#"{"success": true, "error": null, "detail": "Torrent list retrieved successfully.",
        "data": {"id": 42, "hash": "fcd7dbc76cfeca08d3888ccc446e8859fc7c90db", "name": "[MTBB] Shingeki no Kyojin",
        "download_state": "downloading", "download_present": false, "progress": 0.42,
        "download_speed": 5242880, "seeds": 12, "peers": 3, "size": 98000000000,
        "files": [{"id": 0, "name": "[MTBB] Shingeki no Kyojin/[MTBB] Shingeki no Kyojin - 01.mkv", "size": 1400000000,
        "short_name": "[MTBB] Shingeki no Kyojin - 01.mkv", "mimetype": "video/x-matroska"}]}}"#;

    #[test]
    fn reads_a_torrent_and_its_files() {
        let torrent: Torrent = parse_response(StatusCode::OK, TORRENT).unwrap();
        assert_eq!(torrent.id, 42);
        assert_eq!(torrent.progress, 0.42);
        assert!(!torrent.download_present);
        assert_eq!(
            torrent.release_files(),
            vec![ReleaseFile {
                id: 0,
                path: "[MTBB] Shingeki no Kyojin/[MTBB] Shingeki no Kyojin - 01.mkv".into(),
                size: 1_400_000_000
            }]
        );
    }

    #[test]
    fn a_failed_call_returns_the_user_friendly_detail() {
        let body = r#"{"success": false, "error": "ACTIVE_LIMIT", "detail": "You have reached your active limit.", "data": null}"#;
        let result: Result<Torrent, String> = parse_response(StatusCode::BAD_REQUEST, body);
        assert_eq!(result, Err("You have reached your active limit.".into()));
    }

    #[test]
    fn a_rejected_key_has_its_own_message() {
        let result: Result<Value, String> = parse_response(StatusCode::FORBIDDEN, "{}");
        assert_eq!(result, Err("TorBox did not accept the API key.".into()));
    }

    #[test]
    fn a_missing_torrent_is_item_not_found() {
        let body = r#"{"success": false, "error": "ITEM_NOT_FOUND", "detail": "Torrent not found.", "data": null}"#;
        let result: Result<Torrent, String> = parse_response(StatusCode::NOT_FOUND, body);
        assert_eq!(result, Err(ITEM_NOT_FOUND.into()));
    }

    #[test]
    fn reads_the_cached_check_in_every_shape() {
        let hash = "fcd7dbc76cfeca08d3888ccc446e8859fc7c90db";
        let cached = serde_json::json!({ hash: { "name": "x", "size": 1, "hash": hash } });
        assert!(is_cached_in(&cached, hash));
        assert!(!is_cached_in(&serde_json::json!({}), hash));
        assert!(!is_cached_in(&serde_json::json!([]), hash));
        assert!(!is_cached_in(&Value::Null, hash));
    }

    #[test]
    fn an_uncached_check_can_have_no_data() {
        let body = r#"{"success": true, "detail": "Hash not cached.", "data": null}"#;
        assert_eq!(parse_envelope::<Value>(StatusCode::OK, body), Ok(None));
    }

    #[test]
    fn reads_the_account_plan() {
        let body =
            r#"{"success": true, "data": {"plan": 2, "email": "a@b.c", "premium_expires_at": "2026-12-01T00:00:00Z"}}"#;
        let raw: RawAccount = parse_response(StatusCode::OK, body).unwrap();
        assert_eq!(plan_name(raw.plan), "Pro");
        assert_eq!(plan_name(None), "Free");
    }
}
