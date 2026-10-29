use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt as _;
use futures::future::BoxFuture;
use reqwest::{StatusCode, header};
use serde::Deserialize;

use crate::throttle::Attempt;
use crate::{AppId, Error, Result, SearchSource, Throttle, ThrottleConfig};

const DEFAULT_BASE: &str = "https://itunes.apple.com";

/// The iTunes Search API allows at most 200 results per search.
pub const MAX_SEARCH_LIMIT: usize = 200;

/// An app as the iTunes Lookup API describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct AppDetails {
    pub id: AppId,
    pub name: String,
    pub bundle_id: Option<String>,
    pub developer: Option<String>,
    pub icon_url: Option<String>,
    pub store_url: Option<String>,
    pub genre: Option<String>,
    pub version: Option<String>,
    pub rating_count: u64,
    pub average_rating: f32,
    pub price: Option<String>,
}

/// One search result, in rank order.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub id: AppId,
    pub title: String,
    /// The iTunes APIs don't return subtitles; a richer source may.
    pub subtitle: Option<String>,
    pub developer: Option<String>,
    pub icon_url: Option<String>,
    pub rating_count: u64,
    pub average_rating: f32,
}

/// Client for the public iTunes Lookup and Search APIs.
#[derive(Clone)]
pub struct ITunesClient {
    http: reqwest::Client,
    throttle: Arc<Throttle>,
    base: String,
}

impl ITunesClient {
    pub fn new(config: ThrottleConfig) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("AppDoctor/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("reqwest client builds with static config");
        Self {
            http,
            throttle: Arc::new(Throttle::new(config)),
            base: DEFAULT_BASE.into(),
        }
    }

    /// Points the client at another host, for tests and mirrors.
    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    pub fn throttle(&self) -> &Throttle {
        &self.throttle
    }

    /// Looks an app up in one country's store.
    pub async fn lookup(&self, id: AppId, country: &str) -> Result<AppDetails> {
        let url = format!("{}/lookup", self.base);
        let query = [
            ("id", id.to_string()),
            ("country", country.to_ascii_lowercase()),
            ("entity", "software".into()),
        ];
        let body = self.get_json(&url, &query).await?;
        parse_lookup(&body)?
            .into_iter()
            .find(|app| app.id == id)
            .ok_or_else(|| Error::AppNotFound(id, country.to_ascii_uppercase()))
    }

    /// Searches one country's store for `term`, returning up to `limit`
    /// (max 200) iPhone apps in result order.
    pub async fn search(&self, term: &str, country: &str, limit: usize) -> Result<Vec<SearchHit>> {
        let url = format!("{}/search", self.base);
        let query = [
            ("term", term.to_owned()),
            ("country", country.to_ascii_lowercase()),
            ("entity", "software".into()),
            ("media", "software".into()),
            ("limit", limit.clamp(1, MAX_SEARCH_LIMIT).to_string()),
        ];
        let body = self.get_json(&url, &query).await?;
        parse_search(&body)
    }

    async fn get_json(&self, url: &str, query: &[(&str, String)]) -> Result<String> {
        self.throttle
            .run(|| async {
                let response = match self.http.get(url).query(query).send().await {
                    Ok(response) => response,
                    Err(error) if error.is_timeout() || error.is_connect() => {
                        return Attempt::Transient(error.into());
                    }
                    Err(error) => return Attempt::Fatal(error.into()),
                };
                let status = response.status();
                if is_throttled(status) {
                    return Attempt::Throttled {
                        retry_after: retry_after(response.headers()),
                    };
                }
                if status.is_server_error() {
                    return Attempt::Transient(Error::Status(status.as_u16()));
                }
                if !status.is_success() {
                    return Attempt::Fatal(Error::Status(status.as_u16()));
                }
                match response.text().await {
                    Ok(body) => Attempt::Done(body),
                    Err(error) => Attempt::Transient(error.into()),
                }
            })
            .await
    }
}

impl SearchSource for ITunesClient {
    fn search<'a>(&'a self, term: &'a str, country: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
        ITunesClient::search(self, term, country, limit).boxed()
    }

    fn name(&self) -> &'static str {
        "itunes-search"
    }
}

/// Apple answers rate-limited iTunes requests with 403 as often as 429.
fn is_throttled(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
    )
}

