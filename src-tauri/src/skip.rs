//! Skip Segments from AniSkip (https://api.aniskip.com). AniSkip keys Episodes by
//! MyAnimeList ID, so Lokii reads the Show's MAL ID from AniList first. The player asks
//! after mpv knows the Episode's length, because AniSkip uses the length to choose the
//! times that fit this file.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::catalog::{self, CatalogState};
use crate::store::StoreState;

const API: &str = "https://api.aniskip.com/v2/skip-times";
/// Segments shorter than this are bad submissions.
const MIN_LENGTH: f64 = 5.0;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SegmentKind {
    Intro,
    Outro,
    Recap,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SkipSegment {
    pub kind: SegmentKind,
    /// Seconds from the start of the Episode.
    pub start: f64,
    pub end: f64,
}

pub struct SkipState {
    http: reqwest::Client,
    /// By (MAL ID, Episode, length in whole seconds). Empty lists are cached too.
    cache: Mutex<HashMap<(i64, i64, i64), Vec<SkipSegment>>>,
}

impl Default for SkipState {
    fn default() -> Self {
        Self { http: crate::index::http_client(), cache: Mutex::default() }
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    results: Vec<Found>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Found {
    interval: Interval,
    skip_type: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Interval {
    start_time: f64,
    end_time: f64,
}

fn url(mal_id: i64, episode: i64, length: i64) -> String {
    format!("{API}/{mal_id}/{episode}?types[]=op&types[]=ed&types[]=recap&episodeLength={length}")
}

fn kind(skip_type: &str) -> Option<SegmentKind> {
    match skip_type {
        "op" => Some(SegmentKind::Intro),
        "ed" => Some(SegmentKind::Outro),
        "recap" => Some(SegmentKind::Recap),
        _ => None,
    }
}

/// The first valid segment of each kind (AniSkip sorts the best fit first), in time order.
fn parse(body: &str, duration: f64) -> Vec<SkipSegment> {
    let Ok(response) = serde_json::from_str::<Response>(body) else {
        return Vec::new();
    };
    let mut segments: Vec<SkipSegment> = Vec::new();
    for result in response.results {
        let Some(kind) = kind(&result.skip_type) else { continue };
        let start = result.interval.start_time.max(0.0);
        let end = result.interval.end_time.min(duration);
        if end - start < MIN_LENGTH || segments.iter().any(|s| s.kind == kind) {
            continue;
        }
        segments.push(SkipSegment { kind, start, end });
    }
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    segments
}

async fn fetch(http: &reqwest::Client, mal_id: i64, episode: i64, duration: f64) -> Result<Vec<SkipSegment>, String> {
    let response = http
        .get(url(mal_id, episode, duration.round() as i64))
        .send()
        .await
        .map_err(|e| format!("AniSkip did not answer: {e}"))?;
    // AniSkip answers 404 with a JSON body when it has no times for the Episode.
    if !response.status().is_success() && response.status() != reqwest::StatusCode::NOT_FOUND {
        return Err(format!("AniSkip answered {}", response.status()));
    }
    let body = response.text().await.map_err(|e| e.to_string())?;
    Ok(parse(&body, duration))
}

/// The Skip Segments of the Episode, or an empty list when AniSkip has none.
#[tauri::command]
pub async fn skip_segments(
    app: AppHandle,
    catalog: State<'_, CatalogState>,
    store: State<'_, StoreState>,
    skip: State<'_, SkipState>,
    show_id: i64,
    episode: i64,
    duration: f64,
) -> Result<Vec<SkipSegment>, String> {
    if !duration.is_finite() || duration <= 0.0 {
        return Ok(Vec::new());
    }
    let show = catalog::show(&catalog, store.get(&app).await?, show_id).await?;
    let Some(mal_id) = show.id_mal else {
        return Ok(Vec::new());
    };
    let key = (mal_id, episode, duration.round() as i64);
    if let Some(cached) = skip.cache.lock().map_err(|_| "skip cache is poisoned")?.get(&key) {
        return Ok(cached.clone());
    }
    let segments = fetch(&skip.http, mal_id, episode, duration).await?;
    skip.cache.lock().map_err(|_| "skip cache is poisoned")?.insert(key, segments.clone());
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"found":true,"results":[
        {"interval":{"startTime":1342.795,"endTime":1430.616},"skipType":"ed","skipId":"a","episodeLength":1447.0},
        {"interval":{"startTime":128.406,"endTime":218.406},"skipType":"op","skipId":"b","episodeLength":1441.9},
        {"interval":{"startTime":47.3,"endTime":137.3},"skipType":"op","skipId":"c","episodeLength":1540.0},
        {"interval":{"startTime":0.0,"endTime":2.0},"skipType":"recap","skipId":"d","episodeLength":1447.0}
    ],"message":"Successfully found skip times","statusCode":200}"#;

    #[test]
    fn keeps_the_best_segment_of_each_kind_in_time_order() {
        let segments = parse(BODY, 1447.0);
        assert_eq!(
            segments,
            vec![
                SkipSegment { kind: SegmentKind::Intro, start: 128.406, end: 218.406 },
                SkipSegment { kind: SegmentKind::Outro, start: 1342.795, end: 1430.616 },
            ]
        );
    }

    #[test]
    fn a_segment_never_ends_after_the_episode() {
        let segments = parse(BODY, 1400.0);
        assert_eq!(segments[1].end, 1400.0);
    }

    #[test]
    fn no_times_is_an_empty_list() {
        let body = r#"{"found":false,"results":[],"message":"No skip times found","statusCode":404}"#;
        assert!(parse(body, 1440.0).is_empty());
        assert!(parse("not json", 1440.0).is_empty());
    }

    #[test]
    fn asks_for_intro_outro_and_recap_with_the_length() {
        assert_eq!(
            url(16498, 3, 1447),
            "https://api.aniskip.com/v2/skip-times/16498/3?types[]=op&types[]=ed&types[]=recap&episodeLength=1447"
        );
    }

    #[tokio::test]
    #[ignore = "calls AniSkip"]
    async fn live_skip_times() {
        let segments = fetch(&crate::index::http_client(), 16498, 1, 1447.0).await.unwrap();
        assert!(segments.iter().any(|s| s.kind == SegmentKind::Intro));
    }
}
