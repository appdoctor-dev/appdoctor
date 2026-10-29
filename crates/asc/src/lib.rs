//! App Store Connect API client.
//!
//! Authenticates with an ES256 JWT signed locally from the user's own API
//! key (`.p8` file, Issuer ID, Key ID); the key never leaves the machine.
//!
//! What it covers:
//! - Metadata ASO touches: app-info localizations (name, subtitle) and
//!   app-store-version localizations (keywords, promotional text,
//!   description, what's new).
//! - Releasing: builds, attaching a build to an editable version, release
//!   type, uploading a build ([`upload`]), and App Review submissions
//!   ([`submission`]).
//!
//! Submitting for review is outward-facing and hard to undo, so it is never
//! a side effect: [`Client::prepare_submission`] only reads and returns a
//! [`SubmissionPlan`] to show the user, and [`Client::submit_for_review`]
//! and [`Client::cancel_submission`] refuse to run unless the caller passes
//! `confirmed: true` after showing that summary. The UI and the MCP tools
//! both go through these two calls.

mod model;
pub mod submission;
pub mod upload;

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use appdoctor_jwt::EcKey;
use reqwest::{Method, StatusCode};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

pub use model::*;
pub use submission::{ReleaseType, ReviewSubmission, ReviewSubmissionState, SubmissionPlan};

