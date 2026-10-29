//! App Store Connect's JSON:API documents, and the plain types the client
//! returns instead of them.

use serde::{Deserialize, Serialize};

use crate::skip_none;
use crate::submission::ReleaseType;

#[derive(Debug, Deserialize)]
pub(crate) struct Document<T> {
    pub data: T,
    pub links: Option<Links>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Links {
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Resource<A> {
    pub id: String,
    pub attributes: A,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Platform {
    Ios,
    MacOs,
    TvOs,
    VisionOs,
    #[serde(other)]
    Other,
}

// Apps

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppAttributes {
    name: Option<String>,
    bundle_id: Option<String>,
    sku: Option<String>,
    primary_locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// Also the App Store (Adam) ID.
    pub id: String,
    pub name: String,
    pub bundle_id: String,
    pub sku: Option<String>,
    pub primary_locale: Option<String>,
}

impl From<Resource<AppAttributes>> for App {
    fn from(resource: Resource<AppAttributes>) -> Self {
        let a = resource.attributes;
        Self {
            id: resource.id,
            name: a.name.unwrap_or_default(),
            bundle_id: a.bundle_id.unwrap_or_default(),
            sku: a.sku,
            primary_locale: a.primary_locale,
        }
    }
}

// App infos

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfoAttributes {
    /// Current name; `appStoreState` is the deprecated spelling.
    state: Option<String>,
    app_store_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub id: String,
    pub state: Option<String>,
}

impl AppInfo {
    /// Name and subtitle can change only on the app info that isn't live yet.
    pub fn is_editable(&self) -> bool {
        self.state.as_deref().is_some_and(|state| {
            matches!(
                state,
                "PREPARE_FOR_SUBMISSION" | "DEVELOPER_REJECTED" | "REJECTED" | "METADATA_REJECTED"
            )
        })
    }
}

impl From<Resource<AppInfoAttributes>> for AppInfo {
    fn from(resource: Resource<AppInfoAttributes>) -> Self {
        Self {
            id: resource.id,
            state: resource.attributes.state.or(resource.attributes.app_store_state),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppInfoLocalizationAttributes {
    locale: String,
    name: Option<String>,
    subtitle: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppInfoLocalization {
    pub id: String,
    pub locale: String,
    pub name: Option<String>,
    pub subtitle: Option<String>,
}

impl From<Resource<AppInfoLocalizationAttributes>> for AppInfoLocalization {
    fn from(resource: Resource<AppInfoLocalizationAttributes>) -> Self {
        Self {
            id: resource.id,
            locale: resource.attributes.locale,
            name: resource.attributes.name,
            subtitle: resource.attributes.subtitle,
        }
    }
}

/// App name (≤30 characters) and subtitle (≤30).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AppInfoLocalizationUpdate {
    #[serde(skip_serializing_if = "skip_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "skip_none")]
    pub subtitle: Option<String>,
}

// App Store versions

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppStoreVersionAttributes {
    platform: Option<Platform>,
    version_string: Option<String>,
    /// Current name; `appStoreState` is the deprecated spelling.
    app_version_state: Option<String>,
    app_store_state: Option<String>,
    release_type: Option<ReleaseType>,
    earliest_release_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppStoreVersion {
    pub id: String,
    pub platform: Option<Platform>,
    pub version: Option<String>,
    pub state: Option<String>,
    pub release_type: Option<ReleaseType>,
    pub earliest_release_date: Option<String>,
}

impl AppStoreVersion {
    /// Keywords and description can change while the version is being
    /// prepared or after a rejection. Promotional text can change on a live
    /// version too, but this client only edits versions in these states.
    pub fn is_editable(&self) -> bool {
        self.state.as_deref().is_some_and(|state| {
            matches!(
                state,
                "PREPARE_FOR_SUBMISSION" | "DEVELOPER_REJECTED" | "REJECTED" | "METADATA_REJECTED"
            )
        })
    }
}

impl From<Resource<AppStoreVersionAttributes>> for AppStoreVersion {
    fn from(resource: Resource<AppStoreVersionAttributes>) -> Self {
        let a = resource.attributes;
        Self {
            id: resource.id,
            platform: a.platform,
            version: a.version_string,
            state: a.app_version_state.or(a.app_store_state),
            release_type: a.release_type,
            earliest_release_date: a.earliest_release_date,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VersionLocalizationAttributes {
    locale: String,
    keywords: Option<String>,
    promotional_text: Option<String>,
    description: Option<String>,
    whats_new: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VersionLocalization {
    pub id: String,
    pub locale: String,
    pub keywords: Option<String>,
    pub promotional_text: Option<String>,
    pub description: Option<String>,
    pub whats_new: Option<String>,
}

impl From<Resource<VersionLocalizationAttributes>> for VersionLocalization {
    fn from(resource: Resource<VersionLocalizationAttributes>) -> Self {
        let a = resource.attributes;
        Self {
            id: resource.id,
            locale: a.locale,
            keywords: a.keywords,
            promotional_text: a.promotional_text,
            description: a.description,
            whats_new: a.whats_new,
        }
    }
}

/// Keywords (≤100 characters, comma-separated), promotional text (≤170),
/// description (≤4000), what's new (≤4000).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionLocalizationUpdate {
    #[serde(skip_serializing_if = "skip_none")]
    pub keywords: Option<String>,
    #[serde(skip_serializing_if = "skip_none")]
    pub promotional_text: Option<String>,
    #[serde(skip_serializing_if = "skip_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "skip_none")]
    pub whats_new: Option<String>,
}
