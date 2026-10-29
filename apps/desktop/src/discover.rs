//! "What does this competitor rank for?"
//!
//! Apple doesn't publish an app's keywords, so this works the way ASO tools
//! do: build candidate searches from the competitor's own title and
//! description plus the App Store's search suggestions for its title words,
//! search each one, and keep the ones where the competitor actually ranks.

use std::sync::Arc;

use appdoctor_appstore::AppId;
use appdoctor_scoring::RankedApp;
use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedSender;

use crate::services::Services;

/// Searches made per discovery, to stay polite with Apple.
const MAX_CANDIDATES: usize = 36;
const FROM_METADATA: usize = 24;
/// Title words whose search suggestions are added.
const HINT_SEEDS: usize = 4;
const CONCURRENCY: usize = 6;

#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub term: String,
    pub competitor_rank: u32,
    pub your_rank: Option<u32>,
    pub difficulty: f32,
    pub results: u32,
}

pub async fn run(
    services: Arc<Services>,
    competitor: u64,
    you: u64,
    country: String,
    progress: UnboundedSender<(usize, usize)>,
) -> anyhow::Result<Vec<Found>> {
    let details = services.itunes.lookup(AppId(competitor), &country).await?;
    let mut candidates = appdoctor_scoring::candidates(
        &details.name,
        details.description.as_deref().unwrap_or_default(),
        FROM_METADATA,
    );
    // What people actually type, starting from the competitor's title words.
    let seeds: Vec<String> = candidates
        .iter()
        .filter(|c| !c.contains(' '))
        .take(HINT_SEEDS)
        .cloned()
        .collect();
    for seed in seeds {
        match services.search.hints(&seed, &country).await {
            Ok(hints) => {
                for hint in hints.into_iter().take(4) {
                    let hint = appdoctor_store::normalize_term(&hint);
                    if candidates.len() < MAX_CANDIDATES && !candidates.contains(&hint) {
                        candidates.push(hint);
                    }
                }
            }
            Err(error) => tracing::debug!(%error, seed, "search hints failed"),
        }
    }

    let total = candidates.len();
    let _ = progress.unbounded_send((0, total));
    let mut checks = futures::stream::iter(candidates.into_iter().map(|term| {
        let (services, country) = (services.clone(), country.clone());
        async move {
            let page = services.search_source().search(&term, &country, 250).await;
            (term, page)
        }
    }))
    .buffer_unordered(CONCURRENCY);

    let mut found = Vec::new();
    let mut done = 0;
    while let Some((term, page)) = checks.next().await {
        done += 1;
        let _ = progress.unbounded_send((done, total));
        let page = match page {
            Ok(page) => page,
            Err(error) => {
                tracing::debug!(%error, term, "candidate search failed");
                continue;
            }
        };
        let Some(competitor_rank) = page.rank_of(AppId(competitor)) else {
            continue;
        };
        let top: Vec<RankedApp> = page
            .top
            .iter()
            .map(|hit| RankedApp {
                title: hit.title.clone(),
                subtitle: hit.subtitle.clone(),
                rating_count: hit.rating_count,
                average_rating: hit.average_rating,
            })
            .collect();
        found.push(Found {
            competitor_rank,
            your_rank: page.rank_of(AppId(you)),
            difficulty: appdoctor_scoring::difficulty(&term, &top).score,
            results: page.ranked.len() as u32,
            term,
        });
    }
    found.sort_by_key(|f| (f.competitor_rank, f.term.clone()));
    Ok(found)
}
