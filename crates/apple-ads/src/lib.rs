//! Apple Ads Platform API 1.0 client, for keyword popularity.
//!
//! Setup, as Apple documents it, with AppDoctor doing the OpenSSL steps:
//!
//! 1. AppDoctor generates a P-256 key pair ([`generate_key`]); the private
//!    key goes to the OS keychain and never leaves the machine.
//! 2. The user pastes the public PEM into Apple Ads → Account Settings → API
//!    and copies back the `clientId`, `teamId` and `keyId` Apple shows.
//! 3. For each access token, AppDoctor signs a client secret JWT
//!    ([`Credentials::client_secret`]) and exchanges it at
//!    `https://appleid.apple.com/auth/oauth2/token` (scope `searchadsorg`).
//! 4. Requests go to `https://api.ads.apple.com/v1` with `Authorization:
//!    Bearer …` and, for ad-account-scoped calls, `X-AP-Context:
//!    adAccountId={id}`, the ID coming from `GET /v1/acls`.
//!
//! Popularity comes from `POST /v1/suggestions/phrases/query` with
//! `queryType EQUALS SEARCH` and `phrase IN [...]`.

mod model;

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use appdoctor_jwt::EcKey;
use reqwest::{Method, StatusCode, header::HeaderMap};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub use appdoctor_jwt::EcKey as PrivateKey;
pub use model::*;

