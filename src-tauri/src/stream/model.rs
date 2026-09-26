//! The Stream data contract shared with the UI (`src/lib/streams.ts`).

use serde::Serialize;

use crate::sources::SourceKind;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Phase {
    /// Finding the Chosen Release or asking the Source for it. `step` is for the UI.
    Preparing {
        step: String,
    },
    /// TorBox downloads the Release before it can stream it. `progress` is 0 to 1.
    Downloading {
        progress: f64,
    },
    /// The URL plays. A Local Torrent keeps downloading while it plays.
    Ready,
    Failed {
        message: String,
    },
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StreamView {
    pub id: String,
    pub show_id: i64,
    pub episode: i64,
    pub show_title: String,
    pub source: SourceKind,
    pub release_title: Option<String>,
    pub release_group: Option<String>,
    /// The file of the Episode inside the Release.
    pub file_name: Option<String>,
    pub phase: Phase,
    /// The HTTP URL mpv plays, once the phase is `ready`.
    pub url: Option<String>,
    /// File size in bytes; 0 until known.
    pub size: u64,
    pub downloaded: u64,
    /// Bytes per second.
    pub speed: u64,
    pub peers: u32,
    pub watched: bool,
    /// Unix seconds.
    pub started_at: i64,
}

impl StreamView {
    pub fn new(id: String, show_id: i64, episode: i64, source: SourceKind, started_at: i64) -> Self {
        Self {
            id,
            show_id,
            episode,
            show_title: String::new(),
            source,
            release_title: None,
            release_group: None,
            file_name: None,
            phase: Phase::Preparing { step: "Finding the release".to_string() },
            url: None,
            size: 0,
            downloaded: 0,
            speed: 0,
            peers: 0,
            watched: false,
            started_at,
        }
    }

    pub fn is_failed(&self) -> bool {
        matches!(self.phase, Phase::Failed { .. })
    }
}
