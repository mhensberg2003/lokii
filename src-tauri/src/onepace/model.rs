//! The One Pace mapping: every Arc, its Episodes, and the Nyaa Releases that hold each
//! Episode's file. `build.rs` makes it; this file turns it into the catalog and Index
//! types, so the rest of the app sees each Arc as one Show in one Franchise.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::catalog::model::{EpisodeInfo, FranchiseEntry, ShowCard, ShowDetails, ShowDetailsLite};
use crate::index::model::{Coverage, Release};
use crate::index::name;

/// Arc IDs start here, far above AniList IDs, so both share one Show ID space.
pub const ARC_ID_BASE: i64 = 1_000_000_000;
const ARC_ID_SPAN: u32 = 100_000_000;
const TITLE_PREFIX: &str = "One Pace: ";
const GENRES: &[&str] = &["Action", "Adventure", "Comedy", "Fantasy"];

pub fn is_arc(id: i64) -> bool {
    (ARC_ID_BASE..ARC_ID_BASE + i64::from(ARC_ID_SPAN)).contains(&id)
}

/// The Show ID of an Arc, from its title. It stays the same when a refresh adds or
/// moves Arcs, so Watch Progress keeps pointing at the right Arc.
pub fn arc_id(title: &str) -> i64 {
    // FNV-1a: the same value in every build, unlike `DefaultHasher`.
    let hash =
        normalize(title).bytes().fold(0x811c_9dc5_u32, |hash, byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193));
    ARC_ID_BASE + i64::from(hash % ARC_ID_SPAN)
}