pub const TOKEN_URL: &str = "https://appleid.apple.com/auth/oauth2/token";
pub const API_BASE: &str = "https://api.ads.apple.com/v1";
const AUDIENCE: &str = "https://appleid.apple.com";
const SCOPE: &str = "searchadsorg";
/// Apple's ceiling for a client secret's lifetime.
pub const MAX_CLIENT_SECRET_LIFETIME: Duration = Duration::from_secs(180 * 24 * 60 * 60);
/// AppDoctor holds the private key, so it mints a fresh short-lived secret
/// for every token exchange instead of storing a 180-day one.
const CLIENT_SECRET_LIFETIME: Duration = Duration::from_secs(24 * 60 * 60);
/// Phrases per popularity request.
// TODO(apple-ads): Apple doesn't document a cap on `phrase IN`; 50 is a
// conservative guess until we measure it.
const PHRASE_BATCH: usize = 50;
const MAX_RETRIES: u32 = 5;
const MAX_BACKOFF: Duration = Duration::from_secs(16);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Key(#[from] appdoctor_jwt::Error),
    #[error("Apple rejected the Apple Ads credentials: {0}")]
    Auth(String),
    #[error("this Apple Ads user has no ad accounts")]
    NoAdAccount,
    #[error("Apple Ads: {0}")]
    Api(String),
    #[error("Apple Ads returned HTTP {0}")]
    Status(u16),
    #[error("Apple Ads is rate-limiting requests; try again shortly")]
    RateLimited,
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Decode(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Generates the key pair the user registers with Apple Ads.
pub fn generate_key() -> EcKey {
    EcKey::generate()
}

/// The IDs Apple Ads shows after the public key is uploaded, plus the
/// private key that matches it.
#[derive(Clone)]
pub struct Credentials {
    pub client_id: String,
    pub team_id: String,
    pub key_id: String,
    key: EcKey,
}

impl Credentials {
    pub fn new(client_id: &str, team_id: &str, key_id: &str, key: EcKey) -> Self {
        Self {
            client_id: client_id.trim().to_owned(),
            team_id: team_id.trim().to_owned(),
            key_id: key_id.trim().to_owned(),
            key,
        }
    }

    /// The client secret JWT: ES256, `kid` = key ID, `iss` = team ID,
    /// `sub` = client ID, `aud` = `https://appleid.apple.com`, expiring
    /// `lifetime` after `now` (capped just under Apple's 180 days).
    pub fn client_secret(&self, now: SystemTime, lifetime: Duration) -> Result<String> {
        let iat = now.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let lifetime = lifetime.min(MAX_CLIENT_SECRET_LIFETIME - Duration::from_secs(60));
        Ok(self.key.sign(
            &json!({"alg": "ES256", "kid": self.key_id}),
            &json!({
                "sub": self.client_id,
                "aud": AUDIENCE,
                "iat": iat,
                "exp": iat + lifetime.as_secs(),
                "iss": self.team_id,
            }),
        )?)
    }
}

pub struct Client {
    http: reqwest::Client,
    credentials: Credentials,
    token_url: String,
    api_base: String,
    access_token: Mutex<Option<(String, SystemTime)>>,
    ad_account_id: Mutex<Option<i64>>,
}

impl Client {
    pub fn new(credentials: Credentials) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("AppDoctor/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client builds with static config"),
            credentials,
            token_url: TOKEN_URL.into(),
            api_base: API_BASE.into(),
            access_token: Mutex::new(None),
            ad_account_id: Mutex::new(None),
        }
    }

    pub fn with_urls(mut self, token_url: impl Into<String>, api_base: impl Into<String>) -> Self {
        self.token_url = token_url.into();
        self.api_base = api_base.into();
        self
    }

    /// Scopes later requests to this ad account instead of the first one
    /// `GET /v1/acls` returns.
    pub fn set_ad_account(&self, id: i64) {
        *self.ad_account_id.lock().unwrap() = Some(id);
    }

    /// Exchanges a fresh client secret for an access token (valid an hour),
    /// reusing the current one until a minute before it expires.
    pub async fn access_token(&self) -> Result<String> {
        let now = SystemTime::now();
        if let Some((token, expires)) = self.access_token.lock().unwrap().clone()
            && expires > now + Duration::from_secs(60)
        {
            return Ok(token);
        }
        let secret = self.credentials.client_secret(now, CLIENT_SECRET_LIFETIME)?;
        let response = self
            .http
            .post(&self.token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", secret.as_str()),
                ("scope", SCOPE),
            ])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            let reason = serde_json::from_str::<Value>(&body)
                .ok()
                .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
                .unwrap_or_else(|| format!("HTTP {}", status.as_u16()));
            return Err(Error::Auth(reason));
        }
        let token: TokenResponse = serde_json::from_str(&body)?;
        let expires = now + Duration::from_secs(token.expires_in.unwrap_or(3600));
        *self.access_token.lock().unwrap() = Some((token.access_token.clone(), expires));
        Ok(token.access_token)
    }

    /// The ad accounts this API user can access, with their roles.
    pub async fn acls(&self) -> Result<Vec<Acl>> {
        let response: Envelope<AclList> = self.send(Method::GET, "/acls", None, None).await?;
        Ok(response.result.map(|list| list.acls).unwrap_or_default())
    }

    async fn ad_account(&self) -> Result<i64> {
        if let Some(id) = *self.ad_account_id.lock().unwrap() {
            return Ok(id);
        }
        let id = self
            .acls()
            .await?
            .first()
            .map(|acl| acl.ad_account.id)
            .ok_or(Error::NoAdAccount)?;
        self.set_ad_account(id);
        Ok(id)
    }

    /// Search popularity (0–100) for specific phrases.
    ///
    /// Phrases Apple has no data for are missing from the result.
    // TODO(apple-ads): the SEARCH route documents no country/region filter,
    // so it's unverified which storefront these scores describe (likely US
    // or global). `POST /v1/insights/apps/search-term-popularity/query` is
    // country-scoped and may be the better source per country; verify both
    // against a real account.
    pub async fn phrase_popularity(&self, phrases: &[String]) -> Result<Vec<PhrasePopularity>> {
        let context = self.context().await?;
        let mut all = Vec::with_capacity(phrases.len());
        for batch in phrases.chunks(PHRASE_BATCH) {
            let body = phrase_search_query(batch);
            let response: Envelope<Vec<PhrasePopularity>> = self
                .send(Method::POST, "/suggestions/phrases/query", Some(body), Some(&context))
                .await?;
            all.extend(response.result.unwrap_or_default());
        }
        Ok(all)
    }

    /// Keyword ideas for an app, scoped to App Store countries.
    pub async fn keyword_suggestions(
        &self,
        app_id: u64,
        countries: &[String],
        seed_terms: &[String],
    ) -> Result<Vec<KeywordSuggestion>> {
        let context = self.context().await?;
        let body = keyword_suggestions_query(app_id, countries, seed_terms);
        let response: Envelope<Vec<KeywordSuggestion>> = self
            .send(Method::POST, "/suggestions/keywords/query", Some(body), Some(&context))
            .await?;
        Ok(response.result.unwrap_or_default())
    }

    /// The `X-AP-Context` value for ad-account-scoped requests.
    async fn context(&self) -> Result<String> {
        Ok(format!("adAccountId={}", self.ad_account().await?))
    }

    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        context: Option<&str>,
    ) -> Result<T> {
        let url = format!("{}{path}", self.api_base);
        let mut wait = Duration::from_secs(2);
        for _ in 0..MAX_RETRIES {
            let mut request = self
                .http
                .request(method.clone(), &url)
                .bearer_auth(self.access_token().await?);
            if let Some(context) = context {
                request = request.header("X-AP-Context", context);
            }
            if let Some(body) = &body {
                request = request.json(body);
            }
            let response = request.send().await?;
            let status = response.status();
            let headers = response.headers().clone();
            let text = response.text().await?;

            if status == StatusCode::TOO_MANY_REQUESTS {
                let delay = header_secs(&headers, "retry-after")
                    .or_else(|| header_secs(&headers, "ratelimit-reset"))
                    .unwrap_or(wait);
                tokio::time::sleep(delay.min(MAX_BACKOFF)).await;
                wait = (wait * 2).min(MAX_BACKOFF);
                continue;
            }
            if status == StatusCode::UNAUTHORIZED {
                *self.access_token.lock().unwrap() = None;
            }
            if !status.is_success() {
                return Err(api_error(status, &text));
            }
            // Pace proactively, as Apple asks, when the window is nearly spent.
            if let (Some(remaining), Some(reset)) = (
                header_u64(&headers, "ratelimit-remaining"),
                header_secs(&headers, "ratelimit-reset"),
            ) && remaining < 5
            {
                tokio::time::sleep(reset.min(MAX_BACKOFF)).await;
            }
            return Ok(serde_json::from_str(&text)?);
        }
        Err(Error::RateLimited)
    }
}

