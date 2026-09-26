//! The Index data contract shared with the UI (`src/lib/releases.ts`).

use serde::{Deserialize, Serialize};

/// Which Episodes of the Show a Release contains.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Coverage {
    /// One Episode.
    Episode { episode: i64 },
    /// A Batch of Episodes `first..=last`.
    Range { first: i64, last: i64 },
    /// A Batch of the full Show.
    Show,
}

impl Coverage {
    pub fn contains(self, episode: i64) -> bool {
        match self {
            Coverage::Episode { episode: e } => e == episode,
            Coverage::Range { first, last } => (first..=last).contains(&episode),
            Coverage::Show => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    /// Lower-case hex BitTorrent v1 info hash. Identifies the Release.
    pub info_hash: String,
    pub title: String,
    pub group: Option<String>,
    pub resolution: Option<u32>,
    pub size_bytes: u64,
    pub seeders: u32,
    pub leechers: u32,
    pub file_count: u32,
    pub coverage: Coverage,
    /// SeaDex recommends this Release for the Show.
    pub is_best: bool,
    /// Unix seconds.
    pub published_at: i64,
    pub magnet: String,
    /// The Release page on AnimeTosho.
    pub link: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeReleases {
    pub show_id: i64,
    pub episode: i64,
    /// Best Release first, then by resolution and seeders.
    pub releases: Vec<Release>,
    /// The info hash of the Chosen Release, if any Release can play this Episode.
    pub chosen: Option<String>,
    /// True when the Chosen Release comes from the user's pick, not the automatic rule.
    pub picked_by_user: bool,
}
