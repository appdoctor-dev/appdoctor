//! "Check ranks": search every keyword in its storefront, find the app's
//! position, score difficulty from the top results, and record a snapshot.
//! Searches run concurrently; the App Store client's throttle decides how
//! many actually hit Apple at once.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use appdoctor_appstore::{AppId, SearchHit, rank_of};
use appdoctor_scoring::RankedApp;
use appdoctor_secrets::{AppleAdsKey, Secret};
use appdoctor_store::{KeywordRow, Popularity, Snapshot, TopApp};
use futures::StreamExt as _;
use futures::channel::mpsc::UnboundedSender;

use crate::services::Services;

/// How deep to look for the app. The iTunes Search API stops at 200.
const SEARCH_DEPTH: usize = 200;
const TOP_APPS_KEPT: usize = 10;
const CONCURRENCY: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Outcome {
    pub checked: usize,
    pub failed: usize,
    pub first_error: Option<String>,
    /// `None` when Apple Ads isn't connected.
    pub popularity: Option<Result<usize, String>>,
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

pub async fn run(
    services: Arc<Services>,
    app_id: u64,
    keywords: Vec<KeywordRow>,
    progress: UnboundedSender<Progress>,
) -> anyhow::Result<Outcome> {
    let total = keywords.len();
    let checked_at = now();
    let mut outcome = Outcome::default();
    let _ = progress.unbounded_send(Progress {
        done: 0,
        total,
        failed: 0,
    });

    // Owned keywords per future: borrowed ones trip the `Send` bound the
    // runtime puts on this task.
    let mut results = futures::stream::iter(keywords.clone().into_iter().map(|keyword| {
        let services = services.clone();
        async move { check_one(&services, app_id, &keyword, checked_at).await }
    }))
    .buffer_unordered(CONCURRENCY);

    let mut done = 0;
    while let Some(result) = results.next().await {
        done += 1;
        match result {
            Ok(()) => outcome.checked += 1,
            Err(error) => {
                outcome.failed += 1;
                tracing::warn!(%error, "rank check failed");
                outcome.first_error.get_or_insert_with(|| error.to_string());
            }
        }
        let _ = progress.unbounded_send(Progress {
            done,
            total,
            failed: outcome.failed,
        });
    }
    drop(results);

    outcome.popularity = refresh_popularity(&services, &keywords)
        .await
        .map(|result| result.map_err(|error| error.to_string()));
    Ok(outcome)
}

async fn check_one(services: &Services, app_id: u64, keyword: &KeywordRow, checked_at: i64) -> anyhow::Result<()> {
    let source = services.search_source();
    let hits = source.search(&keyword.term, &keyword.country, SEARCH_DEPTH).await?;
    let rank = rank_of(AppId(app_id), &hits);
    let difficulty = appdoctor_scoring::difficulty(&keyword.term, &top_ranked(&hits));
    services.store.record_snapshot(&Snapshot {
        keyword_id: keyword.id,
        checked_at,
        rank,
        results_count: hits.len() as u32,
        difficulty: Some(difficulty.score),
        source: source.name().into(),
        top_apps: hits
            .iter()
            .take(TOP_APPS_KEPT)
            .map(|hit| TopApp {
                id: hit.id.0,
                name: hit.title.clone(),
                icon_url: hit.icon_url.clone(),
            })
            .collect(),
    })?;
    Ok(())
}

fn top_ranked(hits: &[SearchHit]) -> Vec<RankedApp> {
    hits.iter()
        .take(appdoctor_scoring::TOP_N)
        .map(|hit| RankedApp {
            title: hit.title.clone(),
            subtitle: hit.subtitle.clone(),
            rating_count: hit.rating_count,
            average_rating: hit.average_rating,
        })
        .collect()
}

/// Fetches Apple Ads popularity for the checked terms, when Apple Ads is
/// connected. Returns how many terms got a score.
async fn refresh_popularity(services: &Services, keywords: &[KeywordRow]) -> Option<anyhow::Result<usize>> {
    let key = match services.vault.get::<AppleAdsKey>(Secret::AppleAds) {
        Ok(Some(key)) if key.is_complete() => key,
        Ok(_) => return None,
        Err(error) => return Some(Err(error.into())),
    };
    Some(fetch_popularity(services, key, keywords).await)
}

async fn fetch_popularity(services: &Services, key: AppleAdsKey, keywords: &[KeywordRow]) -> anyhow::Result<usize> {
    let private_key = appdoctor_jwt::EcKey::from_pem(&key.private_key_pem)?;
    let client = appdoctor_apple_ads::Client::new(appdoctor_apple_ads::Credentials::new(
        &key.client_id,
        &key.team_id,
        &key.key_id,
        private_key,
    ));
    if let Some(account) = key.ad_account_id {
        client.set_ad_account(account);
    }

    // Term → the storefronts it's tracked in.
    let mut countries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for keyword in keywords {
        countries
            .entry(keyword.term.clone())
            .or_default()
            .push(keyword.country.clone());
    }
    let phrases: Vec<String> = countries.keys().cloned().collect();
    let scores = client.phrase_popularity(&phrases).await?;
    let fetched_at = now();
    for score in &scores {
        let term = appdoctor_store::normalize_term(&score.phrase);
        // TODO(apple-ads): phrase SEARCH isn't country-scoped as far as the
        // docs say, so the same score is filed under every storefront the
        // term is tracked in. Switch to search-term-popularity insights for
        // per-country scores once verified.
        for country in countries.get(&term).into_iter().flatten() {
            services.store.set_popularity(&Popularity {
                term: term.clone(),
                country: country.clone(),
                popularity: score.popularity,
                source: "apple-ads-phrases".into(),
                fetched_at,
            })?;
        }
    }
    Ok(scores.len())
}
