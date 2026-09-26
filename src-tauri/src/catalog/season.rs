//! Pure date -> anime season math. AniList groups Shows into a season+year, and the
//! "airing this season" and "this season" Browse rows need today's season+year.

use chrono::{Datelike, NaiveDate};

use crate::catalog::model::Season;

/// Derives the (Season, year) for a given date.
///
/// December belongs to WINTER of the *next* calendar year (the year of the January
/// that follows it), matching how AniList labels a WINTER season by its January.
pub fn season_for_date(date: NaiveDate) -> (Season, i32) {
    let year = date.year();
    match date.month() {
        12 => (Season::Winter, year + 1),
        1 | 2 => (Season::Winter, year),
        3..=5 => (Season::Spring, year),
        6..=8 => (Season::Summer, year),
        9..=11 => (Season::Fall, year),
        _ => unreachable!("chrono month() is always 1..=12"),
    }
}

/// The current season+year, derived from the local date.
pub fn current_season() -> (Season, i32) {
    season_for_date(chrono::Local::now().date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn december_is_winter_of_next_year() {
        assert_eq!(season_for_date(date(2025, 12, 1)), (Season::Winter, 2026));
        assert_eq!(season_for_date(date(2025, 12, 31)), (Season::Winter, 2026));
    }

    #[test]
    fn january_and_february_are_winter_of_same_year() {
        assert_eq!(season_for_date(date(2026, 1, 1)), (Season::Winter, 2026));
        assert_eq!(season_for_date(date(2026, 2, 28)), (Season::Winter, 2026));
    }

    #[test]
    fn spring_summer_fall_use_the_calendar_year() {
        assert_eq!(season_for_date(date(2026, 3, 1)), (Season::Spring, 2026));
        assert_eq!(season_for_date(date(2026, 5, 31)), (Season::Spring, 2026));
        assert_eq!(season_for_date(date(2026, 6, 1)), (Season::Summer, 2026));
        assert_eq!(season_for_date(date(2026, 8, 31)), (Season::Summer, 2026));
        assert_eq!(season_for_date(date(2026, 9, 1)), (Season::Fall, 2026));
        assert_eq!(season_for_date(date(2026, 11, 30)), (Season::Fall, 2026));
    }
}
