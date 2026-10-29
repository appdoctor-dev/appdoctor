//! Rankings as the iPhone App Store shows them.
//!
//! The App Store app searches `search.itunes.apple.com/WebObjects/MZStore.woa/wa/search`
//! with the storefront in an `X-Apple-Store-Front: {id},24 t:native` header.
//! Its order is what users see on-device; the public iTunes Search API is a
//! separate index that drifts on competitive terms. That endpoint returns
//! ranked app IDs only, so one batched `/lookup` call fills in names,
//! ratings and icons.
//!
//! Anything unexpected (an error, a changed response shape, a storefront
//! without a known ID) falls back to the iTunes Search API, and the result
//! says which source answered.

use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt as _;
use futures::future::BoxFuture;
use reqwest::{StatusCode, header};
use serde::Deserialize;
use serde_json::Value;

use crate::throttle::Attempt;
use crate::{AppId, Error, ITunesClient, Result, SearchHit, SearchPage, SearchSource, Throttle, ThrottleConfig};

pub const NATIVE_SOURCE: &str = "app-store-iphone";
pub const ITUNES_SOURCE: &str = "itunes-search";

/// How many leading results get full details (difficulty scores the top 10).
const DETAILED: usize = 10;

const SEARCH_URL: &str = "https://search.itunes.apple.com/WebObjects/MZStore.woa/wa/search";
/// The App Store's search suggestions: what people type, most popular first.
const HINTS_URL: &str = "https://search.itunes.apple.com/WebObjects/MZSearchHints.woa/wa/hints";
/// The App Store's own requests look like a browser; a bare client UA gets
/// a different (web) response shape.
const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0 Safari/537.36";

/// Apple's numeric storefront IDs, cross-checked against the open-source
/// app-store-scraper's market table. Storefronts missing here fall back to
/// the iTunes Search API.
const STOREFRONTS: &[(&str, u32)] = &[
    ("ae", 143481),
    ("ai", 143538),
    ("am", 143524),
    ("ao", 143564),
    ("ar", 143505),
    ("at", 143445),
    ("au", 143460),
    ("az", 143568),
    ("bb", 143541),
    ("be", 143446),
    ("bg", 143526),
    ("bh", 143559),
    ("bm", 143542),
    ("bn", 143560),
    ("bo", 143556),
    ("br", 143503),
    ("bw", 143525),
    ("by", 143565),
    ("bz", 143555),
    ("ca", 143455),
    ("ch", 143459),
    ("cl", 143483),
    ("cn", 143465),
    ("co", 143501),
    ("cr", 143495),
    ("cy", 143557),
    ("cz", 143489),
    ("de", 143443),
    ("dk", 143458),
    ("dm", 143545),
    ("dz", 143563),
    ("ec", 143509),
    ("ee", 143518),
    ("eg", 143516),
    ("es", 143454),
    ("fi", 143447),
    ("fr", 143442),
    ("gb", 143444),
    ("gd", 143546),
    ("gh", 143573),
    ("gr", 143448),
    ("gt", 143504),
    ("gy", 143553),
    ("hk", 143463),
    ("hn", 143510),
    ("hr", 143494),
    ("hu", 143482),
    ("id", 143476),
    ("ie", 143449),
    ("il", 143491),
    ("in", 143467),
    ("is", 143558),
    ("it", 143450),
    ("jm", 143511),
    ("jo", 143528),
    ("jp", 143462),
    ("ke", 143529),
    ("kr", 143466),
    ("kw", 143493),
    ("ky", 143544),
    ("lb", 143497),
    ("lk", 143486),
    ("lt", 143520),
    ("lu", 143451),
    ("lv", 143519),
    ("mg", 143531),
    ("mk", 143530),
    ("ml", 143532),
    ("mo", 143515),
    ("ms", 143547),
    ("mt", 143521),
    ("mu", 143533),
    ("mx", 143468),
    ("my", 143473),
    ("ne", 143534),
    ("ng", 143561),
    ("ni", 143512),
    ("nl", 143452),
    ("no", 143457),
    ("np", 143484),
    ("nz", 143461),
    ("om", 143562),
    ("pa", 143485),
    ("pe", 143507),
    ("ph", 143474),
    ("pk", 143477),
    ("pl", 143478),
    ("pt", 143453),
    ("py", 143513),
    ("qa", 143498),
    ("ro", 143487),
    ("ru", 143469),
    ("sa", 143479),
    ("se", 143456),
    ("sg", 143464),
    ("si", 143499),
    ("sk", 143496),
    ("sn", 143535),
    ("sr", 143554),
    ("sv", 143506),
    ("th", 143475),
    ("tn", 143536),
    ("tr", 143480),
    ("tw", 143470),
    ("tz", 143572),
    ("ua", 143492),
    ("ug", 143537),
    ("us", 143441),
    ("uy", 143514),
    ("uz", 143566),
    ("ve", 143502),
    ("vg", 143543),
    ("vn", 143471),
    ("ye", 143571),
    ("za", 143472),
];

