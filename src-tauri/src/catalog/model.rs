//! The catalog data contract shared with the UI (`src/lib/catalog.ts`), and the pure
//! mapping from AniList's raw JSON (`raw.rs`) into it. Field names are camelCase on
//! both sides via `#[serde(rename_all = "camelCase")]`.

use serde::{Deserialize, Serialize};

use crate::catalog::raw::{RawFranchiseNode, RawMedia, RawRelationEdge, RelationType, ANIME_TYPE, SHORT_FORMATS};
use crate::catalog::text::{clean_description, parse_streaming_title};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowCard {
    pub id: i64,
    pub title: String,
    pub cover_url: String,
    pub banner_url: Option<String>,
    pub color: Option<String>,
    pub format: Option<String>,
    pub episodes: Option<i64>,
    pub season: Option<Season>,
    pub season_year: Option<i32>,
    pub average_score: Option<i32>,
    pub genres: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowRow {
    pub id: String,
    pub title: String,
    pub shows: Vec<ShowCard>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeFeed {
    pub hero: ShowDetailsLite,
    pub rows: Vec<ShowRow>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowDetailsLite {
    #[serde(flatten)]
    pub card: ShowCard,
    pub description: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FranchiseEntry {
    pub id: i64,
    pub title: String,
    pub format: Option<String>,
    /// AniList status, for example "FINISHED" or "NOT_YET_RELEASED".
    pub status: Option<String>,
    pub season: Option<Season>,
    pub season_year: Option<i32>,
    pub episodes: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeInfo {
    pub number: i64,
    pub title: Option<String>,
    pub thumbnail_url: Option<String>,
    pub airing_at: Option<i64>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NextAiring {
    pub episode: i64,
    pub airing_at: i64,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShowDetails {
    #[serde(flatten)]
    pub lite: ShowDetailsLite,
    pub id_mal: Option<i64>,
    pub title_romaji: Option<String>,
    pub title_native: Option<String>,
    pub synonyms: Vec<String>,
    pub status: Option<String>,
    pub duration: Option<i64>,
    pub studios: Vec<String>,
    pub next_airing: Option<NextAiring>,
    pub franchise: Vec<FranchiseEntry>,
    pub episode_list: Vec<EpisodeInfo>,
    pub related: Vec<ShowCard>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BrowseFeed {
    pub genre: String,
    pub rows: Vec<ShowRow>,
}

/// `title` = english title, falling back to romaji, per the data contract.
fn pick_title(title: &crate::catalog::raw::RawTitle) -> String {
    title.english.clone().or_else(|| title.romaji.clone()).unwrap_or_default()
}

/// `coverUrl` = `coverImage.extraLarge`, falling back to `large`, per the data contract.
fn pick_cover_url(cover: &crate::catalog::raw::RawCoverImage) -> String {
    cover.extra_large.clone().or_else(|| cover.large.clone()).unwrap_or_default()
}

pub fn to_show_card(media: &RawMedia) -> ShowCard {
    ShowCard {
        id: media.id,
        title: pick_title(&media.title),
        cover_url: pick_cover_url(&media.cover_image),
        banner_url: media.banner_image.clone(),
        color: media.cover_image.color.clone(),
        format: media.format.clone(),
        episodes: media.episodes,
        season: media.season,
        season_year: media.season_year,
        average_score: media.average_score,
        genres: media.genres.clone(),
    }
}

pub fn to_show_details_lite(media: &RawMedia) -> ShowDetailsLite {
    ShowDetailsLite {
        card: to_show_card(media),
        description: media.description.as_deref().map(clean_description).unwrap_or_default(),
    }
}

pub fn to_franchise_entry(node: &RawFranchiseNode) -> FranchiseEntry {
    FranchiseEntry {
        id: node.id,
        title: pick_title(&node.title),
        format: node.format.clone(),
        status: node.status.clone(),
        season: node.season,
        season_year: node.season_year,
        episodes: node.episodes,
    }
}

/// Builds the numbered Episode list: 1..=N, where N is `episodes`, or (for a RELEASING
/// Show with no episode count yet) the number of the next airing Episode. Titles and
/// thumbnails come from `streamingEpisodes` when its title parses as `"Episode N - Title"`.
/// Only the next, not-yet-aired Episode carries `airingAt`.
pub fn build_episode_list(media: &RawMedia) -> Vec<EpisodeInfo> {
    let total = episode_count(media);
    let Some(total) = total else {
        return Vec::new();
    };

    let streaming: std::collections::HashMap<i64, &crate::catalog::raw::RawStreamingEpisode> = media
        .streaming_episodes
        .as_deref()
        .unwrap_or_default()
        .iter()
        .filter_map(|ep| {
            let title = ep.title.as_deref()?;
            let (number, _) = parse_streaming_title(title)?;
            Some((number as i64, ep))
        })
        .collect();

    (1..=total)
        .map(|number| {
            let matched = streaming.get(&number);
            let title = matched.and_then(|ep| ep.title.as_deref()).and_then(parse_streaming_title).map(|(_, t)| t);
            let thumbnail_url = matched.and_then(|ep| ep.thumbnail.clone());
            let airing_at =
                media.next_airing_episode.as_ref().filter(|next| next.episode == number).map(|next| next.airing_at);
            EpisodeInfo { number, title, thumbnail_url, airing_at }
        })
        .collect()
}

/// N for the Episode list: the known Episode count, or (only while RELEASING with an
/// unknown count) the number of the next airing Episode.
fn episode_count(media: &RawMedia) -> Option<i64> {
    if let Some(episodes) = media.episodes {
        return Some(episodes);
    }
    if media.status.as_deref() == Some("RELEASING") {
        return media.next_airing_episode.as_ref().map(|next| next.episode);
    }
    None
}

/// Builds the `related` list from a Show's own relations: SIDE_STORY, SPIN_OFF,
/// ALTERNATIVE, PARENT, SUMMARY entries, plus SEQUEL/PREQUEL entries that are a movie,
/// OVA or special (a full season sequel/prequel belongs to the Franchise instead).
/// Excludes Franchise members, non-anime entries and anything marked adult.
pub fn build_related(edges: &[RawRelationEdge], franchise_ids: &std::collections::HashSet<i64>) -> Vec<ShowCard> {
    edges
        .iter()
        .filter(|edge| is_related_candidate(edge))
        .filter(|edge| !franchise_ids.contains(&edge.node.id))
        .filter(|edge| edge.node.media_type.as_deref() == Some(ANIME_TYPE))
        .filter(|edge| !edge.node.is_adult)
        .map(|edge| ShowCard {
            id: edge.node.id,
            title: edge.node.title.as_ref().map(pick_title).unwrap_or_default(),
            cover_url: edge.node.cover_image.as_ref().map(pick_cover_url).unwrap_or_default(),
            banner_url: edge.node.banner_image.clone(),
            color: edge.node.cover_image.as_ref().and_then(|c| c.color.clone()),
            format: edge.node.format.clone(),
            episodes: edge.node.episodes,
            season: edge.node.season,
            season_year: edge.node.season_year,
            average_score: edge.node.average_score,
            genres: edge.node.genres.clone(),
        })
        .collect()
}

fn is_related_candidate(edge: &RawRelationEdge) -> bool {
    use RelationType::*;
    match edge.relation_type {
        SideStory | SpinOff | Alternative | Parent | Summary => true,
        Sequel | Prequel => edge.node.format.as_deref().is_some_and(|f| SHORT_FORMATS.contains(&f)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> RawMedia {
        let path = format!("{}/src/catalog/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
        let body = std::fs::read_to_string(path).expect("fixture file");
        let value: serde_json::Value = serde_json::from_str(&body).expect("valid json");
        serde_json::from_value(value).expect("matches RawMedia")
    }

    #[test]
    fn maps_title_and_cover_with_fallbacks() {
        let media = fixture("finished_show.json");
        let card = to_show_card(&media);
        assert_eq!(card.title, "Finished Show EN");
        assert_eq!(card.cover_url, "https://example.test/extra-large.jpg");
        assert_eq!(card.color, Some("#e4a15d".to_string()));
    }

    #[test]
    fn cleans_the_description() {
        let media = fixture("finished_show.json");
        let lite = to_show_details_lite(&media);
        assert_eq!(lite.description, "A finished show.\nIt has two lines.");
    }

    /// Guards the wire shape against `#[serde(flatten)]` mistakes: `ShowDetailsLite`
    /// must serialize as ShowCard's fields plus `description`, flat, matching the
    /// `ShowDetailsLite = ShowCard & { description: string }` intersection in catalog.ts.
    #[test]
    fn show_details_lite_serializes_flat_not_nested() {
        let media = fixture("finished_show.json");
        let lite = to_show_details_lite(&media);
        let value = serde_json::to_value(&lite).unwrap();
        let obj = value.as_object().unwrap();
        assert!(!obj.contains_key("card"), "ShowCard must be flattened, not nested: {obj:?}");
        for key in [
            "id",
            "title",
            "coverUrl",
            "bannerUrl",
            "color",
            "format",
            "episodes",
            "season",
            "seasonYear",
            "averageScore",
            "genres",
            "description",
        ] {
            assert!(obj.contains_key(key), "missing key {key} in {obj:?}");
        }
    }

    #[test]
    fn finished_show_episode_list_has_no_airing_at() {
        let media = fixture("finished_show.json");
        let episodes = build_episode_list(&media);
        assert_eq!(episodes.len(), 3);
        assert!(episodes.iter().all(|e| e.airing_at.is_none()));
        assert_eq!(episodes[0].title, Some("The Beginning".to_string()));
        assert_eq!(episodes[2].title, None); // no matching streamingEpisodes entry
    }

    #[test]
    fn airing_show_with_null_episodes_uses_next_airing_episode() {
        let media = fixture("airing_show.json");
        let episodes = build_episode_list(&media);
        // nextAiringEpisode.episode = 6 in the fixture.
        assert_eq!(episodes.len(), 6);
        assert!(episodes[..5].iter().all(|e| e.airing_at.is_none()));
        assert_eq!(episodes[5].number, 6);
        assert_eq!(episodes[5].airing_at, Some(1_800_000_000));
    }

    #[test]
    fn related_excludes_franchise_members_and_adult_and_non_anime() {
        let media = fixture("finished_show.json");
        let franchise_ids = std::collections::HashSet::from([media.id]);
        let related = build_related(&media.relations.as_ref().unwrap().edges, &franchise_ids);
        // The fixture has: a SIDE_STORY (kept), a SEQUEL that is TV (excluded: franchise
        // member territory, not a short format), a SEQUEL movie (kept), an adult SIDE_STORY
        // (excluded), and a manga ADAPTATION (excluded: not ANIME).
        let ids: Vec<i64> = related.iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![201, 203]);
    }
}
