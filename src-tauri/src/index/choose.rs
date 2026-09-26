//! The Chosen Release rule and the list order for one Episode.

use std::cmp::Reverse;

use crate::index::model::Release;

/// The user's pick for a Show. When the picked Release does not contain an Episode,
/// the app follows the same group and resolution ("SubsPlease 1080p") to that Episode,
/// the Best Release of that group first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub info_hash: String,
    pub group: Option<String>,
    pub resolution: Option<u32>,
}

impl Pick {
    pub fn from_release(release: &Release) -> Self {
        Self { info_hash: release.info_hash.clone(), group: release.group.clone(), resolution: release.resolution }
    }
}

/// Best Release first, then higher resolution, then more seeders.
pub fn sort(releases: &mut [Release]) {
    releases.sort_by_key(|r| (Reverse(r.is_best), Reverse(r.resolution.unwrap_or(0)), Reverse(r.seeders)));
}

/// Returns the Chosen Release's info hash among `releases` (all of which contain the
/// Episode), and whether it comes from the user's pick.
pub fn choose(releases: &[Release], pick: Option<&Pick>) -> (Option<String>, bool) {
    if let Some(release) = pick.and_then(|pick| follow_pick(releases, pick)) {
        return (Some(release.info_hash.clone()), true);
    }
    (automatic(releases).map(|r| r.info_hash.clone()), false)
}

fn follow_pick<'a>(releases: &'a [Release], pick: &Pick) -> Option<&'a Release> {
    if let Some(exact) = releases.iter().find(|r| r.info_hash == pick.info_hash) {
        return Some(exact);
    }
    let group = pick.group.as_deref()?;
    releases
        .iter()
        .filter(|r| {
            r.group.as_deref().is_some_and(|g| g.eq_ignore_ascii_case(group)) && r.resolution == pick.resolution
        })
        .max_by_key(|r| (r.is_best, r.seeders))
}

/// The Best Release, else the 1080p Release with the most seeders, else the Release
/// with the most seeders. A Release without seeders is a last resort.
fn automatic(releases: &[Release]) -> Option<&Release> {
    let live = || releases.iter().filter(|r| r.seeders > 0);
    live()
        .filter(|r| r.is_best)
        .max_by_key(|r| r.seeders)
        .or_else(|| live().filter(|r| r.resolution == Some(1080)).max_by_key(|r| r.seeders))
        .or_else(|| live().max_by_key(|r| r.seeders))
        .or_else(|| releases.iter().find(|r| r.is_best))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::model::Coverage;

    fn release(hash: &str, group: &str, resolution: u32, seeders: u32, is_best: bool) -> Release {
        Release {
            info_hash: hash.to_string(),
            title: format!("[{group}] Show - 01 ({resolution}p)"),
            group: Some(group.to_string()),
            resolution: Some(resolution),
            size_bytes: 1,
            seeders,
            leechers: 0,
            file_count: 1,
            coverage: Coverage::Episode { episode: 1 },
            is_best,
            published_at: 0,
            magnet: String::new(),
            link: String::new(),
            file_path: None,
        }
    }

    fn hashes(releases: &[Release]) -> Vec<&str> {
        releases.iter().map(|r| r.info_hash.as_str()).collect()
    }

    #[test]
    fn the_best_release_wins_when_it_has_seeders() {
        let list = [release("a", "A", 1080, 500, false), release("b", "B", 1080, 3, true)];
        assert_eq!(choose(&list, None), (Some("b".into()), false));
    }

    #[test]
    fn without_a_best_release_the_most_seeded_1080p_wins() {
        let list = [
            release("a", "A", 720, 900, false),
            release("b", "B", 1080, 40, false),
            release("c", "C", 1080, 90, false),
        ];
        assert_eq!(choose(&list, None), (Some("c".into()), false));
    }

    #[test]
    fn a_dead_best_release_loses_to_a_live_one() {
        let list = [release("a", "A", 1080, 0, true), release("b", "B", 720, 5, false)];
        assert_eq!(choose(&list, None).0.as_deref(), Some("b"));
    }

    #[test]
    fn with_no_seeders_anywhere_the_best_release_still_wins() {
        let list = [release("a", "A", 1080, 0, false), release("b", "B", 1080, 0, true)];
        assert_eq!(choose(&list, None).0.as_deref(), Some("b"));
        assert_eq!(choose(&[release("a", "A", 1080, 0, false)], None).0, None);
    }

    #[test]
    fn the_user_pick_wins_and_follows_its_group_to_other_episodes() {
        let list = [release("a", "SubsPlease", 1080, 10, false), release("b", "Erai-raws", 1080, 99, false)];
        let exact = Pick { info_hash: "a".into(), group: Some("SubsPlease".into()), resolution: Some(1080) };
        assert_eq!(choose(&list, Some(&exact)), (Some("a".into()), true));

        let other_episode = Pick { info_hash: "zz".into(), group: Some("subsplease".into()), resolution: Some(1080) };
        assert_eq!(choose(&list, Some(&other_episode)), (Some("a".into()), true));

        let gone = Pick { info_hash: "zz".into(), group: Some("Nobody".into()), resolution: Some(1080) };
        assert_eq!(choose(&list, Some(&gone)), (Some("b".into()), false));
    }

    #[test]
    fn a_followed_pick_prefers_the_best_release_of_its_group() {
        let list = [release("batch", "One Pace", 1080, 90, false), release("single", "One Pace", 1080, 10, true)];
        let pick = Pick { info_hash: "other-episode".into(), group: Some("One Pace".into()), resolution: Some(1080) };
        assert_eq!(choose(&list, Some(&pick)), (Some("single".into()), true));
    }

    #[test]
    fn sort_puts_best_then_resolution_then_seeders() {
        let mut list = [
            release("a", "A", 720, 900, false),
            release("b", "B", 1080, 40, false),
            release("c", "C", 1080, 90, false),
            release("d", "D", 480, 1, true),
        ];
        sort(&mut list);
        assert_eq!(hashes(&list), vec!["d", "c", "b", "a"]);
    }
}
