//! Builds the One Pace mapping from its sources: the Episode Guide sheet (Arcs, Episodes
//! and the CRC32 of each file), the Episode Descriptions sheet (titles), and the "[One
//! Pace]" Releases on Nyaa. A file belongs to an Episode when the CRC32 in its name
//! matches the Episode Guide.
//!
//! Which Release plays an Episode: the Episode Guide's current cut first; among the
//! Releases that hold that same file, a single-Episode Release before a Batch; then the
//! newest Release.

use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap};

use regex::Regex;

use super::model::{arc_id, normalize, Cut, EpisodeFile, Mapping, NyaaRelease, PaceEpisode, ReleaseFileInfo, StoryArc};

/// Everything the mapping is built from. Network access lives in `sources.rs`.
pub struct Inputs {
    /// Episode Guide tabs in sheet order: (tab name, CSV). Includes "Arc Overview".
    pub guide_tabs: Vec<(String, String)>,
    /// Episode Descriptions sheet, "Episodes" tab as CSV.
    pub episode_descriptions: String,
    /// Episode Descriptions sheet, "Arcs" tab as CSV.
    pub arc_descriptions: String,
    /// (Arc title, poster URL).
    pub posters: Vec<(String, String)>,
    pub releases: Vec<ListedRelease>,
    pub built_at: i64,
}

/// A "[One Pace]" Release from the Nyaa search.
pub struct ListedRelease {
    pub release: NyaaRelease,
    /// The file list. `None` for a Release named like its one file.
    pub files: Option<Vec<ReleaseFileInfo>>,
}

const OVERVIEW_TAB: &str = "arc overview";
/// The Descriptions sheet spells some Arcs differently than the Episode Guide.
const ALIASES: &[(&str, &str)] = &[("arabasta", "alabasta")];

pub fn build(inputs: Inputs) -> Result<Mapping, String> {
    let texts = Texts::parse(&inputs.episode_descriptions, &inputs.arc_descriptions)?;
    let posters: HashMap<String, String> = inputs.posters.into_iter().map(|(t, url)| (arc_key(&t), url)).collect();
    let files = FileIndex::new(&inputs.releases);

    let mut overview = HashMap::new();
    let mut arcs = Vec::new();
    for (name, csv) in &inputs.guide_tabs {
        let rows = parse_csv(csv)?;
        if normalize(name) == OVERVIEW_TAB {
            overview = work_in_progress(&rows);
            continue;
        }
        let Some(rows) = guide_rows(&rows) else { continue };
        let title = name.trim().to_string();
        let episodes = episodes(&title, rows, &texts, &files);
        if episodes.is_empty() {
            continue;
        }
        let (saga, description) = texts.arc(&title);
        arcs.push(StoryArc {
            id: arc_id(&title),
            saga,
            description,
            poster_url: posters.get(&arc_key(&title)).cloned(),
            work_in_progress: false,
            episodes,
            title,
        });
    }
    for arc in &mut arcs {
        arc.work_in_progress = overview.get(&arc_key(&arc.title)).copied().unwrap_or(false);
    }
    if arcs.is_empty() {
        return Err("the Episode Guide has no Arcs".to_string());
    }

    let used: std::collections::HashSet<&str> =
        arcs.iter().flat_map(|a| &a.episodes).flat_map(|e| &e.files).map(|f| f.info_hash.as_str()).collect();
    let releases = inputs
        .releases
        .iter()
        .filter(|r| used.contains(r.release.info_hash.as_str()))
        .map(|r| (r.release.info_hash.clone(), r.release.clone()))
        .collect();
    let file_lists = inputs.releases.iter().filter_map(|r| Some((r.release.nyaa_id, r.files.clone()?))).collect();
    Ok(Mapping { built_at: inputs.built_at, arcs, releases, file_lists })
}

fn arc_key(title: &str) -> String {
    let key = normalize(title);
    ALIASES.iter().find(|(from, _)| *from == key).map(|(_, to)| to.to_string()).unwrap_or(key)
}

pub fn parse_csv(text: &str) -> Result<Vec<Vec<String>>, String> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes())
        .records()
        .map(|record| {
            record.map(|r| r.iter().map(|field| field.trim().to_string()).collect()).map_err(|e| e.to_string())
        })
        .collect()
}

