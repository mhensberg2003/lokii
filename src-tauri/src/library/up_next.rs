//! Up Next: the Episode to play next for a Show. An Episode the user left before the
//! end resumes; else it is the first Episode after the last Watched one, continuing
//! into the next Show of the Franchise.

use serde::Serialize;

use crate::catalog::model::ShowDetails;
use crate::library::progress::WatchProgress;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpNext {
    pub show_id: i64,
    pub episode: i64,
    /// Seconds to resume from; 0 starts the Episode from the beginning.
    pub position: f64,
    /// Seconds; 0 when the Episode was not played yet.
    pub duration: f64,
}

impl UpNext {
    fn start(show_id: i64, episode: i64) -> Self {
        Self { show_id, episode, position: 0.0, duration: 0.0 }
    }
}

/// The Up Next of `show` from its Watch Progress. None when the user did not play the
/// Show, or finished it and the Franchise has no aired next Show. `now` is Unix seconds:
/// `show` can be an old cached copy, so an Episode whose air time passed counts as aired.
pub fn up_next(show: &ShowDetails, progress: &[WatchProgress], now: i64) -> Option<UpNext> {
    let latest = progress.iter().max_by_key(|p| p.updated_at)?;
    if !latest.watched {
        return Some(UpNext {
            show_id: show.lite.card.id,
            episode: latest.episode,
            position: latest.position,
            duration: latest.duration,
        });
    }
    let last_watched = progress.iter().filter(|p| p.watched).map(|p| p.episode).max()?;
    next_after(show, last_watched, now)
}

/// The Episode after `episode`: the next aired Episode of the Show, else Episode 1 of
/// the next TV Show in the Franchise. A Show that still airs has no next Show yet.
/// Mirrors `nextEpisodeOf` in `src/lib/player.ts`.
pub fn next_after(show: &ShowDetails, episode: i64, now: i64) -> Option<UpNext> {
    let id = show.lite.card.id;
    if let Some(next) = show.episode_list.iter().find(|e| e.number == episode + 1) {
        let aired = next.airing_at.is_none_or(|at| at <= now);
        return aired.then(|| UpNext::start(id, next.number));
    }
    if show.status.as_deref() == Some("RELEASING") {
        return None;
    }
    let index = show.franchise.iter().position(|entry| entry.id == id)?;
    let sequel = show.franchise[index + 1..].iter().find(|entry| is_series(entry.format.as_deref()))?;
    match sequel.status.as_deref() {
        None | Some("NOT_YET_RELEASED") => None,
        Some(_) => Some(UpNext::start(sequel.id, 1)),
    }
}

fn is_series(format: Option<&str>) -> bool {
    matches!(format, Some("TV" | "TV_SHORT" | "ONA"))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::catalog::model::{EpisodeInfo, FranchiseEntry, ShowCard, ShowDetailsLite};

    const NOW: i64 = 1_800_000_000;

    pub fn show(id: i64, episodes: i64, franchise: Vec<FranchiseEntry>) -> ShowDetails {
        let card = ShowCard {
            id,
            title: format!("Show {id}"),
            cover_url: String::new(),
            banner_url: None,
            color: None,
            format: Some("TV".into()),
            episodes: Some(episodes),
            season: None,
            season_year: None,
            average_score: None,
            genres: Vec::new(),
        };
        ShowDetails {
            lite: ShowDetailsLite { card, description: String::new() },
            id_mal: None,
            title_romaji: None,
            title_native: None,
            synonyms: Vec::new(),
            status: Some("FINISHED".into()),
            duration: Some(24),
            studios: Vec::new(),
            next_airing: None,
            franchise,
            episode_list: (1..=episodes)
                .map(|number| EpisodeInfo {
                    number,
                    title: Some(format!("Title {number}")),
                    thumbnail_url: None,
                    airing_at: None,
                })
                .collect(),
            related: Vec::new(),
        }
    }

    pub fn entry(id: i64, format: &str, status: Option<&str>) -> FranchiseEntry {
        FranchiseEntry {
            id,
            title: format!("Show {id}"),
            format: Some(format.into()),
            status: status.map(String::from),
            season: None,
            season_year: None,
            episodes: Some(12),
            label: None,
        }
    }

    pub fn progress(episode: i64, position: f64, watched: bool, updated_at: i64) -> WatchProgress {
        WatchProgress { show_id: 1, episode, position, duration: 1440.0, watched, updated_at }
    }

    #[test]
    fn nothing_is_up_next_before_the_first_play() {
        assert_eq!(up_next(&show(1, 12, vec![]), &[], NOW), None);
    }

    #[test]
    fn an_episode_left_before_the_end_resumes() {
        let list = [progress(1, 1400.0, true, 1), progress(2, 300.0, false, 2)];
        let next = up_next(&show(1, 12, vec![]), &list, NOW).unwrap();
        assert_eq!((next.episode, next.position, next.duration), (2, 300.0, 1440.0));
    }

    #[test]
    fn after_a_watched_episode_the_next_one_starts() {
        // Episode 3 was played again after Episode 5; Up Next still follows Episode 5.
        let list = [progress(5, 1400.0, true, 1), progress(3, 1400.0, true, 2)];
        assert_eq!(up_next(&show(1, 12, vec![]), &list, NOW), Some(UpNext::start(1, 6)));
    }

    #[test]
    fn an_episode_that_has_not_aired_is_not_up_next() {
        let mut airing = show(1, 3, vec![]);
        airing.episode_list[2].airing_at = Some(NOW + 60);
        assert_eq!(up_next(&airing, &[progress(2, 1400.0, true, 1)], NOW), None);
    }

    #[test]
    fn an_old_copy_counts_an_episode_whose_air_time_passed_as_aired() {
        let mut airing = show(1, 3, vec![]);
        airing.episode_list[2].airing_at = Some(NOW - 60);
        assert_eq!(up_next(&airing, &[progress(2, 1400.0, true, 1)], NOW), Some(UpNext::start(1, 3)));
    }

    #[test]
    fn a_show_that_still_airs_does_not_continue_into_a_sequel() {
        let franchise = vec![entry(1, "TV", Some("RELEASING")), entry(9, "TV", Some("RELEASING"))];
        let mut airing = show(1, 2, franchise);
        airing.status = Some("RELEASING".into());
        assert_eq!(up_next(&airing, &[progress(2, 1400.0, true, 1)], NOW), None);
    }

    #[test]
    fn the_last_episode_continues_into_the_next_tv_show() {
        let franchise = vec![
            entry(1, "TV", Some("FINISHED")),
            entry(5, "MOVIE", Some("FINISHED")),
            entry(9, "TV", Some("RELEASING")),
        ];
        let finished = show(1, 2, franchise);
        assert_eq!(up_next(&finished, &[progress(2, 1400.0, true, 1)], NOW), Some(UpNext::start(9, 1)));
    }

    #[test]
    fn a_sequel_that_has_not_aired_is_not_up_next() {
        let franchise = vec![entry(1, "TV", Some("FINISHED")), entry(9, "TV", Some("NOT_YET_RELEASED"))];
        assert_eq!(up_next(&show(1, 2, franchise), &[progress(2, 1400.0, true, 1)], NOW), None);
    }
}