pub fn storefront_id(country: &str) -> Option<u32> {
    STOREFRONTS
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(country))
        .map(|(_, id)| *id)
}

/// The header value for the iPhone-native search of a storefront.
pub fn native_store_front(id: u32) -> String {
    format!("{id},24 t:native")
}

#[derive(Deserialize)]
struct Bubbles {
    bubbles: Option<Vec<Bubble>>,
}

#[derive(Deserialize)]
struct Bubble {
    #[serde(default)]
    results: Vec<BubbleResult>,
}

#[derive(Deserialize)]
struct BubbleResult {
    id: Value,
}

/// The `term` values of a hints plist, in order.
pub(crate) fn parse_hints(xml: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find("<key>term</key>") {
        rest = &rest[at + "<key>term</key>".len()..];
        let Some(start) = rest.find("<string>") else { break };
        let after = &rest[start + "<string>".len()..];
        let Some(end) = after.find("</string>") else { break };
        terms.push(unescape_xml(&after[..end]));
        rest = &after[end..];
    }
    terms
}

fn unescape_xml(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// Ranked app IDs from an MZStore search response, deduplicated, best first.
/// An unrecognised shape is an error, so callers fall back rather than
/// reporting every app as unranked.
pub(crate) fn parse_ranked_ids(body: &str) -> Result<Vec<AppId>> {
    let parsed: Bubbles = serde_json::from_str(body)?;
    let bubbles = parsed
        .bubbles
        .ok_or_else(|| Error::Decode(serde::de::Error::custom("no `bubbles` in App Store search response")))?;
    // The first bubble is the organic app ranking; later ones are other
    // shelves and would skew positions.
    let mut ids = Vec::new();
    for result in bubbles.into_iter().take(1).flat_map(|bubble| bubble.results) {
        let id = match &result.id {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.parse().ok(),
            _ => None,
        };
        if let Some(id) = id.map(AppId)
            && !ids.contains(&id)
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// `hits` in the order of `ranked`, dropping IDs `/lookup` didn't return
/// (removed apps, other-platform results).
pub(crate) fn in_rank_order(ranked: &[AppId], hits: Vec<SearchHit>) -> Vec<SearchHit> {
    let mut by_id: std::collections::HashMap<AppId, SearchHit> = hits.into_iter().map(|hit| (hit.id, hit)).collect();
    ranked.iter().filter_map(|id| by_id.remove(id)).collect()
}

/// App Store–native search with an iTunes Search fallback.
#[derive(Clone)]
pub struct NativeSearch {
    http: reqwest::Client,
    throttle: Arc<Throttle>,
    itunes: ITunesClient,
}

impl NativeSearch {
    pub fn new(itunes: ITunesClient, config: ThrottleConfig) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(USER_AGENT)
                .timeout(Duration::from_secs(20))
                .build()
                .expect("reqwest client builds with static config"),
            throttle: Arc::new(Throttle::new(config)),
            itunes,
        }
    }

    /// Ranked IDs from the App Store's own search.
    pub async fn ranked_ids(&self, term: &str, storefront: u32) -> Result<Vec<AppId>> {
        let store_front = native_store_front(storefront);
        let body = self
            .throttle
            .run(|| async {
                let request = self
                    .http
                    .get(SEARCH_URL)
                    .query(&[("clientApplication", "Software"), ("media", "software"), ("term", term)])
                    .header(header::ACCEPT, "application/json")
                    .header(header::ACCEPT_LANGUAGE, "en-us")
                    .header("X-Apple-Store-Front", &store_front);
                let response = match request.send().await {
                    Ok(response) => response,
                    Err(error) if error.is_timeout() || error.is_connect() => return Attempt::Transient(error.into()),
                    Err(error) => return Attempt::Fatal(error.into()),
                };
                let status = response.status();
                if matches!(
                    status,
                    StatusCode::TOO_MANY_REQUESTS | StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
                ) {
                    return Attempt::Throttled { retry_after: None };
                }
                if !status.is_success() {
                    return Attempt::Fatal(Error::Status(status.as_u16()));
                }
                match response.text().await {
                    Ok(body) => Attempt::Done(body),
                    Err(error) => Attempt::Transient(error.into()),
                }
            })
            .await?;
        parse_ranked_ids(&body)
    }

    /// App Store search suggestions for a prefix, most popular first. Empty
    /// for storefronts without a known ID.
    pub async fn hints(&self, term: &str, country: &str) -> Result<Vec<String>> {
        let Some(storefront) = storefront_id(country) else {
            return Ok(Vec::new());
        };
        let store_front = if storefront == 143441 {
            "143441-1,29".to_owned()
        } else {
            format!("{storefront},29")
        };
        let body = self
            .throttle
            .run(|| async {
                let request = self
                    .http
                    .get(HINTS_URL)
                    .query(&[("clientApplication", "Software"), ("term", term)])
                    .header(header::ACCEPT, "text/xml,application/xml;q=0.9,*/*;q=0.8")
                    .header("X-Apple-Store-Front", &store_front);
                match request.send().await {
                    Ok(response) if response.status().is_success() => match response.text().await {
                        Ok(body) => Attempt::Done(body),
                        Err(error) => Attempt::Transient(error.into()),
                    },
                    Ok(response) if response.status() == StatusCode::TOO_MANY_REQUESTS => {
                        Attempt::Throttled { retry_after: None }
                    }
                    Ok(response) => Attempt::Fatal(Error::Status(response.status().as_u16())),
                    Err(error) => Attempt::Transient(error.into()),
                }
            })
            .await?;
        Ok(parse_hints(&body))
    }

    async fn native(&self, term: &str, country: &str, storefront: u32, limit: usize) -> Result<SearchPage> {
        let mut ranked = self.ranked_ids(term, storefront).await?;
        ranked.truncate(limit);
        let leading = &ranked[..ranked.len().min(DETAILED)];
        let top = in_rank_order(leading, self.itunes.lookup_hits(leading, country).await?);
        Ok(SearchPage {
            ranked,
            top,
            source: NATIVE_SOURCE,
        })
    }

    pub async fn search(&self, term: &str, country: &str, limit: usize) -> Result<SearchPage> {
        if let Some(storefront) = storefront_id(country) {
            match self.native(term, country, storefront, limit).await {
                Ok(page) => return Ok(page),
                Err(error) => tracing::warn!(%error, term, country, "App Store search failed; using iTunes Search"),
            }
        }
        let hits = self.itunes.search(term, country, limit).await?;
        Ok(SearchPage {
            ranked: hits.iter().map(|hit| hit.id).collect(),
            top: hits,
            source: ITUNES_SOURCE,
        })
    }
}

impl SearchSource for NativeSearch {
    fn search<'a>(&'a self, term: &'a str, country: &'a str, limit: usize) -> BoxFuture<'a, Result<SearchPage>> {
        NativeSearch::search(self, term, country, limit).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(id: u64) -> SearchHit {
        SearchHit {
            id: AppId(id),
            title: format!("App {id}"),
            subtitle: None,
            developer: None,
            icon_url: None,
            rating_count: 0,
            average_rating: 0.0,
        }
    }

    #[test]
    fn parses_ranked_ids_from_the_first_bubble() {
        let body = r#"{"bubbles": [
            {"name": "software", "results": [{"id": "904237743", "type": "software"}, {"id": 572688855}, {"id": "904237743"}]},
            {"name": "editorial", "results": [{"id": "1"}, {"id": {"nested": true}}]}
        ], "storePlatformData": {}}"#;
        assert_eq!(
            parse_ranked_ids(body).unwrap(),
            vec![AppId(904237743), AppId(572688855)]
        );
        assert!(parse_ranked_ids(r#"{"bubbles": []}"#).unwrap().is_empty());
    }

    #[test]
    fn unexpected_shapes_are_errors_so_callers_fall_back() {
        assert!(parse_ranked_ids(r#"{"pageData": {}}"#).is_err());
        assert!(parse_ranked_ids("<html>").is_err());
    }

    #[test]
    fn lookup_results_follow_store_order() {
        let ranked = [AppId(3), AppId(1), AppId(2), AppId(9)];
        let ordered = in_rank_order(&ranked, vec![hit(1), hit(2), hit(3)]);
        assert_eq!(ordered.iter().map(|h| h.id.0).collect::<Vec<_>>(), vec![3, 1, 2]);
    }

    #[test]
    fn parses_search_hints() {
        let xml = r#"<?xml version="1.0"?><plist version="1.0"><dict>
            <key>title</key><string>Suggestions</string>
            <key>hints</key><array>
              <dict><key>term</key><string>task rabbit</string><key>url</key><string>https://x?a=1&amp;b=2</string></dict>
              <dict><key>term</key><string>notes &amp; tasks</string><key>url</key><string>u</string></dict>
            </array></dict></plist>"#;
        assert_eq!(parse_hints(xml), vec!["task rabbit", "notes & tasks"]);
        assert!(parse_hints("<plist/>").is_empty());
    }

    #[test]
    fn storefronts_and_header() {
        assert_eq!(storefront_id("US"), Some(143441));
        assert_eq!(storefront_id("gb"), Some(143444));
        assert_eq!(storefront_id("zz"), None);
        assert_eq!(native_store_front(143441), "143441,24 t:native");
        for (code, _) in STOREFRONTS {
            assert!(crate::country(code).is_some(), "{code} isn't a known storefront");
        }
    }
}
