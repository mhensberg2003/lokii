//! The GraphQL query text `anilist.rs` sends. Kept separate so the client logic in
//! `anilist.rs` stays readable; every query shares the `MediaFields` fragment.

/// Fields shared by every Show card and detail view.
const MEDIA_FIELDS: &str = r#"
fragment MediaFields on Media {
  id
  idMal
  title { romaji english native }
  description
  coverImage { extraLarge large color }
  bannerImage
  format
  status
  episodes
  duration
  season
  seasonYear
  averageScore
  genres
  isAdult
  type
  studios(isMain: true) { nodes { name } }
  nextAiringEpisode { episode airingAt }
  streamingEpisodes { title thumbnail }
  relations {
    edges {
      relationType
      node {
        id
        isAdult
        type
        title { romaji english native }
        coverImage { extraLarge large color }
        bannerImage
        format
        episodes
        season
        seasonYear
        averageScore
        genres
      }
    }
  }
}
"#;

/// Home: hero (derived from `trending`) + the four Home rows, in one aliased request.
pub const HOME_QUERY: &str = r#"
query Home($season: MediaSeason, $seasonYear: Int) {
  airing: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, season: $season, seasonYear: $seasonYear, sort: POPULARITY_DESC) { ...MediaFields }
  }
  trending: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, sort: TRENDING_DESC) { ...MediaFields }
  }
  popular: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, sort: POPULARITY_DESC) { ...MediaFields }
  }
  top: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, sort: SCORE_DESC) { ...MediaFields }
  }
}
"#;

/// Browse: the four Browse rows for one genre, in one aliased request.
pub const BROWSE_QUERY: &str = r#"
query Browse($genre: String, $season: MediaSeason, $seasonYear: Int) {
  airing: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, genre: $genre, status: RELEASING, sort: POPULARITY_DESC) { ...MediaFields }
  }
  popular: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, genre: $genre, sort: POPULARITY_DESC) { ...MediaFields }
  }
  top: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, genre: $genre, sort: SCORE_DESC) { ...MediaFields }
  }
  season: Page(page: 1, perPage: 20) {
    media(type: ANIME, isAdult: false, genre: $genre, season: $season, seasonYear: $seasonYear, sort: POPULARITY_DESC) { ...MediaFields }
  }
}
"#;

/// Show: full details for one Show, including its immediate relations.
pub const SHOW_QUERY: &str = r#"
query Show($id: Int) {
  Media(id: $id, type: ANIME, isAdult: false) { ...MediaFields }
}
"#;

/// Search: up to 30 best matches for a query string.
pub const SEARCH_QUERY: &str = r#"
query Search($search: String) {
  Page(page: 1, perPage: 30) {
    media(type: ANIME, isAdult: false, search: $search, sort: SEARCH_MATCH) { ...MediaFields }
  }
}
"#;

/// Franchise-walk hop: just enough to keep walking (PREQUEL/SEQUEL) and build a
/// `FranchiseEntry`. Deliberately leaner than `MediaFields`.
pub const FRANCHISE_NODE_QUERY: &str = r#"
query FranchiseNode($id: Int) {
  Media(id: $id, type: ANIME, isAdult: false) {
    id
    isAdult
    type
    title { romaji english native }
    format
    season
    seasonYear
    episodes
    relations {
      edges {
        relationType
        node { id isAdult type format }
      }
    }
  }
}
"#;

/// Appends the shared `MediaFields` fragment when the query spreads it. AniList rejects
/// a query that defines a fragment it never uses (HTTP 400), so other queries pass through.
pub fn with_fragment(query: &str) -> String {
    if query.contains("...MediaFields") {
        format!("{query}\n{MEDIA_FIELDS}")
    } else {
        query.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_the_fragment_only_to_queries_that_use_it() {
        assert!(with_fragment(SHOW_QUERY).contains("fragment MediaFields"));
        assert_eq!(with_fragment(FRANCHISE_NODE_QUERY), FRANCHISE_NODE_QUERY);
    }
}
