//! AnimeTosho, the Index: every Release linked to an AniDB anime ID, and single
//! Releases by info hash. The JSON feed pages hold 75 Releases each, newest first.

use std::future::Future;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::store::Store;

/// The one place the AnimeTosho feed address lives.
pub const FEED_URL: &str = "https://feed.animetosho.xyz/json";
const PAGE_SIZE: usize = 75;
/// A long-running Show has 1,000+ Releases. The newest 450 hold the recent
/// per-Episode Releases and the popular Batches; SeaDex Best Releases are looked up
/// by hash when they are older. The feed answers about one page per second, whatever
/// the number of parallel requests, so more pages cost more wait on a cold cache.
const MAX_PAGES: u32 = 6;
const PAGES_AT_ONCE: u32 = 2;
const MAX_TRACKERS: usize = 8;

pub const TTL_AIRING: Duration = Duration::from_secs(30 * 60);
pub const TTL_FINISHED: Duration = Duration::from_secs(12 * 3600);
const TTL_HASH: Duration = Duration::from_secs(24 * 3600);

/// One feed entry, trimmed to what the Index uses. Also the cached form.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ToshoItem {
    pub id: i64,
    pub title: String,
    pub info_hash: Option<String>,
    pub magnet_uri: Option<String>,
    #[serde(default)]
    pub seeders: Option<u32>,
    #[serde(default)]
    pub leechers: Option<u32>,
    pub total_size: Option<u64>,
    pub num_files: Option<u32>,
    pub timestamp: Option<i64>,
    pub link: Option<String>,
}

pub struct AnimeTosho<'a> {
    pub http: &'a reqwest::Client,
    pub store: &'a Store,
}

impl AnimeTosho<'_> {
    /// Every Release AnimeTosho links to the AniDB anime `aid`, up to `MAX_PAGES` pages.
    pub async fn by_anidb(&self, aid: i64, ttl: Duration) -> Result<Vec<ToshoItem>, String> {
        let key = format!("animetosho:aid:{aid}");
        let body = self
            .store
            .get_or_fetch(&key, ttl, || async {
                let items = all_pages(|page| self.page(format!("{FEED_URL}?aid={aid}&page={page}"))).await?;
                serde_json::to_string(&items).map_err(|e| e.to_string())
            })
            .await?;
        serde_json::from_str(&body).map_err(|e| format!("cannot read cached AnimeTosho data: {e}"))
    }

    /// The Release with this info hash, if AnimeTosho has it.
    pub async fn by_hash(&self, info_hash: &str) -> Result<Option<ToshoItem>, String> {
        let hash = info_hash.to_lowercase();
        let key = format!("animetosho:hash:{hash}");
        let body = self
            .store
            .get_or_fetch(&key, TTL_HASH, || async {
                let items = self.page(format!("{FEED_URL}?q={hash}")).await?;
                let found = items
                    .into_iter()
                    .find(|item| item.info_hash.as_deref().is_some_and(|h| h.eq_ignore_ascii_case(&hash)));
                serde_json::to_string(&found).map_err(|e| e.to_string())
            })
            .await?;
        serde_json::from_str(&body).map_err(|e| format!("cannot read cached AnimeTosho data: {e}"))
    }

    async fn page(&self, url: String) -> Result<Vec<ToshoItem>, String> {
        let response = self.http.get(&url).send().await.map_err(|_| "AnimeTosho is not reachable.".to_string())?;
        if !response.status().is_success() {
            return Err(format!("AnimeTosho returned an error (HTTP {}).", response.status()));
        }
        let text = response.text().await.map_err(|_| "AnimeTosho sent an unreadable response.".to_string())?;
        parse_page(&text)
    }
}

pub fn parse_page(text: &str) -> Result<Vec<ToshoItem>, String> {
    let mut items: Vec<ToshoItem> =
        serde_json::from_str(text).map_err(|e| format!("AnimeTosho sent data we could not read: {e}"))?;
    for item in &mut items {
        item.info_hash = item.info_hash.as_ref().map(|h| h.to_lowercase());
        item.magnet_uri = item.magnet_uri.as_deref().map(trim_magnet);
    }
    Ok(items)
}

/// Fetches pages `PAGES_AT_ONCE` at a time until a short page or `MAX_PAGES`. A failed
/// first page is an error; a later failure keeps the pages that loaded.
async fn all_pages<F, Fut>(fetch: F) -> Result<Vec<ToshoItem>, String>
where
    F: Fn(u32) -> Fut,
    Fut: Future<Output = Result<Vec<ToshoItem>, String>>,
{
    let mut items: Vec<ToshoItem> = Vec::new();
    let mut next = 1;
    while next <= MAX_PAGES {
        let last = (next + PAGES_AT_ONCE - 1).min(MAX_PAGES);
        let pages = futures::future::join_all((next..=last).map(&fetch)).await;
        for (offset, page) in pages.into_iter().enumerate() {
            match page {
                Ok(page) if page.len() == PAGE_SIZE => items.extend(page),
                Ok(page) => {
                    items.extend(page);
                    return Ok(dedupe(items));
                }
                Err(err) if next == 1 && offset == 0 => return Err(err),
                Err(_) => return Ok(dedupe(items)),
            }
        }
        next = last + 1;
    }
    Ok(dedupe(items))
}

