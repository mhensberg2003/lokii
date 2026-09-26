//! SeaDex (releases.moe): the Best Release list for a Show, keyed by AniList ID.

use std::time::Duration;

use serde::Deserialize;

use crate::store::Store;

const API_URL: &str = "https://releases.moe/api/collections/entries/records";
const TTL: Duration = Duration::from_secs(24 * 3600);

#[derive(Deserialize)]
struct EntryPage {
    #[serde(default)]
    items: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    expand: Option<Expand>,
}

#[derive(Deserialize)]
struct Expand {
    #[serde(default)]
    trs: Vec<Torrent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Torrent {
    #[serde(default)]
    info_hash: String,
    #[serde(default)]
    is_best: bool,
}

pub struct SeaDex<'a> {
    pub http: &'a reqwest::Client,
    pub store: &'a Store,
}

impl SeaDex<'_> {
    /// Lower-case info hashes of the Best Releases for the Show. Private-tracker
    /// Releases have no public hash and are left out.
    pub async fn best_hashes(&self, anilist_id: i64) -> Result<Vec<String>, String> {
        let key = format!("seadex:{anilist_id}");
        let body = self.store.get_or_fetch(&key, TTL, || self.fetch(anilist_id)).await?;
        parse_best_hashes(&body)
    }

    async fn fetch(&self, anilist_id: i64) -> Result<String, String> {
        let url = format!("{API_URL}?filter=alID%3D{anilist_id}&expand=trs");
        let response = self.http.get(&url).send().await.map_err(|_| "SeaDex is not reachable.".to_string())?;
        if !response.status().is_success() {
            return Err(format!("SeaDex returned an error (HTTP {}).", response.status()));
        }
        response.text().await.map_err(|_| "SeaDex sent an unreadable response.".to_string())
    }
}

fn parse_best_hashes(body: &str) -> Result<Vec<String>, String> {
    let page: EntryPage = serde_json::from_str(body).map_err(|e| format!("SeaDex sent data we could not read: {e}"))?;
    let mut hashes: Vec<String> = page
        .items
        .into_iter()
        .filter_map(|entry| entry.expand)
        .flat_map(|expand| expand.trs)
        .filter(|torrent| torrent.is_best && is_info_hash(&torrent.info_hash))
        .map(|torrent| torrent.info_hash.to_lowercase())
        .collect();
    hashes.sort();
    hashes.dedup();
    Ok(hashes)
}

fn is_info_hash(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_public_best_hashes_only() {
        let body = r#"{"items": [{"alID": 16498, "expand": {"trs": [
            {"infoHash": "FCD7DBC76CFECA08D3888CCC446E8859FC7C90DB", "isBest": true, "tracker": "Nyaa"},
            {"infoHash": "<redacted>", "isBest": true, "tracker": "AB"},
            {"infoHash": "e33fb393037459757c2447d39fd1cd256e557059", "isBest": false, "tracker": "Nyaa"},
            {"infoHash": "fcd7dbc76cfeca08d3888ccc446e8859fc7c90db", "isBest": true, "tracker": "Nyaa"}
        ]}}]}"#;
        assert_eq!(parse_best_hashes(body).unwrap(), vec!["fcd7dbc76cfeca08d3888ccc446e8859fc7c90db"]);
    }

    #[test]
    fn a_show_without_an_entry_has_no_best_release() {
        assert!(parse_best_hashes(r#"{"page": 1, "items": []}"#).unwrap().is_empty());
        assert!(parse_best_hashes(r#"{"items": [{"id": "x"}]}"#).unwrap().is_empty());
    }
}
