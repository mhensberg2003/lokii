//! Decides which Episodes of a Show a Release contains. AnimeTosho links Releases to
//! AniDB IDs loosely: the list for season 1 also holds sequels, OVAs and spin-offs.
//! These rules drop Releases that name another Show, season or type.

use crate::catalog::model::ShowDetails;
use crate::index::model::Coverage;
use crate::index::name::ParsedName;

/// Words a Release title can add to the Show title without naming a different Show.
const FILLER_WORDS: &[&str] = &[
    "the",
    "season",
    "tv",
    "complete",
    "batch",
    "series",
    "uncensored",
    "bd",
    "bdrip",
    "web",
    "dub",
    "dubbed",
    "sub",
    "subbed",
    "english",
    "eng",
    "dual",
    "audio",
    "multi",
    "remux",
];

/// Anime types that are not regular Episodes, unless the Show itself has that format.
const EXTRA_KINDS: &[&str] = &[
    "OVA", "OAD", "ONA", "SPECIAL", "SPECIALS", "SP", "OP", "ED", "NCOP", "NCED", "PV", "PREVIEW", "TRAILER", "CM",
    "MENU", "MOVIE", "FILM",
];

#[derive(Debug, Clone)]
pub struct ShowContext {
    /// Normalized titles, longest first.
    titles: Vec<String>,
    /// Known or aired Episode count.
    episodes: Option<i64>,
    /// 1-based position in the Franchise; 1 for a Show without one.
    season_index: usize,
    /// The "Part N" in the Show's titles, if any.
    part: Option<u32>,
    /// Numbers to subtract from absolute Episode numbers ("Attack on Titan - 81").
    offsets: Vec<i64>,
    format: Option<String>,
}

impl ShowContext {
    pub fn new(
        titles: &[&str],
        episodes: Option<i64>,
        season_index: usize,
        offsets: Vec<i64>,
        format: Option<&str>,
    ) -> Self {
        let mut titles: Vec<String> = titles.iter().map(|t| normalize(t)).filter(|t| t.len() >= 3).collect();
        titles.sort_by_key(|t| std::cmp::Reverse(t.len()));
        titles.dedup();
        let part = titles.iter().find_map(|title| title_part(title));
        Self { titles, episodes, season_index, part, offsets, format: format.map(str::to_string) }
    }

    pub fn from_show(show: &ShowDetails) -> Self {
        let card = &show.lite.card;
        let mut titles: Vec<&str> = vec![card.title.as_str()];
        titles.extend(show.title_romaji.as_deref());
        titles.extend(show.synonyms.iter().map(String::as_str));

        let aired = show.next_airing.as_ref().map(|next| next.episode - 1);
        let episodes = card.episodes.or(aired).or(Some(show.episode_list.len() as i64).filter(|n| *n > 0));

        let position = show.franchise.iter().position(|entry| entry.id == card.id);
        let earlier = position.map(|p| &show.franchise[..p]).unwrap_or_default();
        let mut offsets = Vec::new();
        if let Some(total) = earlier.iter().map(|e| e.episodes).sum::<Option<i64>>().filter(|t| *t > 0) {
            offsets.push(total);
        }
        if let Some(previous) = earlier.last().and_then(|e| e.episodes) {
            offsets.push(previous);
        }
        Self::new(&titles, episodes, position.map_or(1, |p| p + 1), offsets, card.format.as_deref())
    }

    fn is_single_episode(&self) -> bool {
        self.episodes == Some(1) || self.format.as_deref() == Some("MOVIE")
    }
}

/// Which Episodes of the Show the Release contains, or `None` when it is not a Release
/// of this Show.
pub fn coverage(ctx: &ShowContext, parsed: &ParsedName, file_count: u32) -> Option<Coverage> {
    if !parsed.is_video || parsed.has_volume || parsed.seasons.len() > 1 {
        return None;
    }
    if !kind_matches(ctx, parsed.kind.as_deref())
        || !season_matches(ctx, parsed.seasons.first().copied())
        || !part_matches(ctx, parsed.part)
    {
        return None;
    }
    if !title_matches(ctx, parsed.title.as_deref()?, parsed.kind.as_deref()) {
        return None;
    }
    episode_coverage(ctx, &parsed.episodes, file_count)
}

