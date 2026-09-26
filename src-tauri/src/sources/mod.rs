//! Sources: where the video bytes of a Release come from. TorBox when the user has
//! connected an account, else Local Torrent.

pub mod files;
pub mod local;
pub mod torbox;

use serde::Serialize;

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    #[serde(rename = "torbox")]
    TorBox,
    #[serde(rename = "local")]
    LocalTorrent,
}