fn retry_after(headers: &header::HeaderMap) -> Option<Duration> {
    headers
        .get(header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    results: Vec<RawApp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawApp {
    track_id: Option<u64>,
    track_name: Option<String>,
    kind: Option<String>,
    wrapper_type: Option<String>,
    bundle_id: Option<String>,
    artist_name: Option<String>,
    seller_name: Option<String>,
    artwork_url512: Option<String>,
    artwork_url100: Option<String>,
    artwork_url60: Option<String>,
    track_view_url: Option<String>,
    primary_genre_name: Option<String>,
    version: Option<String>,
    user_rating_count: Option<u64>,
    average_user_rating: Option<f32>,
    formatted_price: Option<String>,
}

impl RawApp {
    fn is_app(&self) -> bool {
        let software = |kind: &Option<String>| {
            kind.as_deref()
                .is_some_and(|kind| kind.eq_ignore_ascii_case("software") || kind.eq_ignore_ascii_case("mac-software"))
        };
        self.track_id.is_some() && (software(&self.kind) || software(&self.wrapper_type))
    }

    fn icon(&self) -> Option<String> {
        self.artwork_url512
            .clone()
            .or_else(|| self.artwork_url100.clone())
            .or_else(|| self.artwork_url60.clone())
    }
}

pub(crate) fn parse_lookup(body: &str) -> Result<Vec<AppDetails>> {
    let envelope: Envelope = serde_json::from_str(body)?;
    Ok(envelope
        .results
        .into_iter()
        .filter(RawApp::is_app)
        .map(|raw| AppDetails {
            id: AppId(raw.track_id.unwrap_or_default()),
            icon_url: raw.icon(),
            name: raw.track_name.unwrap_or_default(),
            bundle_id: raw.bundle_id,
            developer: raw.seller_name.or(raw.artist_name),
            store_url: raw.track_view_url,
            genre: raw.primary_genre_name,
            version: raw.version,
            rating_count: raw.user_rating_count.unwrap_or(0),
            average_rating: raw.average_user_rating.unwrap_or(0.0),
            price: raw.formatted_price,
        })
        .collect())
}

pub(crate) fn parse_search(body: &str) -> Result<Vec<SearchHit>> {
    let envelope: Envelope = serde_json::from_str(body)?;
    Ok(envelope
        .results
        .into_iter()
        .filter(RawApp::is_app)
        .map(|raw| SearchHit {
            id: AppId(raw.track_id.unwrap_or_default()),
            icon_url: raw.icon(),
            title: raw.track_name.unwrap_or_default(),
            subtitle: None,
            developer: raw.seller_name.or(raw.artist_name),
            rating_count: raw.user_rating_count.unwrap_or(0),
            average_rating: raw.average_user_rating.unwrap_or(0.0),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank_of;

    const LOOKUP: &str = r#"{
      "resultCount": 1,
      "results": [{
        "wrapperType": "software", "kind": "software",
        "trackId": 904237743, "trackName": "Things 3",
        "bundleId": "com.culturedcode.ThingsiPhone",
        "artistName": "Cultured Code GmbH & Co. KG",
        "sellerName": "Cultured Code GmbH & Co. KG",
        "artworkUrl60": "https://is1.mzstatic.com/60.jpg",
        "artworkUrl100": "https://is1.mzstatic.com/100.jpg",
        "artworkUrl512": "https://is1.mzstatic.com/512.jpg",
        "trackViewUrl": "https://apps.apple.com/us/app/things-3/id904237743",
        "primaryGenreName": "Productivity",
        "version": "3.21.4",
        "averageUserRating": 4.79, "userRatingCount": 31337,
        "formattedPrice": "$9.99", "price": 9.99,
        "description": "Get things done."
      }]
    }"#;

    const SEARCH: &str = r#"{
      "resultCount": 4,
      "results": [
        {"wrapperType": "software", "kind": "software", "trackId": 1, "trackName": "Todoist", "userRatingCount": 100000, "averageUserRating": 4.8},
        {"wrapperType": "software", "kind": "software", "trackId": 904237743, "trackName": "Things 3", "userRatingCount": 31337, "averageUserRating": 4.79},
        {"wrapperType": "track", "kind": "song", "trackId": 7, "trackName": "Not an app"},
        {"wrapperType": "software", "kind": "software", "trackId": 3, "trackName": "TickTick"}
      ]
    }"#;

    #[test]
    fn parses_lookup() {
        let apps = parse_lookup(LOOKUP).unwrap();
        assert_eq!(apps.len(), 1);
        let app = &apps[0];
        assert_eq!(app.id, AppId(904237743));
        assert_eq!(app.name, "Things 3");
        assert_eq!(app.bundle_id.as_deref(), Some("com.culturedcode.ThingsiPhone"));
        assert_eq!(app.icon_url.as_deref(), Some("https://is1.mzstatic.com/512.jpg"));
        assert_eq!(app.rating_count, 31337);
        assert!((app.average_rating - 4.79).abs() < 1e-6);
        assert_eq!(app.genre.as_deref(), Some("Productivity"));
    }

    #[test]
    fn parses_search_in_order_and_skips_non_apps() {
        let hits = parse_search(SEARCH).unwrap();
        let ids: Vec<u64> = hits.iter().map(|hit| hit.id.0).collect();
        assert_eq!(ids, vec![1, 904237743, 3]);
        assert_eq!(hits[2].rating_count, 0);
        assert_eq!(rank_of(AppId(904237743), &hits), Some(2));
        assert_eq!(rank_of(AppId(42), &hits), None);
    }

    #[test]
    fn empty_and_malformed_bodies() {
        assert!(parse_search(r#"{"resultCount":0,"results":[]}"#).unwrap().is_empty());
        assert!(parse_search(r#"{"resultCount":0}"#).unwrap().is_empty());
        assert!(matches!(parse_search("<html>"), Err(Error::Decode(_))));
    }

    #[test]
    fn retry_after_seconds() {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::RETRY_AFTER, "12".parse().unwrap());
        assert_eq!(retry_after(&headers), Some(Duration::from_secs(12)));
        headers.insert(header::RETRY_AFTER, "Wed, 21 Oct 2015 07:28:00 GMT".parse().unwrap());
        assert_eq!(retry_after(&headers), None);
    }
}
