//! Public App Store data, fetched from the user's own machine.
//!
//! - [`parse_app_ref`] turns an App Store URL or a bare ID into an [`AppId`].
//! - [`ITunesClient`] looks apps up (iTunes Lookup API) and runs keyword
//!   searches (iTunes Search API) through a [`Throttle`] that limits
//!   concurrency and backs off when Apple starts refusing requests, since every
//!   request comes from the user's IP.
//! - [`SearchSource`] is the seam for keyword search: rank checks depend on the
//!   trait, so a source that mirrors the App Store's real result order can
//!   replace the iTunes Search API without touching callers.

mod countries;
mod itunes;
mod parse;
mod throttle;

pub use countries::{COUNTRIES, Country, country};
pub use itunes::{AppDetails, ITunesClient, SearchHit};
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

/// Where rank checks get search results from.
///
/// Returns hits in the order the store ranks them, at most `limit` long. A
/// keyword's rank for an app is its 1-based position in this list.
pub trait SearchSource: Send + Sync {
    fn search<'a>(&'a self, term: &'a str, country: &'a str, limit: usize) -> BoxFuture<'a, Result<Vec<SearchHit>>>;

    /// A short name for the source, stored with rank snapshots so history
    /// from different sources is never compared as if it were one series.
    fn name(&self) -> &'static str;
}

/// The 1-based position of `app` in `hits`, or `None` when it isn't there.
pub fn rank_of(app: AppId, hits: &[SearchHit]) -> Option<u32> {
    hits.iter().position(|hit| hit.id == app).map(|ix| ix as u32 + 1)
}