/// Arc titles marked "(WIP)" in the "Arc Overview" tab.
fn work_in_progress(rows: &[Vec<String>]) -> HashMap<String, bool> {
    rows.iter()
        .filter_map(|row| row.get(1))
        .map(|label| {
            let title = label.split('(').next().unwrap_or(label);
            (arc_key(title), label.contains("(WIP)"))
        })
        .collect()
}

/// One Episode row of an Episode Guide tab.
struct GuideRow<'a> {
    label: &'a str,
    chapters: &'a str,
    anime_episodes: &'a str,
    released: &'a str,
    length: &'a str,
    crc32: &'a str,
    crc32_extended: &'a str,
}

/// The Episode rows of an Arc tab, or `None` for a tab without a CRC32 column.
fn guide_rows(rows: &[Vec<String>]) -> Option<Vec<GuideRow<'_>>> {
    let header = rows.first()?;
    let column = |name: &str| header.iter().position(|h| h.eq_ignore_ascii_case(name));
    let crc = column("MKV CRC32")?;
    let extended = column("MKV CRC32 (Extended)");
    fn field(row: &[String], i: Option<usize>) -> &str {
        i.and_then(|i| row.get(i)).map_or("", String::as_str)
    }
    let (chapters, anime, released, length) =
        (column("Chapters"), column("Episodes"), column("Release Date"), column("Length"));
    Some(
        rows[1..]
            .iter()
            .map(|row| GuideRow {
                label: field(row, Some(1)),
                chapters: field(row, chapters),
                anime_episodes: field(row, anime),
                released: field(row, released),
                length: field(row, length),
                crc32: field(row, Some(crc)),
                crc32_extended: field(row, extended),
            })
            .filter(|row| !row.label.is_empty() && is_crc32(row.crc32))
            .collect(),
    )
}

fn is_crc32(text: &str) -> bool {
    text.len() == 8 && text.chars().all(|c| c.is_ascii_hexdigit())
}

fn episodes(arc: &str, rows: Vec<GuideRow>, texts: &Texts, files: &FileIndex) -> Vec<PaceEpisode> {
    let mut episodes: Vec<PaceEpisode> = Vec::new();
    for row in rows {
        let number_in_label = label_number(arc, row.label);
        // "Skypiea 25 (G8)" is another version of "Skypiea 25", not a new Episode.
        if row.label.ends_with(')') {
            if let Some(previous) = episodes.last_mut() {
                previous.files.extend(files.by_crc(row.crc32, Cut::Alternate));
                continue;
            }
        }
        let mut found = files.by_crc(row.crc32, Cut::Standard);
        if is_crc32(row.crc32_extended) {
            found.extend(files.by_crc(row.crc32_extended, Cut::Extended));
        }
        if found.is_empty() {
            found = files.by_label(row.label);
        }
        let (title, description) = texts.episode(arc, number_in_label);
        episodes.push(PaceEpisode {
            number: episodes.len() as i64 + 1,
            label: row.label.to_string(),
            title,
            description,
            chapters: row.chapters.to_string(),
            anime_episodes: row.anime_episodes.split_whitespace().collect::<Vec<_>>().join(" "),
            released: Some(row.released.to_string()).filter(|r| r.chars().next().is_some_and(|c| c.is_ascii_digit())),
            length: parse_length(row.length),
            chosen: None,
            files: found,
        });
    }
    for episode in &mut episodes {
        episode.chosen = choose(&episode.files, files);
    }
    episodes
}

/// "Wano 02" → 2, "Long Ring Long Land 00" → 0, "The Trials of Koby-Meppo" → 1.
fn label_number(arc: &str, label: &str) -> i64 {
    let rest = label.get(arc.len()..).filter(|_| label.to_lowercase().starts_with(&arc.to_lowercase())).unwrap_or("");
    rest.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()).and_then(|n| n.parse().ok()).unwrap_or(1)
}

/// "22:48" or "1:02:03" in seconds.
fn parse_length(text: &str) -> Option<i64> {
    let parts: Vec<i64> = text.split(':').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    (parts.len() >= 2).then(|| parts.iter().fold(0, |total, part| total * 60 + part))
}

