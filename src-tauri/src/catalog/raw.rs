//! Deserialization shapes for AniList's GraphQL JSON. These mirror the `MediaFields`
//! fragment in `anilist.rs`; field names are camelCase, matching AniList's schema.

use serde::Deserialize;

use crate::catalog::model::Season;

#[derive(Deserialize, Debug, Clone, Default)]
pub struct RawTitle {
    pub romaji: Option<String>,
    pub english: Option<String>,
    pub native: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawCoverImage {
    pub extra_large: Option<String>,
    pub large: Option<String>,
    pub color: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct RawStudioConnection {
    pub nodes: Vec<RawStudioNode>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RawStudioNode {
    pub name: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawNextAiringEpisode {
    pub episode: i64,
    pub airing_at: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RawStreamingEpisode {
    pub title: Option<String>,
    pub thumbnail: Option<String>,
}

/// The AniList `MediaRelation` enum. `Unknown` absorbs any variant AniList adds later
/// (e.g. `CHARACTER`, `OTHER`) so a schema addition never breaks deserialization.
#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationType {
    Adaptation,
    Prequel,
    Sequel,
    Parent,
    SideStory,
    Character,
    Summary,
    Alternative,
    SpinOff,
    Other,
    Compilation,
    Contains,
    #[serde(other)]
    Unknown,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawRelationNode {
    pub id: i64,
    #[serde(default)]
    pub is_adult: bool,
    #[serde(rename = "type")]
    pub media_type: Option<String>,
    pub title: Option<RawTitle>,
    pub cover_image: Option<RawCoverImage>,
    pub banner_image: Option<String>,
    pub format: Option<String>,
    pub episodes: Option<i64>,
    pub season: Option<Season>,
    pub season_year: Option<i32>,
    pub average_score: Option<i32>,
    #[serde(default)]
    pub genres: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawRelationEdge {
    pub relation_type: RelationType,
    pub node: RawRelationNode,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct RawRelationConnection {
    #[serde(default)]
    pub edges: Vec<RawRelationEdge>,
}

/// A full `Media` object, as returned by the `MediaFields` fragment.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawMedia {
    pub id: i64,
    pub id_mal: Option<i64>,
    pub title: RawTitle,
    #[serde(default)]
    pub synonyms: Vec<String>,
    pub description: Option<String>,
    pub cover_image: RawCoverImage,
    pub banner_image: Option<String>,
    pub format: Option<String>,
    pub status: Option<String>,
    pub episodes: Option<i64>,
    pub duration: Option<i64>,
    pub season: Option<Season>,
    pub season_year: Option<i32>,
    pub average_score: Option<i32>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub is_adult: bool,
    #[serde(rename = "type")]
    pub media_type: String,
    pub studios: Option<RawStudioConnection>,
    pub next_airing_episode: Option<RawNextAiringEpisode>,
    pub relations: Option<RawRelationConnection>,
    pub streaming_episodes: Option<Vec<RawStreamingEpisode>>,
}

/// A `Page { media { ... } }` result, used by the Home and Browse aliased queries.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct RawMediaPage {
    #[serde(default)]
    pub media: Vec<RawMedia>,
}

/// The lean shape used while walking the Franchise chain: just enough to keep walking
/// and to build a `FranchiseEntry`.
#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RawFranchiseNode {
    pub id: i64,
    #[serde(default)]
    pub is_adult: bool,
    #[serde(rename = "type")]
    pub media_type: String,
    pub title: RawTitle,
    pub format: Option<String>,
    pub status: Option<String>,
    pub season: Option<Season>,
    pub season_year: Option<i32>,
    pub episodes: Option<i64>,
    pub relations: Option<RawRelationConnection>,
}

impl RawFranchiseNode {
    /// Builds the Franchise-walk starting node from an already-fetched full `Media`,
    /// so the requested Show's own relations do not need a second network round trip.
    pub fn from_media(media: &RawMedia) -> Self {
        Self {
            id: media.id,
            is_adult: media.is_adult,
            media_type: media.media_type.clone(),
            title: media.title.clone(),
            format: media.format.clone(),
            status: media.status.clone(),
            season: media.season,
            season_year: media.season_year,
            episodes: media.episodes,
            relations: media.relations.clone(),
        }
    }
}

pub const ANIME_TYPE: &str = "ANIME";
pub const CHAIN_FORMATS: &[&str] = &["TV", "TV_SHORT", "ONA"];
pub const SHORT_FORMATS: &[&str] = &["MOVIE", "OVA", "SPECIAL"];
