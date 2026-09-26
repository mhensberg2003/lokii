//! AniList GraphQL client: builds the aliased queries, throttles to 30 requests per
//! minute, retries once on HTTP 429 honoring `Retry-After`, and caches every response.

use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::catalog::cache::{cache_key, Cache};
use crate::catalog::franchise::NodeFetcher;
use crate::catalog::model::Season;
use crate::catalog::queries::{with_fragment, BROWSE_QUERY, FRANCHISE_NODE_QUERY, HOME_QUERY, SEARCH_QUERY, SHOW_QUERY};
use crate::catalog::raw::{RawFranchiseNode, RawMedia, RawMediaPage};
use crate::catalog::throttle::Throttle;

const DEFAULT_ENDPOINT: &str = "https://graphql.anilist.co";

pub const TTL_HOME: Duration = Duration::from_secs(6 * 3600);
pub const TTL_BROWSE: Duration = Duration::from_secs(6 * 3600);
pub const TTL_SHOW: Duration = Duration::from_secs(24 * 3600);
pub const TTL_FRANCHISE: Duration = Duration::from_secs(7 * 24 * 3600);
pub const TTL_SEARCH: Duration = Duration::from_secs(3600);

#[derive(Deserialize)]
pub struct HomeData {
    pub airing: RawMediaPage,
    pub trending: RawMediaPage,
    pub popular: RawMediaPage,
    pub top: RawMediaPage,
}

#[derive(Deserialize)]
pub struct BrowseData {
    pub airing: RawMediaPage,
    pub popular: RawMediaPage,
    pub top: RawMediaPage,
    pub season: RawMediaPage,
}

#[derive(Deserialize)]
struct ShowData {
    #[serde(rename = "Media")]
    media: RawMedia,
}

#[derive(Deserialize)]
struct SearchData {
    #[serde(rename = "Page")]
    page: RawMediaPage,
}

#[derive(Deserialize)]
struct FranchiseNodeData {
    #[serde(rename = "Media")]
    media: RawFranchiseNode,
}

pub struct AniListClient {
    http: reqwest::Client,
    endpoint: String,
    throttle: Throttle,
}

impl Default for AniListClient {
    fn default() -> Self {
        Self::new()
    }
}

impl AniListClient {
    pub fn new() -> Self {
        Self { http: reqwest::Client::new(), endpoint: DEFAULT_ENDPOINT.to_string(), throttle: Throttle::anilist() }
    }

    #[cfg(test)]
    fn with_endpoint(endpoint: impl Into<String>) -> Self {
        Self { http: reqwest::Client::new(), endpoint: endpoint.into(), throttle: Throttle::anilist() }
    }

    pub async fn fetch_home(&self, cache: &Cache, season: Season, season_year: i32) -> Result<HomeData, String> {
        let variables = json!({ "season": season, "seasonYear": season_year });
        self.cached_query("home", HOME_QUERY, variables, TTL_HOME, cache).await
    }

    pub async fn fetch_browse(
        &self,
        cache: &Cache,
        genre: &str,
        season: Season,
        season_year: i32,
    ) -> Result<BrowseData, String> {
        let variables = json!({ "genre": genre, "season": season, "seasonYear": season_year });
        self.cached_query("browse", BROWSE_QUERY, variables, TTL_BROWSE, cache).await
    }

    pub async fn fetch_show(&self, cache: &Cache, id: i64) -> Result<RawMedia, String> {
        let variables = json!({ "id": id });
        let data: ShowData = self.cached_query("show", SHOW_QUERY, variables, TTL_SHOW, cache).await?;
        Ok(data.media)
    }

    pub async fn fetch_search(&self, cache: &Cache, search: &str) -> Result<Vec<RawMedia>, String> {
        let variables = json!({ "search": search });
        let data: SearchData = self.cached_query("search", SEARCH_QUERY, variables, TTL_SEARCH, cache).await?;
        Ok(data.page.media)
    }

    async fn fetch_franchise_node(&self, cache: &Cache, id: i64) -> Result<RawFranchiseNode, String> {
        let variables = json!({ "id": id });
        let data: FranchiseNodeData =
            self.cached_query("franchise", FRANCHISE_NODE_QUERY, variables, TTL_FRANCHISE, cache).await?;
        Ok(data.media)
    }

    /// Runs a query through the response cache, deserializing the cached/fresh JSON
    /// body into `T`. `namespace` only affects the cache key's readable prefix.
    async fn cached_query<T: for<'de> Deserialize<'de>>(
        &self,
        namespace: &str,
        query: &str,
        variables: Value,
        ttl: Duration,
        cache: &Cache,
    ) -> Result<T, String> {
        let full_query = with_fragment(query);
        let key = cache_key(namespace, &full_query, &variables);
        let body = cache.get_or_fetch(&key, ttl, || self.execute_text(&full_query, variables.clone())).await?;
        serde_json::from_str(&body).map_err(|e| format!("AniList response did not match the expected shape: {e}"))
    }

    async fn execute_text(&self, query: &str, variables: Value) -> Result<String, String> {
        let data = self.execute(query, variables).await?;
        Ok(data.to_string())
    }

