//! Episode titles and thumbnails from ani.zip (https://api.ani.zip), keyed by AniList
//! ID. ani.zip numbers the Episodes of each Show from 1 and takes its data from TVDB and
//! AniDB, so it has data for most Shows. AniList `streamingEpisodes` is only a fallback:
//! it is empty for most new Shows, and some sequels have the list of the first season.

use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;

use crate::store::Store;

const API: &str = "https://api.ani.zip/mappings";
/// The Show page waits for this call, so it must not wait long.
const TIMEOUT: Duration = Duration::from_secs(8);

/// The title and thumbnail of one Episode. Each can be missing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EpisodeDetail {
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// By Episode number.
pub type EpisodeDetails = HashMap<i64, EpisodeDetail>;

pub struct EpisodeClient {
    http: reqwest::Client,
    endpoint: String,
}

impl Default for EpisodeClient {
    fn default() -> Self {
        Self::with_endpoint(API)
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    episodes: HashMap<String, RawEpisode>,
}

#[derive(Deserialize)]
struct RawEpisode {
    title: Option<HashMap<String, Option<String>>>,
    image: Option<String>,
}

impl EpisodeClient {
    fn with_endpoint(endpoint: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(crate::index::USER_AGENT)
            .timeout(TIMEOUT)
            .build()
            .unwrap_or_default();
        Self { http, endpoint: endpoint.into() }
    }

    /// The Episode details of the Show, from the cache when younger than `ttl`. An error
    /// gives an empty map: the Episode list then shows numbers only.
    pub async fn fetch(&self, cache: &Store, anilist_id: i64, ttl: Duration) -> EpisodeDetails {
        let key = format!("anizip:{anilist_id}");
        let body = cache.get_or_fetch(&key, ttl, || self.request(anilist_id)).await;
        body.ok().map(|body| parse(&body)).unwrap_or_default()
    }

    async fn request(&self, anilist_id: i64) -> Result<String, String> {
        let url = format!("{}?anilist_id={anilist_id}", self.endpoint);
        let response = self.http.get(url).send().await.map_err(|e| e.to_string())?;
        // ani.zip has no entry for some Shows. Cache that as "no details".
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok("{}".into());
        }
        let response = response.error_for_status().map_err(|e| e.to_string())?;
        response.text().await.map_err(|e| e.to_string())
    }
}

/// Keeps the numbered Episodes. Specials have keys such as "S1" and are ignored.
fn parse(body: &str) -> EpisodeDetails {
    let Ok(response) = serde_json::from_str::<Response>(body) else {
        return EpisodeDetails::new();
    };
    response
        .episodes
        .into_iter()
        .filter_map(|(key, raw)| {
            let number = key.parse::<i64>().ok()?;
            let title = raw.title.as_ref().and_then(english_title);
            let thumbnail_url = raw.image.filter(|url| url.starts_with("https://"));
            Some((number, EpisodeDetail { title, thumbnail_url }))
        })
        .collect()
}

/// The English title, else the romanized one. A title such as "Episode 5" is no title.
fn english_title(titles: &HashMap<String, Option<String>>) -> Option<String> {
    ["en", "x-jat"]
        .iter()
        .filter_map(|lang| titles.get(*lang)?.as_deref())
        .map(str::trim)
        .find(|title| !title.is_empty() && !is_placeholder(title))
        .map(String::from)
}

fn is_placeholder(title: &str) -> bool {
    title.strip_prefix("Episode ").is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"episodes": {
        "1": {"title": {"en": "Beast Titan", "x-jat": "Kemono no Kyojin"}, "image": "https://artworks.thetvdb.com/1.jpg"},
        "2": {"title": {"en": "Episode 2", "x-jat": "Tadaima"}, "image": null},
        "3": {"title": {"en": "Episode 3"}},
        "S1": {"title": {"en": "Episode S1"}}
    }}"#;

    #[test]
    fn reads_titles_and_images_of_numbered_episodes() {
        let details = parse(BODY);
        assert_eq!(details.len(), 3);
        assert_eq!(details[&1].title.as_deref(), Some("Beast Titan"));
        assert_eq!(details[&1].thumbnail_url.as_deref(), Some("https://artworks.thetvdb.com/1.jpg"));
    }

    #[test]
    fn a_placeholder_title_falls_back_to_the_romanized_title_or_none() {
        let details = parse(BODY);
        assert_eq!(details[&2].title.as_deref(), Some("Tadaima"));
        assert_eq!(details[&2].thumbnail_url, None);
        assert_eq!(details[&3].title, None);
    }

    #[test]
    fn a_bad_body_gives_no_details() {
        assert!(parse("{}").is_empty());
        assert!(parse("not json").is_empty());
    }

    #[tokio::test]
    async fn a_network_error_gives_no_details() {
        let client = EpisodeClient::with_endpoint("http://127.0.0.1:9");
        let cache = Store::in_memory().unwrap();
        assert!(client.fetch(&cache, 1, Duration::from_secs(60)).await.is_empty());
    }

    #[tokio::test]
    #[ignore = "calls the live ani.zip API"]
    async fn smoke_test_the_second_season_has_its_own_episodes() {
        let cache = Store::in_memory().unwrap();
        // Attack on Titan Season 2: AniList streamingEpisodes has the Season 1 list.
        let details = EpisodeClient::default().fetch(&cache, 20958, Duration::from_secs(60)).await;
        assert_eq!(details[&1].title.as_deref(), Some("Beast Titan"));
    }
}