/// The coverage for a Release that SeaDex vouches for: the parsed Episodes when the
/// name has them, else the full Show.
pub fn trusted_coverage(ctx: &ShowContext, parsed: &ParsedName, file_count: u32) -> Coverage {
    episode_coverage(ctx, &parsed.episodes, file_count.max(2)).unwrap_or(Coverage::Show)
}

fn episode_coverage(ctx: &ShowContext, episodes: &[f64], file_count: u32) -> Option<Coverage> {
    match episodes {
        [] => (ctx.is_single_episode() || file_count > 1).then_some(Coverage::Show),
        [single] => map_episode(ctx, *single).map(|episode| Coverage::Episode { episode }),
        [first, last, ..] => {
            let (first, last) = (map_episode(ctx, *first)?, map_episode(ctx, *last)?);
            if first >= last {
                None
            } else if first == 1 && Some(last) == ctx.episodes {
                Some(Coverage::Show)
            } else {
                Some(Coverage::Range { first, last })
            }
        }
    }
}

/// Maps a number from a Release name to an Episode of this Show. Tries absolute
/// numbering (continuing from earlier seasons) when the number is past the last Episode.
fn map_episode(ctx: &ShowContext, number: f64) -> Option<i64> {
    if number.fract() != 0.0 || number < 1.0 {
        return None;
    }
    let number = number as i64;
    let Some(total) = ctx.episodes else {
        return Some(number);
    };
    if number <= total {
        return Some(number);
    }
    ctx.offsets.iter().map(|offset| number - offset).find(|episode| (1..=total).contains(episode))
}

fn kind_matches(ctx: &ShowContext, kind: Option<&str>) -> bool {
    let Some(kind) = kind else { return true };
    if !EXTRA_KINDS.contains(&kind) {
        return true;
    }
    match ctx.format.as_deref() {
        Some("MOVIE") => matches!(kind, "MOVIE" | "FILM"),
        Some("OVA") | Some("SPECIAL") => {
            matches!(kind, "OVA" | "OAD" | "SPECIAL" | "SPECIALS" | "SP")
        }
        Some("ONA") => kind == "ONA",
        _ => false,
    }
}

/// A season number must agree with the Show's place in its Franchise: "S2" is never
/// the first season, and "S1" is never a sequel.
fn season_matches(ctx: &ShowContext, season: Option<u32>) -> bool {
    match season {
        None => true,
        Some(season) if ctx.season_index <= 1 => season == 1,
        Some(season) => season >= 2,
    }
}

/// A "Part 2" Release belongs to the Show whose title says "Part 2". A Show without a
/// part in its title is part 1.
fn part_matches(ctx: &ShowContext, part: Option<u32>) -> bool {
    part.is_none_or(|part| part == ctx.part.unwrap_or(1))
}

/// The number after "part" in a normalized title.
fn title_part(title: &str) -> Option<u32> {
    let mut words = title.split(' ');
    while let Some(word) = words.next() {
        if word == "part" {
            return words.next()?.parse().ok();
        }
    }
    None
}

/// True when the Release title is one of the Show's titles plus only filler words,
/// or a whole-word part of one of the Show's titles ("Frieren" for "Sousou no Frieren").
/// The type word ("Movie") also counts as filler, because `kind_matches` passed.
fn title_matches(ctx: &ShowContext, title: &str, kind: Option<&str>) -> bool {
    let title = normalize(title);
    if title.len() < 3 {
        return false;
    }
    let padded = format!(" {title} ");
    if ctx.titles.iter().any(|show_title| format!(" {show_title} ").contains(&padded)) {
        return true;
    }

    let mut rest = padded;
    let mut found = false;
    for show_title in &ctx.titles {
        let needle = format!(" {show_title} ");
        while let Some(start) = rest.find(&needle) {
            rest.replace_range(start..start + needle.len(), " ");
            found = true;
        }
    }
    let kind = kind.map(str::to_lowercase);
    found && rest.split_whitespace().all(|word| FILLER_WORDS.contains(&word) || kind.as_deref() == Some(word))
}