    /// Sends one GraphQL request and returns its `data` object, or a user-readable error.
    async fn execute(&self, query: &str, variables: Value) -> Result<Value, String> {
        let payload = serde_json::to_string(&json!({ "query": query, "variables": variables }))
            .map_err(|e| format!("cannot build the AniList request: {e}"))?;
        let text = self.post_with_retry(&payload).await?;
        let parsed: Value =
            serde_json::from_str(&text).map_err(|e| format!("AniList sent a response we could not read: {e}"))?;
        if let Some(message) = graphql_error_message(&parsed) {
            return Err(format!("AniList reported an error: {message}"));
        }
        parsed.get("data").cloned().ok_or_else(|| "AniList sent no data".to_string())
    }

    /// Posts `payload`; if AniList answers 429, waits for `Retry-After` (or 60s) once
    /// and retries exactly once more. Every attempt (including the retry) goes
    /// through the throttle, so a retry never bypasses the 30/min budget.
    async fn post_with_retry(&self, payload: &str) -> Result<String, String> {
        self.throttle.acquire().await;
        let response = self.post(payload).await?;
        if response.status().as_u16() != 429 {
            return read_body(response).await;
        }
        let retry_after = retry_after_seconds(&response);
        tokio::time::sleep(Duration::from_secs(retry_after)).await;
        self.throttle.acquire().await;
        let response = self.post(payload).await?;
        read_body(response).await
    }

    async fn post(&self, payload: &str) -> Result<reqwest::Response, String> {
        self.http
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|_| "AniList is not reachable.".to_string())
    }
}

fn retry_after_seconds(response: &reqwest::Response) -> u64 {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
}

async fn read_body(response: reqwest::Response) -> Result<String, String> {
    let status = response.status();
    let text = response.text().await.map_err(|_| "AniList sent an unreadable response.".to_string())?;
    if !status.is_success() {
        return Err(format!("AniList returned an error (HTTP {status})."));
    }
    Ok(text)
}

fn graphql_error_message(parsed: &Value) -> Option<&str> {
    parsed.get("errors")?.as_array()?.first()?.get("message")?.as_str()
}

/// Bridges the AniList client + cache into the `franchise::NodeFetcher` the walk uses,
/// so each hop is cached under the Franchise TTL.
pub struct CachingFetcher<'a> {
    pub client: &'a AniListClient,
    pub cache: &'a Cache,
}

#[async_trait]
impl NodeFetcher for CachingFetcher<'_> {
    async fn fetch(&self, id: i64) -> Result<RawFranchiseNode, String> {
        self.client.fetch_franchise_node(self.cache, id).await
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    #[test]
    fn extracts_the_first_graphql_error_message() {
        let parsed = json!({ "errors": [{ "message": "Too Many Requests." }] });
        assert_eq!(graphql_error_message(&parsed), Some("Too Many Requests."));
        assert_eq!(graphql_error_message(&json!({ "data": {} })), None);
    }

    /// Writes one raw HTTP response to `socket` after best-effort draining the request.
    async fn respond(mut socket: tokio::net::TcpStream, status_line: &str, headers: &str, body: &str) {
        let mut buf = [0u8; 4096];
        let _ = tokio::time::timeout(Duration::from_secs(2), socket.read(&mut buf)).await;
        let response = format!("{status_line}\r\nContent-Length: {}\r\n{headers}\r\n{body}", body.len());
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    }

    /// Starts a local server that answers each accepted connection in turn, then
    /// returns its `http://127.0.0.1:PORT` base URL.
    async fn mock_server(responses: Vec<(&'static str, &'static str, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for (status_line, headers, body) in responses {
                let (socket, _) = listener.accept().await.unwrap();
                respond(socket, status_line, headers, body).await;
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn retries_once_after_a_429_honoring_retry_after() {
        let ok_body = r#"{"data":{"ok":true}}"#;
        let endpoint = mock_server(vec![
            ("HTTP/1.1 429 Too Many Requests", "Retry-After: 0\r\n", "rate limited"),
            ("HTTP/1.1 200 OK", "Content-Type: application/json\r\n", ok_body),
        ])
        .await;

        let client = AniListClient::with_endpoint(endpoint);
        let data = client.execute("query { ok }", json!({})).await.unwrap();
        assert_eq!(data, json!({ "ok": true }));
    }

    #[tokio::test]
    async fn a_graphql_error_becomes_a_readable_message() {
        let error_body = r#"{"errors":[{"message":"Do not have permission to access this media"}]}"#;
        let endpoint = mock_server(vec![("HTTP/1.1 200 OK", "Content-Type: application/json\r\n", error_body)]).await;

        let client = AniListClient::with_endpoint(endpoint);
        let err = client.execute("query { Media(id: 1) { id } }", json!({})).await.unwrap_err();
        assert!(err.contains("Do not have permission"), "unexpected error: {err}");
    }

    /// SMOKE CHECK: hits the real AniList API for the Home hero, the same query
    /// `catalog_home` runs, and prints its title. Run explicitly with
    /// `cargo test -- --ignored`; never runs as part of the normal test suite.
    #[tokio::test]
    #[ignore]
    async fn smoke_test_fetches_the_real_home_hero() {
        let client = AniListClient::new();
        let cache = Cache::in_memory().unwrap();
        let (season, year) = crate::catalog::season::current_season();

        let data = client.fetch_home(&cache, season, year).await.expect("AniList request failed");
        let hero = data
            .trending
            .media
            .iter()
            .find(|m| m.banner_image.is_some() && m.description.is_some())
            .or_else(|| data.trending.media.first())
            .expect("AniList returned no trending Shows");

        let title = hero.title.english.as_deref().or(hero.title.romaji.as_deref()).unwrap_or("<no title>");
        println!("SMOKE hero title: {title}");
        assert!(!title.is_empty());
    }
}