fn choose(candidates: &[EpisodeFile], files: &FileIndex) -> Option<String> {
    candidates
        .iter()
        .filter_map(|file| Some((file, files.releases.get(file.info_hash.as_str())?)))
        .min_by_key(|(file, release)| (file.cut, release.file_count > 1, Reverse(release.published_at)))
        .map(|(file, _)| file.info_hash.clone())
}

/// Episode files in the listed Releases, by CRC32 and by Episode name.
struct FileIndex<'a> {
    releases: HashMap<&'a str, &'a NyaaRelease>,
    by_crc: HashMap<String, Vec<(&'a str, String)>>,
    by_label: HashMap<String, Vec<(&'a str, String)>>,
}

impl<'a> FileIndex<'a> {
    fn new(listed: &'a [ListedRelease]) -> Self {
        let crc = Regex::new(r"\[([0-9A-Fa-f]{8})\]").expect("valid regex");
        // "[One Pace][679-680] Punk Hazard 13 [720p][316829437].mkv" → "Punk Hazard 13"
        let label = Regex::new(r"^\[One Pace\](?:\[[^\]]*\])?\s*([^\[]+?)\s*\[").expect("valid regex");
        let mut index = Self { releases: HashMap::new(), by_crc: HashMap::new(), by_label: HashMap::new() };
        for listed in listed {
            let release = &listed.release;
            index.releases.insert(&release.info_hash, release);
            let single = [ReleaseFileInfo { path: release.title.clone(), size: release.size_bytes }];
            let files = listed.files.as_deref().unwrap_or(&single);
            for file in files {
                let name = file.path.rsplit('/').next().unwrap_or(&file.path);
                for found in crc.captures_iter(name) {
                    let entry = index.by_crc.entry(found[1].to_uppercase()).or_default();
                    entry.push((&release.info_hash, file.path.clone()));
                }
                if let Some(found) = label.captures(name) {
                    let entry = index.by_label.entry(normalize(&found[1])).or_default();
                    entry.push((&release.info_hash, file.path.clone()));
                }
            }
        }
        index
    }

    fn by_crc(&self, crc32: &str, cut: Cut) -> Vec<EpisodeFile> {
        Self::files(self.by_crc.get(&crc32.to_uppercase()), cut)
    }

    fn by_label(&self, label: &str) -> Vec<EpisodeFile> {
        Self::files(self.by_label.get(&normalize(label)), Cut::NameOnly)
    }

    fn files(hits: Option<&Vec<(&str, String)>>, cut: Cut) -> Vec<EpisodeFile> {
        hits.into_iter()
            .flatten()
            .map(|(hash, path)| EpisodeFile { info_hash: hash.to_string(), path: path.clone(), cut })
            .collect()
    }
}

/// Titles and descriptions from the Episode Descriptions sheet.
struct Texts {
    /// (Arc key, Episode number in the label) → (title, description).
    episodes: HashMap<(String, i64), (Option<String>, Option<String>)>,
    /// Arc key → (saga, description).
    arcs: BTreeMap<String, (Option<String>, Option<String>)>,
}

impl Texts {
    fn parse(episodes_csv: &str, arcs_csv: &str) -> Result<Self, String> {
        let text = |row: &Vec<String>, i: usize| row.get(i).filter(|s| !s.is_empty()).cloned();
        let episodes = parse_csv(episodes_csv)?
            .iter()
            .filter_map(|row| {
                let number = row.get(1)?.parse().ok()?;
                Some(((arc_key(row.first()?), number), (text(row, 2), text(row, 3))))
            })
            .collect();
        let arcs = parse_csv(arcs_csv)?
            .iter()
            .filter_map(|row| Some((arc_key(row.get(2)?), (text(row, 0), text(row, 3)))))
            .filter(|(key, _)| !key.is_empty())
            .collect();
        Ok(Self { episodes, arcs })
    }

    fn arc(&self, title: &str) -> (Option<String>, Option<String>) {
        self.arc_key(title).and_then(|key| self.arcs.get(&key).cloned()).unwrap_or_default()
    }

    fn episode(&self, arc: &str, number: i64) -> (Option<String>, Option<String>) {
        self.arc_key(arc).and_then(|key| self.episodes.get(&(key, number)).cloned()).unwrap_or_default()
    }

    /// The sheet's key for an Arc: the same title, or a longer one that ends with it
    /// ("If You Could Go Anywhere... The Adventures of the Straw Hats").
    fn arc_key(&self, title: &str) -> Option<String> {
        let key = arc_key(title);
        if self.arcs.contains_key(&key) {
            return Some(key);
        }
        self.arcs.keys().find(|k| k.ends_with(&format!(" {key}"))).cloned().or(Some(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::onepace::model::tests::release;

    const OVERVIEW: &str = "No.,Arcs,Manga Chapters\n1,Punk Hazard (TBR),654 - 699\n2,Wano (WIP),909 - 1057\n";
    const WANO: &str = "\
,One Pace Episode,Chapters,Episodes,Release Date,Length,MKV CRC32,MKV CRC32 (Extended),Length (Extended)
,Wano 01,Ch. 909-910,Ep. 890-891,2025.04.04,22:48,F15AFDE0,,
,Wano 02,Ch. 909-911,\"Ep. 892,\n894\",2025.05.17,29:06,8538EDC4,6F26FDAB,32:01
,Wano 63 Forward,Ch. 1014-1058,Ep. 1034-1085,To Be Released,,,,
";
    const PUNK_HAZARD: &str = "\
Punk Hazard,One Pace Episode,Chapters,Episodes,Release Date,Length,MKV CRC32
,Punk Hazard 13,Ch. 679-680,Ep. 598,2013.06.01,28:00,964FB36B
,Punk Hazard 13 (Alt),Ch. 679-680,Ep. 598,2013.06.01,28:00,AAAAAAAA
";
    const EPISODES: &str = "arc_title,arc_part,title_en,description_en\nWano,1,The Land of Wano,Samurai.\n";
    const ARCS: &str = "saga_title,part,title_en,description_en\nFour Emperors,35,Wano,Luffy fights Kaido.\n";

    fn listed(hash: &str, id: u64, title: &str, published_at: i64, files: Option<&[&str]>) -> ListedRelease {
        let files = files.map(|paths| paths.iter().map(|p| ReleaseFileInfo { path: p.to_string(), size: 1 }).collect());
        let count = files.as_ref().map_or(1, |f: &Vec<ReleaseFileInfo>| f.len() as u32);
        ListedRelease { release: release(hash, id, title, published_at, count), files }
    }

    fn inputs(releases: Vec<ListedRelease>) -> Inputs {
        Inputs {
            guide_tabs: vec![
                ("Arc Overview".into(), OVERVIEW.into()),
                ("Punk Hazard".into(), PUNK_HAZARD.into()),
                ("Wano".into(), WANO.into()),
            ],
            episode_descriptions: EPISODES.into(),
            arc_descriptions: ARCS.into(),
            posters: vec![("Wano".into(), "https://example.test/wano.png".into())],
            releases,
            built_at: 7,
        }
    }

    fn wano_batch() -> ListedRelease {
        let files: &[&str] = &[
            "[One Pace][909-924] Wano Act 1/[One Pace][909-910] Wano 01 [1080p][F15AFDE0].mkv",
            "[One Pace][909-924] Wano Act 1/[One Pace][909-911] Wano 02 [1080p][8538EDC4].mkv",
        ];
        listed("batch", 2, "[One Pace][909-924] Wano Act 1", 50, Some(files))
    }

    #[test]
    fn arcs_come_from_the_guide_with_titles_and_status() {
        let mapping = build(inputs(vec![wano_batch()])).unwrap();
        let titles: Vec<_> = mapping.arcs.iter().map(|a| (a.title.as_str(), a.work_in_progress)).collect();
        assert_eq!(titles, [("Punk Hazard", false), ("Wano", true)]);
        let wano = &mapping.arcs[1];
        assert_eq!(wano.saga.as_deref(), Some("Four Emperors"));
        assert_eq!(wano.poster_url.as_deref(), Some("https://example.test/wano.png"));
        let first = &wano.episodes[0];
        assert_eq!((first.number, first.title.as_deref(), first.length), (1, Some("The Land of Wano"), Some(1368)));
        assert_eq!(wano.episodes[1].anime_episodes, "Ep. 892, 894");
        assert_eq!(wano.episodes.len(), 2, "rows without a CRC32 are not Episodes yet");
        assert_eq!(mapping.built_at, 7);
    }

    #[test]
    fn a_single_file_wins_over_a_newer_batch_with_the_same_file() {
        let single = listed("single", 3, "[One Pace][909-910] Wano 01 [1080p][F15AFDE0].mkv", 10, None);
        let mapping = build(inputs(vec![wano_batch(), single])).unwrap();
        let wano = &mapping.arcs[1];
        assert_eq!(wano.episodes[0].chosen.as_deref(), Some("single"));
        assert_eq!(wano.episodes[0].files.len(), 2);
        assert_eq!(wano.episodes[1].chosen.as_deref(), Some("batch"));
        assert_eq!(
            wano.episodes[1].files[0].path,
            "[One Pace][909-924] Wano Act 1/[One Pace][909-911] Wano 02 [1080p][8538EDC4].mkv"
        );
        assert_eq!(mapping.file_lists.len(), 1);
    }

    #[test]
    fn the_newest_release_of_the_same_kind_wins() {
        let old = listed("old", 4, "[One Pace][909-910] Wano 01 [1080p][F15AFDE0].mkv", 10, None);
        let new = listed("new", 5, "[One Pace][909-910] Wano 01 [1080p][F15AFDE0].mkv", 20, None);
        let mapping = build(inputs(vec![old, new])).unwrap();
        assert_eq!(mapping.arcs[1].episodes[0].chosen.as_deref(), Some("new"));
    }

    #[test]
    fn the_standard_cut_wins_over_a_newer_extended_cut() {
        let standard = listed("std", 6, "[One Pace][909-911] Wano 02 [1080p][8538EDC4].mkv", 10, None);
        let extended = listed("ext", 7, "[One Pace][909-911] Wano 02 Extended [1080p][6F26FDAB].mkv", 20, None);
        let mapping = build(inputs(vec![standard, extended])).unwrap();
        let episode = &mapping.arcs[1].episodes[1];
        assert_eq!(episode.chosen.as_deref(), Some("std"));
        assert_eq!(episode.files.iter().map(|f| f.cut).collect::<Vec<_>>(), [Cut::Standard, Cut::Extended]);
    }

    #[test]
    fn a_wrong_crc32_falls_back_to_the_episode_name_and_alternates_join_the_episode() {
        let path = "[One Pace][654-699] Punk Hazard [720p]/[One Pace][679-680] Punk Hazard 13 [720p][316829437].mkv";
        let batch = listed("ph", 8, "[One Pace][654-699] Punk Hazard [720p]", 10, Some(&[path]));
        let alt = listed("alt", 9, "[One Pace][679-680] Punk Hazard 13 (Alt) [720p][AAAAAAAA].mkv", 11, None);
        let mapping = build(inputs(vec![batch, alt])).unwrap();
        let punk_hazard = &mapping.arcs[0];
        assert_eq!(punk_hazard.episodes.len(), 1);
        let cuts: Vec<_> = punk_hazard.episodes[0].files.iter().map(|f| (f.info_hash.as_str(), f.cut)).collect();
        assert_eq!(cuts, [("ph", Cut::NameOnly), ("alt", Cut::Alternate)]);
        assert_eq!(punk_hazard.episodes[0].chosen.as_deref(), Some("alt"));
    }

    #[test]
    fn releases_without_episode_files_are_left_out() {
        let other = listed("other", 10, "[One Pace] Chapter 1 [480p][12345678].mkv", 10, None);
        let mapping = build(inputs(vec![other])).unwrap();
        assert!(mapping.releases.is_empty());
        assert!(mapping.arcs[1].episodes[0].chosen.is_none());
    }

    #[test]
    fn labels_and_lengths_parse() {
        assert_eq!(label_number("Long Ring Long Land", "Long Ring Long Land 00"), 0);
        assert_eq!(label_number("Skypiea", "Skypiea 25 (G8)"), 25);
        assert_eq!(label_number("The Trials of Koby-Meppo", "The Trials of Koby-Meppo"), 1);
        assert_eq!(parse_length("1:02:03"), Some(3723));
        assert_eq!(parse_length(""), None);
    }
}