/// Lower case, with every run of non-alphanumeric characters as one space.
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::name::parse;

    fn aot_s1() -> ShowContext {
        ShowContext::new(&["Attack on Titan", "Shingeki no Kyojin", "AoT"], Some(25), 1, vec![], Some("TV"))
    }

    fn final_season() -> ShowContext {
        ShowContext::new(
            &["Attack on Titan Final Season", "Shingeki no Kyojin: The Final Season"],
            Some(16),
            4,
            vec![59, 22],
            Some("TV"),
        )
    }

    fn cover(ctx: &ShowContext, name: &str, files: u32) -> Option<Coverage> {
        coverage(ctx, &parse(name), files)
    }

    #[test]
    fn single_episodes_match_by_number() {
        let ctx = aot_s1();
        assert_eq!(
            cover(&ctx, "[MTBB] Shingeki no Kyojin - 09 (BD 1080p) [8A96AE8A].mkv", 1),
            Some(Coverage::Episode { episode: 9 })
        );
        assert_eq!(
            cover(&ctx, "[ZeroBuild] Attack on Titan - S01E06 (BD 1080p HEVC 10-bit OPUS) [Dual Audio]", 1),
            Some(Coverage::Episode { episode: 6 })
        );
    }

    #[test]
    fn season_batches_cover_the_show() {
        let ctx = aot_s1();
        assert_eq!(cover(&ctx, "[MTBB] Shingeki no Kyojin S1 (BD 1080p) | Attack on Titan", 25), Some(Coverage::Show));
        assert_eq!(
            cover(&ctx, "[Prof] Attack on Titan (2013) S01 [1080p x265 HEVC 10bit BluRay Dual Audio AAC]", 25),
            Some(Coverage::Show)
        );
    }

    #[test]
    fn sequels_and_spin_offs_are_dropped() {
        let ctx = aot_s1();
        assert_eq!(
            cover(&ctx, "[GapMoe] Attack on Titan S02 REPACK (2013) (1080p BluRay HEVC Dual-Audio FLAC 2.0)", 12),
            None
        );
        assert_eq!(
            cover(
                &ctx,
                "[Erai-raws] Shingeki no Kyojin - The Final Season - 15 [1080p HEVC][Multiple Subtitle].mkv",
                1
            ),
            None
        );
        assert_eq!(
            cover(&ctx, "[CBM] Attack on Titan - Junior High 1-12 Complete (Dual Audio) [BDRip 1080p AVC 8bit]", 12),
            None
        );
        assert_eq!(cover(&ctx, "[YuiSubs] Shingeki no Kyojin - Chronicle (NVENC H.265 1080p)", 1), None);
    }

    #[test]
    fn extras_and_bad_numbers_are_dropped() {
        let ctx = aot_s1();
        assert_eq!(
            cover(&ctx, "[AoTs] Shingeki no Kyojin - Lost Girls OVA - 01 (DVD 1024x576 x264 10bit FLAC)", 1),
            None
        );
        assert_eq!(cover(&ctx, "[iPUNISHER] Shingeki no Kyojin - SPECIAL 01 [BDRip 1080p x264]", 1), None);
        assert_eq!(cover(&ctx, "[HorribleSubs] Shingeki no Kyojin - 13.5 [1080p].mkv", 1), None);
        assert_eq!(cover(&ctx, "[Metaljerk] Attack on Titan - 81 [1080p] [CR] (English Dub)", 1), None);
        assert_eq!(cover(&ctx, "[Flax] Shingeki no Kyojin - Volume 1 (BD 720p AAC)", 4), None);
        assert_eq!(cover(&ctx, "[gg]_Shingeki_no_Kyojin_-_03v3_[4F3140DA]_patch.zip", 1), None);
    }

    #[test]
    fn a_single_file_without_a_number_is_not_a_batch() {
        assert_eq!(cover(&aot_s1(), "[Group] Attack on Titan (1080p)", 1), None);
    }

    #[test]
    fn absolute_numbers_map_to_the_sequel_episode() {
        let ctx = final_season();
        assert_eq!(
            cover(&ctx, "[Metaljerk] Attack on Titan - 60 [1080p] [CR] (English Dub)", 1),
            Some(Coverage::Episode { episode: 1 })
        );
        assert_eq!(
            cover(&ctx, "[ASW] Shingeki no Kyojin - The Final Season - 23 [1080p]", 1),
            Some(Coverage::Episode { episode: 1 })
        );
        assert_eq!(
            cover(&ctx, "[Erai-raws] Shingeki no Kyojin - The Final Season - 15 [1080p].mkv", 1),
            Some(Coverage::Episode { episode: 15 })
        );
        assert_eq!(cover(&ctx, "[Group] Attack on Titan S01 (1080p)", 25), None);
    }

    #[test]
    fn batches_of_another_part_are_dropped() {
        let part_one = final_season();
        let part_two = ShowContext::new(
            &["Attack on Titan Final Season Part 2", "Shingeki no Kyojin: The Final Season Part 2"],
            Some(12),
            5,
            vec![75, 16],
            Some("TV"),
        );
        let ember =
            "[EMBER] Attack on Titan (2022) (Season 4 | Part 02) [BDRip] [1080p Dual Audio HEVC 10 bits] (Batch)";
        assert_eq!(cover(&part_one, ember, 12), None);
        assert_eq!(cover(&part_two, ember, 12), Some(Coverage::Show));
        assert_eq!(
            cover(&part_one, "Attack.on.Titan.S04.P2.1080p.Blu-Ray.10-Bit.Dual-Audio.TrueHD.x265-iAHD", 12),
            None
        );
        assert_eq!(
            cover(&part_one, "[ASW] Shingeki no Kyojin - The Final Season Part 3 - 02 [1080p HEVC x265 10Bit][AAC]", 1),
            None
        );
    }

    #[test]
    fn episode_ranges_become_range_or_show_batches() {
        let ctx = final_season();
        assert_eq!(
            cover(&ctx, "[Erai-raws] Shingeki no Kyojin - The Final Season - 01 ~ 06 [1080p][Multiple Subtitle]", 6),
            Some(Coverage::Range { first: 1, last: 6 })
        );
        assert_eq!(
            cover(&ctx, "[Group] Shingeki no Kyojin - The Final Season - 01-16 [1080p]", 16),
            Some(Coverage::Show)
        );
    }

    #[test]
    fn a_short_title_inside_a_show_title_matches() {
        let ctx =
            ShowContext::new(&["Frieren: Beyond Journey's End", "Sousou no Frieren"], Some(28), 1, vec![], Some("TV"));
        assert_eq!(
            cover(&ctx, "[SubsPlease] Frieren - 05 (1080p) [ABCD1234].mkv", 1),
            Some(Coverage::Episode { episode: 5 })
        );
        assert_eq!(
            cover(&ctx, "[SubsPlease] Sousou no Frieren - 05 (1080p) [ABCD1234].mkv", 1),
            Some(Coverage::Episode { episode: 5 })
        );
    }

    #[test]
    fn a_movie_is_one_file_without_a_number() {
        let ctx = ShowContext::new(&["Your Name.", "Kimi no Na wa."], Some(1), 1, vec![], Some("MOVIE"));
        assert_eq!(cover(&ctx, "[Group] Kimi no Na wa. (BD 1080p HEVC) [Dual Audio]", 1), Some(Coverage::Show));
        assert_eq!(cover(&ctx, "[Group] Kimi no Na wa. Movie (BD 1080p)", 1), Some(Coverage::Show));
    }

    #[test]
    fn trusted_releases_default_to_the_full_show() {
        let ctx = aot_s1();
        assert_eq!(trusted_coverage(&ctx, &parse("[MTBB] Something Else S4 (BD 1080p)"), 1), Coverage::Show);
    }

    #[test]
    fn normalize_joins_words_with_single_spaces() {
        assert_eq!(normalize("Frieren: Beyond Journey's End"), "frieren beyond journey s end");
        assert_eq!(normalize("  Shingeki_no_Kyojin  "), "shingeki no kyojin");
    }
}