/// Lower-case words split by single spaces: "Buggy's Crew" → "buggy s crew".
pub fn normalize(text: &str) -> String {
    text.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    /// Unix seconds.
    pub built_at: i64,
    /// In story order.
    pub arcs: Vec<StoryArc>,
    /// Every Release that holds an Episode file, by info hash.
    pub releases: BTreeMap<String, NyaaRelease>,
    /// File lists by Nyaa ID, for Releases not named like their one file. A torrent
    /// never changes, so a refresh reads only the lists of new Releases.
    pub file_lists: BTreeMap<u64, Vec<ReleaseFileInfo>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StoryArc {
    pub id: i64,
    pub title: String,
    pub saga: Option<String>,
    pub description: Option<String>,
    pub poster_url: Option<String>,
    /// The One Pace team still adds Episodes to this Arc.
    pub work_in_progress: bool,
    pub episodes: Vec<PaceEpisode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PaceEpisode {
    /// 1-based position in the Arc.
    pub number: i64,
    /// The Episode Guide name, for example "Wano 02".
    pub label: String,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Manga chapters, for example "Ch. 909-911".
    pub chapters: String,
    /// Anime episodes, for example "Ep. 892-894".
    pub anime_episodes: String,
    pub released: Option<String>,
    /// Seconds.
    pub length: Option<i64>,
    /// A still of the anime Episode that the Episode starts from.
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    pub files: Vec<EpisodeFile>,
    /// The info hash of the Release to play when the user did not pick one.
    pub chosen: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeFile {
    pub info_hash: String,
    /// The path inside the Release, with `/` between folders.
    pub path: String,
    pub cut: Cut,
}

/// Which version of the Episode a file holds.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Cut {
    /// The CRC32 matches the Episode Guide.
    Standard,
    /// The CRC32 matches the Episode Guide's extended cut.
    Extended,
    /// Another version the Episode Guide lists, for example "Skypiea 25 (G8)".
    Alternate,
    /// The name matches ("Punk Hazard 13"), but the CRC32 does not. An older version,
    /// or a file with a wrong CRC32 in its name.
    NameOnly,
}

impl Cut {
    fn group(self) -> &'static str {
        match self {
            Cut::Standard | Cut::NameOnly => "One Pace",
            Cut::Extended => "One Pace Extended",
            Cut::Alternate => "One Pace Alternate",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NyaaRelease {
    pub info_hash: String,
    pub nyaa_id: u64,
    pub title: String,
    pub magnet: String,
    pub size_bytes: u64,
    pub seeders: u32,
    pub leechers: u32,
    /// Unix seconds.
    pub published_at: i64,
    pub file_count: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFileInfo {
    pub path: String,
    pub size: u64,
}

impl Mapping {
    pub fn arc(&self, id: i64) -> Option<&StoryArc> {
        self.arcs.iter().find(|arc| arc.id == id)
    }

    /// Episodes that have at least one Release.
    pub fn playable_episodes(&self) -> usize {
        self.arcs.iter().flat_map(|arc| &arc.episodes).filter(|e| e.chosen.is_some()).count()
    }

    pub fn show(&self, id: i64) -> Option<ShowDetails> {
        let arc = self.arc(id)?;
        let lengths: Vec<i64> = arc.episodes.iter().filter_map(|e| e.length).collect();
        let duration = (!lengths.is_empty()).then(|| (lengths.iter().sum::<i64>() / lengths.len() as i64 + 30) / 60);
        Some(ShowDetails {
            lite: ShowDetailsLite { card: card(arc), description: arc.description.clone().unwrap_or_default() },
            id_mal: None,
            title_romaji: None,
            title_native: None,
            synonyms: Vec::new(),
            status: Some(status(arc).to_string()),
            duration,
            studios: vec!["One Pace".to_string()],
            next_airing: None,
            franchise: self.arcs.iter().map(franchise_entry).collect(),
            episode_list: arc
                .episodes
                .iter()
                .map(|e| EpisodeInfo {
                    number: e.number,
                    title: Some(e.title.clone().unwrap_or_else(|| e.label.clone())),
                    thumbnail_url: e.thumbnail_url.clone(),
                    airing_at: None,
                })
                .collect(),
            related: Vec::new(),
        })
    }

    /// The Releases of one Episode. The mapping's choice is the Best Release.
    pub fn releases(&self, id: i64, episode: i64) -> Vec<Release> {
        let Some(arc) = self.arc(id) else { return Vec::new() };
        let Some(ep) = arc.episodes.iter().find(|e| e.number == episode) else { return Vec::new() };
        let mut seen = std::collections::HashSet::new();
        ep.files
            .iter()
            .filter(|file| seen.insert(file.info_hash.as_str()))
            .filter_map(|file| {
                let release = self.releases.get(&file.info_hash)?;
                let resolution = name::parse(&release.title).resolution.or_else(|| name::parse(&file.path).resolution);
                Some(Release {
                    info_hash: release.info_hash.clone(),
                    title: release.title.clone(),
                    group: Some(file.cut.group().to_string()),
                    resolution,
                    size_bytes: release.size_bytes,
                    seeders: release.seeders,
                    leechers: release.leechers,
                    file_count: release.file_count,
                    coverage: coverage(arc, &release.info_hash),
                    is_best: ep.chosen.as_deref() == Some(release.info_hash.as_str()),
                    published_at: release.published_at,
                    magnet: release.magnet.clone(),
                    link: format!("https://nyaa.si/view/{}", release.nyaa_id),
                    file_path: Some(file.path.clone()),
                })
            })
            .collect()
    }

    /// Gives each Episode without a thumbnail the one it had in `older`.
    pub fn keep_thumbnails(&mut self, older: &Mapping) {
        let known: HashMap<&str, &String> = older
            .arcs
            .iter()
            .flat_map(|arc| &arc.episodes)
            .filter_map(|e| Some((e.label.as_str(), e.thumbnail_url.as_ref()?)))
            .collect();
        for episode in self.arcs.iter_mut().flat_map(|arc| &mut arc.episodes) {
            if episode.thumbnail_url.is_none() {
                episode.thumbnail_url = known.get(episode.label.as_str()).map(|url| url.to_string());
            }
        }
    }

    /// Arcs whose words start with every word of the query ("one pace", "wano").
    pub fn search(&self, query: &str) -> Vec<ShowCard> {
        let words: Vec<String> = normalize(query).split(' ').filter(|w| !w.is_empty()).map(str::to_string).collect();
        if words.is_empty() {
            return Vec::new();
        }
        self.arcs
            .iter()
            .filter(|arc| {
                let haystack =
                    normalize(&format!("one pace onepace {} {}", arc.title, arc.saga.as_deref().unwrap_or("")));
                words.iter().all(|word| haystack.split(' ').any(|h| h.starts_with(word.as_str())))
            })
            .map(card)
            .collect()
    }
}

/// True when the query names One Pace, so its Arcs lead the search results.
pub fn names_one_pace(query: &str) -> bool {
    let words = normalize(query);
    words.contains("pace")
}

fn card(arc: &StoryArc) -> ShowCard {
    ShowCard {
        id: arc.id,
        title: format!("{TITLE_PREFIX}{}", arc.title),
        // Three early Arcs have no poster: their first still is better than nothing.
        cover_url: arc
            .poster_url
            .clone()
            .or_else(|| arc.episodes.iter().find_map(|e| e.thumbnail_url.clone()))
            .unwrap_or_default(),
        banner_url: None,
        color: None,
        // ONA lets Up Next continue into the next Arc (see `library::up_next`).
        format: Some("ONA".to_string()),
        episodes: Some(arc.episodes.len() as i64),
        season: None,
        season_year: None,
        average_score: None,
        genres: GENRES.iter().map(|g| g.to_string()).collect(),
    }
}

fn status(arc: &StoryArc) -> &'static str {
    if arc.work_in_progress {
        "RELEASING"
    } else {
        "FINISHED"
    }
}

fn franchise_entry(arc: &StoryArc) -> FranchiseEntry {
    let card = card(arc);
    FranchiseEntry {
        id: arc.id,
        title: card.title,
        format: card.format,
        status: Some(status(arc).to_string()),
        season: None,
        season_year: None,
        episodes: card.episodes,
        label: Some(arc.title.clone()),
    }
}

/// Which Episodes of the Arc a Release holds.
fn coverage(arc: &StoryArc, info_hash: &str) -> Coverage {
    let numbers: Vec<i64> =
        arc.episodes.iter().filter(|e| e.files.iter().any(|f| f.info_hash == info_hash)).map(|e| e.number).collect();
    match numbers.as_slice() {
        [episode] => Coverage::Episode { episode: *episode },
        _ if numbers.len() == arc.episodes.len() => Coverage::Show,
        [first, .., last] => Coverage::Range { first: *first, last: *last },
        [] => Coverage::Show,
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn release(hash: &str, nyaa_id: u64, title: &str, published_at: i64, file_count: u32) -> NyaaRelease {
        NyaaRelease {
            info_hash: hash.to_string(),
            nyaa_id,
            title: title.to_string(),
            magnet: format!("magnet:?xt=urn:btih:{hash}"),
            size_bytes: 1000,
            seeders: 5,
            leechers: 0,
            published_at,
            file_count,
        }
    }

    fn episode(number: i64, files: Vec<(&str, Cut)>, chosen: &str) -> PaceEpisode {
        PaceEpisode {
            number,
            label: format!("Wano {number:02}"),
            title: (number == 1).then(|| "The Land of Wano".to_string()),
            description: None,
            chapters: String::new(),
            anime_episodes: String::new(),
            released: None,
            length: Some(1500),
            thumbnail_url: None,
            files: files
                .into_iter()
                .map(|(hash, cut)| EpisodeFile { info_hash: hash.into(), path: format!("Wano {number:02}.mkv"), cut })
                .collect(),
            chosen: Some(chosen.to_string()),
        }
    }

    fn mapping() -> Mapping {
        let wano = StoryArc {
            id: arc_id("Wano"),
            title: "Wano".into(),
            saga: Some("Four Emperors".into()),
            description: Some("Samurai.".into()),
            poster_url: None,
            work_in_progress: true,
            episodes: vec![
                episode(1, vec![("single", Cut::Standard), ("batch", Cut::Standard)], "single"),
                episode(2, vec![("batch", Cut::Standard), ("ext", Cut::Extended)], "batch"),
                episode(3, vec![("batch", Cut::Standard)], "batch"),
            ],
        };
        let egghead = StoryArc { id: arc_id("Egghead"), title: "Egghead".into(), episodes: vec![], ..wano.clone() };
        let releases = [
            release("single", 1, "[One Pace][909-910] Wano 01 [1080p][F15AFDE0].mkv", 10, 1),
            release("batch", 2, "[One Pace][909-924] Wano Act 1", 5, 3),
            release("ext", 3, "[One Pace][909-911] Wano 02 Extended [1080p][6F26FDAB].mkv", 10, 1),
        ];
        Mapping {
            built_at: 0,
            arcs: vec![wano, egghead],
            releases: releases.into_iter().map(|r| (r.info_hash.clone(), r)).collect(),
            file_lists: BTreeMap::new(),
        }
    }

    #[test]
    fn arc_ids_are_stable_distinct_and_outside_anilist() {
        assert_eq!(arc_id("Wano"), arc_id("wano"));
        assert_ne!(arc_id("Wano"), arc_id("Egghead"));
        assert!(is_arc(arc_id("Buggy's Crew")));
        assert!(!is_arc(16498));
    }

    #[test]
    fn an_arc_is_a_show_in_the_one_pace_franchise() {
        let show = mapping().show(arc_id("Wano")).unwrap();
        assert_eq!(show.lite.card.title, "One Pace: Wano");
        assert_eq!(show.status.as_deref(), Some("RELEASING"));
        assert_eq!(show.duration, Some(25));
        assert_eq!(
            show.franchise.iter().map(|f| f.label.as_deref()).collect::<Vec<_>>(),
            [Some("Wano"), Some("Egghead")]
        );
        let titles: Vec<_> = show.episode_list.iter().map(|e| e.title.as_deref().unwrap()).collect();
        assert_eq!(titles, ["The Land of Wano", "Wano 02", "Wano 03"]);
        assert!(mapping().show(16498).is_none());
    }

    #[test]
    fn releases_mark_the_choice_and_name_the_file() {
        let list = mapping().releases(arc_id("Wano"), 2);
        let summary: Vec<_> =
            list.iter().map(|r| (r.info_hash.as_str(), r.group.as_deref().unwrap(), r.is_best, r.coverage)).collect();
        assert_eq!(
            summary,
            [
                ("batch", "One Pace", true, Coverage::Show),
                ("ext", "One Pace Extended", false, Coverage::Episode { episode: 2 }),
            ]
        );
        assert_eq!(list[0].file_path.as_deref(), Some("Wano 02.mkv"));
        assert_eq!(list[0].link, "https://nyaa.si/view/2");
        assert_eq!(list[1].resolution, Some(1080));
    }

    #[test]
    fn episodes_show_their_still_and_a_poster_less_arc_shows_the_first_one() {
        let mut mapping = mapping();
        mapping.arcs[0].episodes[1].thumbnail_url = Some("https://example.test/892.jpg".into());
        let show = mapping.show(arc_id("Wano")).unwrap();
        let stills: Vec<_> = show.episode_list.iter().map(|e| e.thumbnail_url.as_deref()).collect();
        assert_eq!(stills, [None, Some("https://example.test/892.jpg"), None]);
        assert_eq!(show.lite.card.cover_url, "https://example.test/892.jpg");

        mapping.arcs[0].poster_url = Some("https://example.test/wano.png".into());
        assert_eq!(mapping.show(arc_id("Wano")).unwrap().lite.card.cover_url, "https://example.test/wano.png");
    }

    #[test]
    fn a_rebuild_without_stills_keeps_the_older_ones() {
        let mut older = mapping();
        older.arcs[0].episodes[0].thumbnail_url = Some("https://example.test/old.jpg".into());
        older.arcs[0].episodes[1].thumbnail_url = Some("https://example.test/old2.jpg".into());
        let mut fresh = mapping();
        fresh.arcs[0].episodes[1].thumbnail_url = Some("https://example.test/new2.jpg".into());
        fresh.keep_thumbnails(&older);
        let stills: Vec<_> = fresh.arcs[0].episodes.iter().map(|e| e.thumbnail_url.as_deref()).collect();
        assert_eq!(stills, [Some("https://example.test/old.jpg"), Some("https://example.test/new2.jpg"), None]);
    }

    #[test]
    fn search_matches_word_starts() {
        let ids = |query: &str| mapping().search(query).into_iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids("one pace").len(), 2);
        assert_eq!(ids("one pace wan"), [arc_id("Wano")]);
        assert_eq!(ids("egghead"), [arc_id("Egghead")]);
        assert!(ids("one piece").is_empty());
        assert!(names_one_pace("One Pace wano") && !names_one_pace("wano"));
    }
}