/// New Releases shift the pages while they load, so one Release can appear twice.
fn dedupe(mut items: Vec<ToshoItem>) -> Vec<ToshoItem> {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(item.id));
    items
}

/// Keeps the info hash, the name and the first `MAX_TRACKERS` trackers. Feed magnets
/// carry 40+ trackers, which would make the cache many megabytes larger.
fn trim_magnet(magnet: &str) -> String {
    let Some(query) = magnet.strip_prefix("magnet:?") else {
        return magnet.to_string();
    };
    let mut trackers = 0;
    let kept: Vec<&str> = query
        .split('&')
        .filter(|part| {
            if part.starts_with("tr=") {
                trackers += 1;
                trackers <= MAX_TRACKERS
            } else {
                true
            }
        })
        .collect();
    format!("magnet:?{}", kept.join("&"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: i64) -> ToshoItem {
        ToshoItem {
            id,
            title: format!("Release {id}"),
            info_hash: None,
            magnet_uri: None,
            seeders: None,
            leechers: None,
            total_size: None,
            num_files: None,
            timestamp: None,
            link: None,
        }
    }

    fn page(start: i64, len: usize) -> Vec<ToshoItem> {
        (start..start + len as i64).map(item).collect()
    }

    #[tokio::test]
    async fn stops_at_the_first_short_page() {
        let requested = std::sync::Mutex::new(Vec::new());
        let items = all_pages(|p| {
            requested.lock().unwrap().push(p);
            async move {
                Ok(match p {
                    1 => page(0, PAGE_SIZE),
                    2 => page(100, 10),
                    _ => page(200, PAGE_SIZE),
                })
            }
        })
        .await
        .unwrap();
        assert_eq!(items.len(), PAGE_SIZE + 10);
        assert_eq!(
            *requested.lock().unwrap(),
            (1..=PAGES_AT_ONCE).collect::<Vec<_>>(),
            "only the first group of pages loads"
        );
    }

    #[tokio::test]
    async fn stops_at_the_page_limit() {
        let items = all_pages(|p| async move { Ok(page(i64::from(p) * 1000, PAGE_SIZE)) }).await.unwrap();
        assert_eq!(items.len(), PAGE_SIZE * MAX_PAGES as usize);
    }

    #[tokio::test]
    async fn a_failed_first_page_is_an_error_and_a_later_one_is_not() {
        assert!(all_pages(|_| async { Err::<Vec<ToshoItem>, _>("down".to_string()) }).await.is_err());
        let items = all_pages(|p| async move {
            if p == 1 {
                Ok(page(0, PAGE_SIZE))
            } else {
                Err("down".to_string())
            }
        })
        .await
        .unwrap();
        assert_eq!(items.len(), PAGE_SIZE);
    }

    #[tokio::test]
    async fn duplicate_releases_across_pages_are_dropped() {
        let items =
            all_pages(|p| async move { Ok(if p == 1 { page(0, PAGE_SIZE) } else { page(70, 3) }) }).await.unwrap();
        assert_eq!(items.len(), PAGE_SIZE);
    }

    #[test]
    fn parse_page_lowercases_hashes_and_trims_magnets() {
        let trackers: String = (0..20).map(|i| format!("&tr=udp%3A%2F%2Ft{i}")).collect();
        let json = format!(
            r#"[{{"id": 1, "title": "x", "info_hash": "ABCDEF", "magnet_uri": "magnet:?xt=urn:btih:ABCDEF&dn=x{trackers}",
                "seeders": 3, "leechers": 1, "total_size": 10, "num_files": 1, "timestamp": 5, "link": "l", "extra": true}}]"#
        );
        let items = parse_page(&json).unwrap();
        assert_eq!(items[0].info_hash.as_deref(), Some("abcdef"));
        let magnet = items[0].magnet_uri.as_deref().unwrap();
        assert_eq!(magnet.matches("&tr=").count(), MAX_TRACKERS);
        assert!(magnet.starts_with("magnet:?xt=urn:btih:ABCDEF&dn=x&tr="));
    }

    #[test]
    fn parse_page_accepts_missing_numbers() {
        let items = parse_page(
            r#"[{"id": 2, "title": "y", "info_hash": null, "magnet_uri": null, "seeders": null,
            "leechers": null, "total_size": null, "num_files": null, "timestamp": null, "link": null}]"#,
        )
        .unwrap();
        assert_eq!(items, vec![ToshoItem { title: "y".into(), ..item(2) }]);
    }
}
