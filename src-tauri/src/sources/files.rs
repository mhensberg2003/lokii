//! Picks the file of one Episode inside a Release. A single-Episode Release plays its
//! largest video file; a Batch plays the file whose name holds the Episode number.

use crate::index::matching::{self, FileEpisode, ShowContext};
use crate::index::model::Coverage;
use crate::index::name;

const VIDEO_EXTENSIONS: &[&str] = &["mkv", "mp4", "m4v", "webm", "avi", "ts", "m2ts"];

/// Folder names that hold extras, not Episodes.
const EXTRA_FOLDERS: &[&str] = &["extra", "extras", "special", "specials", "bonus", "nc", "ncop", "nced", "menu", "sp"];

/// One file of a Release, as a Source lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseFile {
    /// The Source's own file ID (librqbit file index, TorBox file ID).
    pub id: u64,
    /// The path inside the Release, with `/` between folders.
    pub path: String,
    pub size: u64,
}

/// Returns the ID of the file that plays `episode`.
pub fn pick(files: &[ReleaseFile], coverage: Coverage, ctx: &ShowContext, episode: i64) -> Result<u64, String> {
    let all_videos: Vec<&ReleaseFile> = files.iter().filter(|f| is_video(&f.path)).collect();
    let main: Vec<&ReleaseFile> = all_videos.iter().copied().filter(|f| !in_extras_folder(&f.path)).collect();
    // A Show title with "Special" in it names the root folder; then nothing is an extra.
    let videos = if main.is_empty() { all_videos } else { main };
    let largest = || videos.iter().max_by_key(|f| f.size).map(|f| f.id);
    let not_found = || format!("This Release has no file for Episode {episode}.");

    if matches!(coverage, Coverage::Episode { .. }) || videos.len() == 1 {
        return largest().ok_or_else(not_found);
    }

    let matches: Vec<(&ReleaseFile, FileEpisode)> = videos
        .iter()
        .filter_map(|file| matching::file_episode(ctx, &name::parse(file_name(&file.path))).map(|m| (*file, m)))
        .filter(|(_, m)| m.episode == episode)
        .collect();
    // In a Batch that numbers all parts of a split season in one run, "- 05" is part 1.
    let prefer_direct = !ctx.is_later_part();
    matches
        .iter()
        .max_by_key(|(file, m)| (m.direct == prefer_direct, m.season_exact, file.size))
        .map(|(file, _)| file.id)
        .ok_or_else(not_found)
}

pub fn is_video(path: &str) -> bool {
    let lower = path.to_lowercase();
    VIDEO_EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// True for a file in an "Extras" or "NC" folder below the Release's root folder.
fn in_extras_folder(path: &str) -> bool {
    let mut folders: Vec<&str> = path.split('/').collect();
    folders.pop();
    // The root folder carries the Release name, which can hold any word.
    folders.iter().skip(1).any(|folder| {
        let lower = folder.to_lowercase();
        lower.split(|c: char| !c.is_alphanumeric()).any(|word| EXTRA_FOLDERS.contains(&word))
    })
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(id: u64, path: &str, size: u64) -> ReleaseFile {
        ReleaseFile { id, path: path.to_string(), size }
    }

    fn aot_s1() -> ShowContext {
        ShowContext::new(&["Attack on Titan", "Shingeki no Kyojin"], Some(25), 1, vec![], Some("TV"))
    }

    /// A Batch of both parts of AoT's Final Season, numbered S04E01 to S04E28.
    fn final_season_batch() -> Vec<ReleaseFile> {
        (1..=28)
            .map(|n| file(n, &format!("AoT Final/[Group] Attack on Titan - S04E{n:02} [1080p].mkv"), 1_000 + n))
            .collect()
    }

    #[test]
    fn a_single_episode_release_plays_its_largest_video() {
        let files = vec![
            file(0, "[SubsPlease] Frieren - 05 (1080p).mkv", 1_400),
            file(1, "readme.txt", 1),
            file(2, "sample.mkv", 20),
        ];
        assert_eq!(pick(&files, Coverage::Episode { episode: 5 }, &aot_s1(), 5), Ok(0));
    }

    #[test]
    fn a_batch_plays_the_file_with_the_episode_number() {
        let files: Vec<ReleaseFile> = (1..=25)
            .map(|n| {
                file(n, &format!("[MTBB] Shingeki no Kyojin/[MTBB] Shingeki no Kyojin - {n:02} [BD 1080p].mkv"), 1_000)
            })
            .chain([file(99, "[MTBB] Shingeki no Kyojin/Extras/[MTBB] Shingeki no Kyojin - NCOP 01.mkv", 5_000)])
            .collect();
        assert_eq!(pick(&files, Coverage::Show, &aot_s1(), 9), Ok(9));
        assert_eq!(pick(&files, Coverage::Show, &aot_s1(), 25), Ok(25));
    }

    #[test]
    fn a_show_title_with_an_extras_word_is_not_an_extras_folder() {
        let files = vec![
            file(1, "[Group] Bonus Track Club/[Group] Bonus Track Club - 01.mkv", 1_000),
            file(2, "[Group] Bonus Track Club/[Group] Bonus Track Club - 02.mkv", 1_000),
            file(3, "[Group] Bonus Track Club/Extras/[Group] Bonus Track Club - 02 Preview.mkv", 9_000),
        ];
        let ctx = ShowContext::new(&["Bonus Track Club"], Some(24), 1, vec![], Some("TV"));
        assert_eq!(pick(&files, Coverage::Show, &ctx, 2), Ok(2));
    }

    #[test]
    fn creditless_openings_and_other_seasons_are_not_episodes() {
        let files = vec![
            file(1, "AoT/S01/Attack on Titan - S01E03.mkv", 1_000),
            file(2, "AoT/S02/Attack on Titan - S02E03.mkv", 9_000),
            file(3, "AoT/Attack on Titan - NCOP1.mkv", 9_000),
        ];
        assert_eq!(pick(&files, Coverage::Show, &aot_s1(), 3), Ok(1));
        assert!(pick(&files, Coverage::Show, &aot_s1(), 4).is_err());
    }

    #[test]
    fn part_one_of_a_split_season_plays_the_low_numbers() {
        let part_one = ShowContext::new(
            &["Attack on Titan Final Season", "Shingeki no Kyojin: The Final Season"],
            Some(16),
            4,
            vec![59, 10],
            Some("TV"),
        );
        assert_eq!(pick(&final_season_batch(), Coverage::Show, &part_one, 7), Ok(7));
    }

    #[test]
    fn part_two_of_a_split_season_continues_the_numbering() {
        let part_two = ShowContext::new(
            &["Attack on Titan Final Season Part 2", "Shingeki no Kyojin: The Final Season Part 2"],
            Some(12),
            5,
            vec![75, 16],
            Some("TV"),
        );
        assert_eq!(pick(&final_season_batch(), Coverage::Show, &part_two, 1), Ok(17));
        assert_eq!(pick(&final_season_batch(), Coverage::Show, &part_two, 12), Ok(28));
    }

    #[test]
    fn a_batch_without_the_episode_is_an_error() {
        let files = vec![file(1, "Show - 01.mkv", 10), file(2, "Show - 02.mkv", 10)];
        let ctx = ShowContext::new(&["Show"], Some(12), 1, vec![], Some("TV"));
        assert_eq!(
            pick(&files, Coverage::Range { first: 1, last: 2 }, &ctx, 3),
            Err("This Release has no file for Episode 3.".into())
        );
    }
}