const API_BASE: &str = "https://api.appstoreconnect.apple.com";
const AUDIENCE: &str = "appstoreconnect-v1";
/// Apple rejects tokens that live longer than 20 minutes.
pub const MAX_TOKEN_LIFETIME: Duration = Duration::from_secs(20 * 60);
const TOKEN_LIFETIME: Duration = Duration::from_secs(19 * 60);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Key(#[from] appdoctor_jwt::Error),
    #[error("App Store Connect rejected the API key (check the Issuer ID, Key ID and .p8 file)")]
    Unauthorized,
    #[error("App Store Connect: {0}")]
    Api(String),
    #[error("App Store Connect returned HTTP {0}")]
    Status(u16),
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("not confirmed: show the user the submission summary and pass `confirmed: true`")]
    NotConfirmed,
    #[error("no editable App Store version: create a new version in App Store Connect first")]
    NoEditableVersion,
    #[error("the submission changed since it was confirmed: {0}")]
    PlanOutdated(String),
    #[error("build {0} isn't ready for review (processing state {1})")]
    BuildNotReady(String, String),
    #[error("couldn't read the build file: {0}")]
    File(#[from] std::io::Error),
    #[error("upload failed: {0}")]
    Upload(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// An App Store Connect API key, as the user downloads it from
/// Users and Access → Integrations → App Store Connect API.
#[derive(Clone)]
pub struct Credentials {
    pub issuer_id: String,
    pub key_id: String,
    key: EcKey,
}

impl Credentials {
    pub fn new(issuer_id: impl Into<String>, key_id: impl Into<String>, p8_pem: &str) -> Result<Self> {
        Ok(Self {
            issuer_id: issuer_id.into().trim().to_owned(),
            key_id: key_id.into().trim().to_owned(),
            key: EcKey::from_pem(p8_pem)?,
        })
    }

    /// A signed API token valid from `now` for `lifetime` (at most 20 minutes).
    pub fn token(&self, now: SystemTime, lifetime: Duration) -> Result<String> {
        let iat = unix(now);
        let exp = iat + lifetime.min(MAX_TOKEN_LIFETIME).as_secs();
        Ok(self.key.sign(
            &json!({"alg": "ES256", "kid": self.key_id, "typ": "JWT"}),
            &json!({"iss": self.issuer_id, "iat": iat, "exp": exp, "aud": AUDIENCE}),
        )?)
    }
}

fn unix(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}

pub struct Client {
    http: reqwest::Client,
    credentials: Credentials,
    base: String,
    cached_token: Mutex<Option<(String, SystemTime)>>,
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
            base: API_BASE.into(),
            cached_token: Mutex::new(None),
        }
    }

    pub fn with_base_url(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    fn token(&self) -> Result<String> {
        let now = SystemTime::now();
        let mut cached = self.cached_token.lock().unwrap();
        if let Some((token, expires)) = cached.as_ref()
            && *expires > now + Duration::from_secs(60)
        {
            return Ok(token.clone());
        }
        let token = self.credentials.token(now, TOKEN_LIFETIME)?;
        *cached = Some((token.clone(), now + TOKEN_LIFETIME));
        Ok(token)
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path_or_url: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let text = self.send(method, path_or_url, body).await?;
        Ok(serde_json::from_str(&text)?)
    }

    /// For endpoints that answer `204 No Content`.
    async fn request_empty(&self, method: Method, path: &str, body: Option<serde_json::Value>) -> Result<()> {
        self.send(method, path, body).await.map(drop)
    }

    async fn send(&self, method: Method, path_or_url: &str, body: Option<serde_json::Value>) -> Result<String> {
        let url = if path_or_url.starts_with("http") {
            path_or_url.to_owned()
        } else {
            format!("{}{path_or_url}", self.base)
        };
        let mut request = self.http.request(method, &url).bearer_auth(self.token()?);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await?;
        let status = response.status();
        let text = response.text().await?;
        if status == StatusCode::UNAUTHORIZED {
            return Err(Error::Unauthorized);
        }
        if !status.is_success() {
            return Err(api_error(status, &text));
        }
        Ok(text)
    }

    async fn get_all<A: DeserializeOwned>(&self, path: &str) -> Result<Vec<Resource<A>>> {
        let mut next = Some(path.to_owned());
        let mut all = Vec::new();
        while let Some(url) = next {
            let page: Document<Vec<Resource<A>>> = self.request(Method::GET, &url, None).await?;
            all.extend(page.data);
            next = page.links.and_then(|links| links.next);
        }
        Ok(all)
    }

    /// Every app the key can see.
    pub async fn apps(&self) -> Result<Vec<App>> {
        let resources = self
            .get_all::<AppAttributes>("/v1/apps?fields[apps]=name,bundleId,sku,primaryLocale&limit=200")
            .await?;
        Ok(resources.into_iter().map(App::from).collect())
    }

    pub async fn app_infos(&self, app_id: &str) -> Result<Vec<AppInfo>> {
        let resources = self
            .get_all::<AppInfoAttributes>(&format!("/v1/apps/{app_id}/appInfos?limit=50"))
            .await?;
        Ok(resources.into_iter().map(AppInfo::from).collect())
    }

    /// The app info a new version's metadata goes on: the one not yet live.
    pub async fn editable_app_info(&self, app_id: &str) -> Result<Option<AppInfo>> {
        Ok(self.app_infos(app_id).await?.into_iter().find(AppInfo::is_editable))
    }

    pub async fn app_info_localizations(&self, app_info_id: &str) -> Result<Vec<AppInfoLocalization>> {
        let resources = self
            .get_all::<AppInfoLocalizationAttributes>(&format!(
                "/v1/appInfos/{app_info_id}/appInfoLocalizations?limit=200"
            ))
            .await?;
        Ok(resources.into_iter().map(AppInfoLocalization::from).collect())
    }

    /// Updates name and/or subtitle; `None` fields are left unchanged.
    pub async fn update_app_info_localization(
        &self,
        id: &str,
        update: &AppInfoLocalizationUpdate,
    ) -> Result<AppInfoLocalization> {
        let body = json!({"data": {"type": "appInfoLocalizations", "id": id, "attributes": update}});
        let document: Document<Resource<AppInfoLocalizationAttributes>> = self
            .request(Method::PATCH, &format!("/v1/appInfoLocalizations/{id}"), Some(body))
            .await?;
        Ok(document.data.into())
    }

    pub async fn create_app_info_localization(
        &self,
        app_info_id: &str,
        locale: &str,
        fields: &AppInfoLocalizationUpdate,
    ) -> Result<AppInfoLocalization> {
        let mut attributes = serde_json::to_value(fields)?;
        attributes["locale"] = locale.into();
        let body = json!({"data": {
            "type": "appInfoLocalizations",
            "attributes": attributes,
            "relationships": {"appInfo": {"data": {"type": "appInfos", "id": app_info_id}}}
        }});
        let document: Document<Resource<AppInfoLocalizationAttributes>> = self
            .request(Method::POST, "/v1/appInfoLocalizations", Some(body))
            .await?;
        Ok(document.data.into())
    }

    pub async fn app_store_versions(&self, app_id: &str) -> Result<Vec<AppStoreVersion>> {
        let resources = self
            .get_all::<AppStoreVersionAttributes>(&format!("/v1/apps/{app_id}/appStoreVersions?limit=50"))
            .await?;
        Ok(resources.into_iter().map(AppStoreVersion::from).collect())
    }

    /// The version whose metadata can still change (being prepared or rejected).
    pub async fn editable_version(&self, app_id: &str, platform: Platform) -> Result<AppStoreVersion> {
        self.app_store_versions(app_id)
            .await?
            .into_iter()
            .find(|version| version.platform == Some(platform) && version.is_editable())
            .ok_or(Error::NoEditableVersion)
    }

    pub async fn version_localizations(&self, version_id: &str) -> Result<Vec<VersionLocalization>> {
        let resources = self
            .get_all::<VersionLocalizationAttributes>(&format!(
                "/v1/appStoreVersions/{version_id}/appStoreVersionLocalizations?limit=200"
            ))
            .await?;
        Ok(resources.into_iter().map(VersionLocalization::from).collect())
    }

    /// Updates keywords, promotional text, description or what's new;
    /// `None` fields are left unchanged.
    pub async fn update_version_localization(
        &self,
        id: &str,
        update: &VersionLocalizationUpdate,
    ) -> Result<VersionLocalization> {
        let body = json!({"data": {"type": "appStoreVersionLocalizations", "id": id, "attributes": update}});
        let document: Document<Resource<VersionLocalizationAttributes>> = self
            .request(
                Method::PATCH,
                &format!("/v1/appStoreVersionLocalizations/{id}"),
                Some(body),
            )
            .await?;
        Ok(document.data.into())
    }

    pub async fn create_version_localization(
        &self,
        version_id: &str,
        locale: &str,
        fields: &VersionLocalizationUpdate,
    ) -> Result<VersionLocalization> {
        let mut attributes = serde_json::to_value(fields)?;
        attributes["locale"] = locale.into();
        let body = json!({"data": {
            "type": "appStoreVersionLocalizations",
            "attributes": attributes,
            "relationships": {"appStoreVersion": {"data": {"type": "appStoreVersions", "id": version_id}}}
        }});
        let document: Document<Resource<VersionLocalizationAttributes>> = self
            .request(Method::POST, "/v1/appStoreVersionLocalizations", Some(body))
            .await?;
        Ok(document.data.into())
    }
}

fn api_error(status: StatusCode, body: &str) -> Error {
    #[derive(serde::Deserialize)]
    struct Errors {
        errors: Vec<ApiError>,
    }
    #[derive(serde::Deserialize)]
    struct ApiError {
        title: Option<String>,
        detail: Option<String>,
    }
    match serde_json::from_str::<Errors>(body) {
        Ok(errors) if !errors.errors.is_empty() => Error::Api(
            errors
                .errors
                .into_iter()
                .map(|e| e.detail.or(e.title).unwrap_or_default())
                .collect::<Vec<_>>()
                .join("; "),
        ),
        _ => Error::Status(status.as_u16()),
    }
}

/// Attribute patches serialize only the fields that are set.
fn skip_none<T: Serialize>(value: &Option<T>) -> bool {
    value.is_none()
}

#[cfg(test)]
mod tests;
