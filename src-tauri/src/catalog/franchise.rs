//! Walks the Franchise chain: PREQUEL relations back to the first entry, then SEQUEL
//! relations forward, keeping only TV / TV_SHORT / ONA entries. A movie or OVA queried
//! directly is its own one-entry Franchise.

use std::collections::HashSet;

use async_trait::async_trait;

use crate::catalog::model::{to_franchise_entry, FranchiseEntry};
use crate::catalog::raw::{RawFranchiseNode, RelationType, ANIME_TYPE, CHAIN_FORMATS};

/// Total prequel + sequel hops allowed in one walk. Also the backstop against a bad
/// AniList relation graph forming a cycle (paired with the `visited` set below).
pub const MAX_HOPS: usize = 12;

/// Fetches the lean Franchise-node shape for one Show id. Implemented by the AniList
/// client for real use, and by an in-memory map in tests.
#[async_trait]
pub trait NodeFetcher {
    async fn fetch(&self, id: i64) -> Result<RawFranchiseNode, String>;
}

fn is_chain_member(node: &RawFranchiseNode) -> bool {
    node.media_type == ANIME_TYPE
        && !node.is_adult
        && node.format.as_deref().is_some_and(|f| CHAIN_FORMATS.contains(&f))
}

fn relation_target(node: &RawFranchiseNode, relation: RelationType) -> Option<i64> {
    node.relations.as_ref()?.edges.iter().find(|edge| edge.relation_type == relation).map(|edge| edge.node.id)
}

/// Walks one direction (PREQUEL or SEQUEL) from `start`, stopping at the hop budget,
/// a cycle, a fetch error, or a neighbour that is not a chain member. Returns the
/// visited nodes nearest-to-`start` first.
async fn walk_direction<F: NodeFetcher>(
    start: &RawFranchiseNode,
    relation: RelationType,
    visited: &mut HashSet<i64>,
    hops_left: &mut usize,
    fetcher: &F,
) -> Vec<RawFranchiseNode> {
    let mut chain = Vec::new();
    let mut current = start.clone();
    while *hops_left > 0 {
        let Some(id) = relation_target(&current, relation) else {
            break;
        };
        if !visited.insert(id) {
            break; // cycle guard: this id is already part of the walk
        }
        let Ok(node) = fetcher.fetch(id).await else {
            break;
        };
        if !is_chain_member(&node) {
            break;
        }
        *hops_left -= 1;
        chain.push(node.clone());
        current = node;
    }
    chain
}

/// Walks the Franchise chain starting at `start`. Returns entries oldest first,
/// including `start`. If `start` itself is not a TV / TV_SHORT / ONA entry, the
/// Franchise is just `[start]`.
pub async fn walk<F: NodeFetcher>(start: RawFranchiseNode, fetcher: &F) -> Vec<FranchiseEntry> {
    if !is_chain_member(&start) {
        return vec![to_franchise_entry(&start)];
    }

    let mut visited = HashSet::from([start.id]);
    let mut hops_left = MAX_HOPS;

    let mut backward = walk_direction(&start, RelationType::Prequel, &mut visited, &mut hops_left, fetcher).await;
    backward.reverse(); // oldest prequel first

    let forward = walk_direction(&start, RelationType::Sequel, &mut visited, &mut hops_left, fetcher).await;

    backward.into_iter().chain(std::iter::once(start)).chain(forward).map(|n| to_franchise_entry(&n)).collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::catalog::raw::{RawRelationConnection, RawRelationEdge, RawRelationNode, RawTitle};

    struct MapFetcher(HashMap<i64, RawFranchiseNode>);

    #[async_trait]
    impl NodeFetcher for MapFetcher {
        async fn fetch(&self, id: i64) -> Result<RawFranchiseNode, String> {
            self.0.get(&id).cloned().ok_or_else(|| format!("no node {id}"))
        }
    }

    fn relation_node(id: i64, media_type: &str) -> RawRelationNode {
        RawRelationNode {
            id,
            is_adult: false,
            media_type: Some(media_type.to_string()),
            title: None,
            cover_image: None,
            banner_image: None,
            format: None,
            episodes: None,
            season: None,
            season_year: None,
            average_score: None,
            genres: Vec::new(),
        }
    }

    fn node(id: i64, format: &str, edges: Vec<(RelationType, i64)>) -> RawFranchiseNode {
        RawFranchiseNode {
            id,
            is_adult: false,
            media_type: ANIME_TYPE.to_string(),
            title: RawTitle { romaji: Some(format!("Show {id}")), english: None, native: None },
            format: Some(format.to_string()),
            season: None,
            season_year: None,
            episodes: Some(12),
            relations: Some(RawRelationConnection {
                edges: edges
                    .into_iter()
                    .map(|(relation_type, target)| RawRelationEdge {
                        relation_type,
                        node: relation_node(target, ANIME_TYPE),
                    })
                    .collect(),
            }),
        }
    }

    fn fetcher_of(nodes: Vec<RawFranchiseNode>) -> MapFetcher {
        MapFetcher(nodes.into_iter().map(|n| (n.id, n)).collect())
    }

    #[tokio::test]
    async fn orders_prequels_then_self_then_sequels() {
        // Chain: 1 <-prequel- 2 (start) -sequel-> 3
        let one = node(1, "TV", vec![(RelationType::Sequel, 2)]);
        let two = node(2, "TV", vec![(RelationType::Prequel, 1), (RelationType::Sequel, 3)]);
        let three = node(3, "TV", vec![(RelationType::Prequel, 2)]);
        let fetcher = fetcher_of(vec![one, three]);

        let chain = walk(two, &fetcher).await;
        let ids: Vec<i64> = chain.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn a_movie_start_is_its_own_franchise() {
        let movie = node(9, "MOVIE", vec![(RelationType::Prequel, 1)]);
        let fetcher = fetcher_of(vec![]);

        let chain = walk(movie, &fetcher).await;
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].id, 9);
    }

    #[tokio::test]
    async fn stops_at_a_sequel_that_is_a_movie() {
        // start -sequel-> movie: the movie is not a chain member, so it is dropped.
        let start = node(1, "TV", vec![(RelationType::Sequel, 2)]);
        let movie = node(2, "MOVIE", vec![]);
        let fetcher = fetcher_of(vec![movie]);

        let chain = walk(start, &fetcher).await;
        let ids: Vec<i64> = chain.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1]);
    }

    #[tokio::test]
    async fn cycle_guard_terminates() {
        // 1 -sequel-> 2 -sequel-> 1 (a malformed, cyclic graph).
        let one = node(1, "TV", vec![(RelationType::Sequel, 2)]);
        let two = node(2, "TV", vec![(RelationType::Sequel, 1)]);
        let fetcher = fetcher_of(vec![two]);

        let chain = walk(one, &fetcher).await;
        let ids: Vec<i64> = chain.iter().map(|e| e.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[tokio::test]
    async fn caps_total_hops_at_max_hops() {
        // A one-directional chain of 20 sequels; only MAX_HOPS beyond `start` are kept.
        let mut nodes = Vec::new();
        for id in 1..=20i64 {
            let next = if id < 20 { vec![(RelationType::Sequel, id + 1)] } else { vec![] };
            nodes.push(node(id, "TV", next));
        }
        let start = nodes[0].clone();
        let fetcher = fetcher_of(nodes[1..].to_vec());

        let chain = walk(start, &fetcher).await;
        assert_eq!(chain.len(), MAX_HOPS + 1);
        assert_eq!(chain.first().unwrap().id, 1);
        assert_eq!(chain.last().unwrap().id, (MAX_HOPS + 1) as i64);
    }
}