fn header_u64(headers: &HeaderMap, name: &str) -> Option<u64> {
    headers.get(name)?.to_str().ok()?.trim().parse().ok()
}

fn header_secs(headers: &HeaderMap, name: &str) -> Option<Duration> {
    header_u64(headers, name).map(Duration::from_secs)
}

pub(crate) fn phrase_search_query(phrases: &[String]) -> Value {
    json!({
        "filters": [
            {"field": "queryType", "operator": "EQUALS", "value": ["SEARCH"]},
            {"field": "phrase", "operator": "IN", "value": phrases},
        ],
        "pagination": {"offset": 0, "pageSize": phrases.len().max(1)},
    })
}

pub(crate) fn keyword_suggestions_query(app_id: u64, countries: &[String], seed_terms: &[String]) -> Value {
    let mut filters = vec![
        json!({"field": "promotedObjectId", "operator": "EQUALS", "value": [app_id.to_string()]}),
        json!({"field": "promotedObjectType", "operator": "EQUALS", "value": ["APPSTORE_APP"]}),
    ];
    if !seed_terms.is_empty() {
        filters.push(json!({"field": "terms", "operator": "IN", "value": seed_terms}));
    }
    if !countries.is_empty() {
        let countries: Vec<String> = countries.iter().map(|c| c.to_ascii_uppercase()).collect();
        filters.push(json!({"field": "countriesOrRegions", "operator": "IN", "value": countries}));
    }
    json!({
        "filters": filters,
        "sorting": [{"field": "popularity", "order": "DESC"}],
        "pagination": {"offset": 0, "pageSize": 100},
    })
}

fn api_error(status: StatusCode, body: &str) -> Error {
    match serde_json::from_str::<Envelope<Value>>(body) {
        Ok(Envelope { error: Some(error), .. }) => {
            let details: Vec<String> = error.details.iter().filter_map(|d| d.message.clone()).collect();
            let message = error.message.unwrap_or(error.code);
            if details.is_empty() {
                Error::Api(message)
            } else {
                Error::Api(format!("{message}: {}", details.join("; ")))
            }
        }
        _ => Error::Status(status.as_u16()),
    }
}

#[cfg(test)]
mod tests;
