//! Public App Store data, fetched from the user's own machine.
//!
//! - [`parse_app_ref`] turns an App Store URL or a bare ID into an [`AppId`].
//! - [`ITunesClient`] looks apps up (iTunes Lookup API) and runs keyword
//!   searches (iTunes Search API) through a [`Throttle`] that limits
//!   concurrency and backs off when Apple starts refusing requests, since every
//!   request comes from the user's IP.
//! - [`NativeSearch`] gets rankings the way the iPhone App Store does
//!   (Apple's MZStore search with a native storefront header), falling back
//!   to the iTunes Search API, whose order drifts on competitive terms.
//! - [`SearchSource`] is the seam rank checks depend on; each
//!   [`SearchPage`] says which source answered.

mod countries;
mod itunes;
mod native;
mod parse;
mod throttle;

pub use countries::{COUNTRIES, Country, country};
pub use itunes::{AppDetails, ITunesClient, SearchHit};
pub use native::{ITUNES_SOURCE, NATIVE_SOURCE, NativeSearch, native_store_front, storefront_id};
pub use parse::{AppRef, parse_app_ref};
pub use throttle::{Throttle, ThrottleConfig};

use futures::future::BoxFuture;

/// An App Store app ID (Apple's "Adam ID", `trackId` in iTunes APIs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AppId(pub u64);

impl std::fmt::Display for AppId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not an App Store URL or app ID: {0}")]
    InvalidAppRef(String),
    #[error("app {0} was not found in the {1} App Store")]
    AppNotFound(AppId, String),
    #[error("the App Store is rate-limiting requests from this network; try again in a few minutes")]
    RateLimited,
    #[error("App Store request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected App Store response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("App Store returned HTTP {0}")]
    Status(u16),
}

pub type Result<T> = std::result::Result<T, Error>;

/// One search's results and where they came from.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchPage {
    /// Every result, in the order the store ranks them.
    pub ranked: Vec<AppId>,
    /// Details for the leading results (at least the top 10 when that many
    /// exist), in rank order. Rank needs only `ranked`; difficulty and the
    /// "apps in ranking" icons need these.
    pub top: Vec<SearchHit>,
    /// Stored with rank snapshots so history from different sources is
    /// never compared as one series.
    pub source: &'static str,
}

impl SearchPage {
    pub fn rank_of(&self, app: AppId) -> Option<u32> {
        self.ranked.iter().position(|id| *id == app).map(|ix| ix as u32 + 1)
    }
}

/// Where rank checks get search results from.
///
/// Returns at most `limit` hits in the order the store ranks them. A
/// keyword's rank for an app is its 1-based position in this list.
pub trait SearchSource: Send + Sync {
    fn search<'a>(&'a self, term: &'a str, country: &'a str, limit: usize) -> BoxFuture<'a, Result<SearchPage>>;
}

/// The 1-based position of `app` in `hits`, or `None` when it isn't there.
pub fn rank_of(app: AppId, hits: &[SearchHit]) -> Option<u32> {
    hits.iter().position(|hit| hit.id == app).map(|ix| ix as u32 + 1)
}
