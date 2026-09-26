//! Reads a Release name ("[MTBB] Shingeki no Kyojin - 09 (BD 1080p) [8A96AE8A].mkv")
//! into the parts the Index needs, with the anitomy-ng filename parser.

use anitomy_ng::{ElementKind, Options};

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "avi", "webm", "ts", "m2ts"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedName {
    pub group: Option<String>,
    pub title: Option<String>,
    /// Every season number in the name. More than one means a multi-season Batch.
    pub seasons: Vec<u32>,
    /// Every episode number in the name: one for a single Episode, two for a range.
    pub episodes: Vec<f64>,
    /// Vertical resolution: 1080 for "1080p", "1920x1080" and "1080i".
    pub resolution: Option<u32>,
    /// The anime type in the name, upper case: "OVA", "SPECIAL", "MOVIE", …
    pub kind: Option<String>,
    pub has_volume: bool,
    /// "Part 2" of a split season, when the parser finds it outside the title.
    pub part: Option<u32>,
    /// False when the name ends in a non-video extension (".zip" patches, ".ass" files).
    pub is_video: bool,
}

pub fn parse(name: &str) -> ParsedName {
    let mut parsed = ParsedName { is_video: true, ..ParsedName::default() };
    for element in anitomy_ng::parse(name, Options::default()) {
        let value = element.value.trim();
        match element.kind {
            ElementKind::ReleaseGroup if parsed.group.is_none() => parsed.group = Some(value.to_string()),
            ElementKind::Title if parsed.title.is_none() => parsed.title = Some(value.to_string()),
            ElementKind::Season => parsed.seasons.extend(value.parse::<u32>().ok()),
            ElementKind::Episode => parsed.episodes.extend(value.parse::<f64>().ok()),
            ElementKind::VideoResolution if parsed.resolution.is_none() => parsed.resolution = resolution(value),
            ElementKind::Type if parsed.kind.is_none() => parsed.kind = Some(value.to_uppercase()),
            ElementKind::Volume => parsed.has_volume = true,
            ElementKind::Part if parsed.part.is_none() => parsed.part = value.parse().ok(),
            ElementKind::FileExtension => {
                parsed.is_video = VIDEO_EXTENSIONS.contains(&value.to_lowercase().as_str());
            }
            _ => {}
        }
    }
    parsed
}

/// "1080p" → 1080, "1920x1080" → 1080, "1080i" → 1080, "4K" → 2160.
fn resolution(value: &str) -> Option<u32> {
    let lower = value.to_lowercase();
    if lower == "4k" || lower == "uhd" {
        return Some(2160);
    }
    let height = match lower.split_once('x') {
        Some((_, height)) => height,
        None => lower.trim_end_matches(['p', 'i']),
    };
    height.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_single_episode_release() {
        let parsed = parse("[MTBB] Shingeki no Kyojin - 09 (BD 1080p) [8A96AE8A].mkv");
        assert_eq!(parsed.group.as_deref(), Some("MTBB"));
        assert_eq!(parsed.title.as_deref(), Some("Shingeki no Kyojin"));
        assert_eq!(parsed.episodes, vec![9.0]);
        assert_eq!(parsed.resolution, Some(1080));
        assert!(parsed.is_video);
    }

    #[test]
    fn reads_a_season_batch() {
        let parsed = parse("[GapMoe] Attack on Titan S02 REPACK (2013) (1080p BluRay HEVC Dual-Audio FLAC 2.0)");
        assert_eq!(parsed.seasons, vec![2]);
        assert!(parsed.episodes.is_empty());
    }

    #[test]
    fn reads_an_episode_range() {
        let parsed = parse("[Erai-raws] Shingeki no Kyojin - The Final Season - 01 ~ 06 [1080p][Multiple Subtitle]");
        assert_eq!(parsed.episodes, vec![1.0, 6.0]);
        assert_eq!(parsed.title.as_deref(), Some("Shingeki no Kyojin - The Final Season"));
    }

    #[test]
    fn reads_the_type_and_the_pixel_size() {
        let parsed = parse(
            "[Cat66] Shingeki no Kyojin - 0.5B OAD5 - A Choice With No Regrets (Part 2) (BD 1920x1080 x265 FLAC)",
        );
        assert_eq!(parsed.kind.as_deref(), Some("OAD"));
        assert_eq!(parsed.resolution, Some(1080));
    }

    #[test]
    fn reads_the_part_of_a_split_season() {
        assert_eq!(parse("Attack.on.Titan.S04.P2.1080p.Blu-Ray.10-Bit.Dual-Audio.TrueHD.x265-iAHD").part, Some(2));
        assert_eq!(parse("[MTBB] Shingeki no Kyojin - 09 (BD 1080p) [8A96AE8A].mkv").part, None);
    }

    #[test]
    fn marks_non_video_files() {
        assert!(!parse("[gg]_Shingeki_no_Kyojin_-_03v3_[4F3140DA]_patch.zip").is_video);
    }

    #[test]
    fn resolution_forms() {
        assert_eq!(resolution("720p"), Some(720));
        assert_eq!(resolution("1080i"), Some(1080));
        assert_eq!(resolution("1280x720"), Some(720));
        assert_eq!(resolution("4K"), Some(2160));
        assert_eq!(resolution("HD"), None);
    }
}
