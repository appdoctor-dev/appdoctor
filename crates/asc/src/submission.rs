//! App Review: builds, attaching one to a version, release type, and
//! review submissions (`reviewSubmissions` + `reviewSubmissionItems`).
//!
//! Submitting goes in two steps so a person always sees what will happen:
//!
//! 1. [`Client::prepare_submission`] reads the version and build, checks
//!    they can be submitted, and returns a [`SubmissionPlan`]. Nothing changes.
//! 2. After showing [`SubmissionPlan::summary`], the caller passes the plan
//!    and `confirmed: true` to [`Client::submit_for_review`], which checks
//!    again that nothing changed, attaches the build, sets the release type,
//!    adds the version to an open review submission and submits it.

use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::model::{AppStoreVersionAttributes, Document, Links, Resource};
use crate::{AppStoreVersion, Client, Error, Platform, Result};

/// When an approved version goes live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseType {
    /// The developer releases it by hand after approval.
    Manual,
    /// Automatically, as soon as App Review approves it.
    AfterApproval,
    /// Automatically, on or after `earliestReleaseDate`.
    Scheduled,
}

impl ReleaseType {
    pub fn describe(self) -> &'static str {
        match self {
            ReleaseType::Manual => "Release manually after approval",
            ReleaseType::AfterApproval => "Release automatically when approved",
            ReleaseType::Scheduled => "Release automatically on the scheduled date",
        }
    }
}

// Builds

/// An uploaded build.
#[derive(Debug, Clone, PartialEq)]
pub struct Build {
    pub id: String,
    /// `CFBundleVersion`, e.g. "1234".
    pub number: String,
    /// `CFBundleShortVersionString`, e.g. "3.22", from the pre-release version.
    pub version: Option<String>,
    /// `PROCESSING`, `FAILED`, `INVALID` or `VALID`.
    pub processing_state: String,
    pub uploaded_date: Option<String>,
    pub expired: bool,
    /// `None` until export compliance is answered; App Review needs it.
    pub uses_non_exempt_encryption: Option<bool>,
}

impl Build {
    pub fn is_ready(&self) -> bool {
        self.processing_state == "VALID" && !self.expired
    }

    /// "3.22 (1234)".
    pub fn label(&self) -> String {
        match &self.version {
            Some(version) => format!("{version} ({})", self.number),
            None => self.number.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildAttributes {
    version: Option<String>,
    processing_state: Option<String>,
    uploaded_date: Option<String>,
    #[serde(default)]
    expired: bool,
    uses_non_exempt_encryption: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ResourceId {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToOne {
    pub data: Option<ResourceId>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BuildRelationships {
    pre_release_version: Option<ToOne>,
}

#[derive(Debug, Deserialize)]
struct BuildResource {
    id: String,
    attributes: BuildAttributes,
    relationships: Option<BuildRelationships>,
}

#[derive(Debug, Deserialize)]
struct Included {
    #[serde(rename = "type")]
    kind: String,
    id: String,
    #[serde(default)]
    attributes: Value,
}

#[derive(Debug, Deserialize)]
struct BuildsDocument {
    data: Vec<BuildResource>,
    #[serde(default)]
    included: Vec<Included>,
    links: Option<Links>,
}

fn build_from(resource: BuildResource, included: &[Included]) -> Build {
    let version = resource
        .relationships
        .and_then(|r| r.pre_release_version)
        .and_then(|link| link.data)
        .and_then(|link| {
            included
                .iter()
                .find(|inc| inc.kind == "preReleaseVersions" && inc.id == link.id)
        })
        .and_then(|inc| inc.attributes.get("version").and_then(Value::as_str).map(str::to_owned));
    let a = resource.attributes;
    Build {
        id: resource.id,
        number: a.version.unwrap_or_default(),
        version,
        processing_state: a.processing_state.unwrap_or_else(|| "UNKNOWN".into()),
        uploaded_date: a.uploaded_date,
        expired: a.expired,
        uses_non_exempt_encryption: a.uses_non_exempt_encryption,
    }
}

pub(crate) fn parse_builds(body: &str) -> Result<(Vec<Build>, Option<String>)> {
    let document: BuildsDocument = serde_json::from_str(body)?;
    let included = document.included;
    let builds = document.data.into_iter().map(|r| build_from(r, &included)).collect();
    Ok((builds, document.links.and_then(|links| links.next)))
}

#[derive(Debug, Deserialize)]
struct BuildDocument {
    data: Option<BuildResource>,
    #[serde(default)]
    included: Vec<Included>,
}

pub(crate) fn parse_build(body: &str) -> Result<Option<Build>> {
    let document: BuildDocument = serde_json::from_str(body)?;
    Ok(document.data.map(|r| build_from(r, &document.included)))
}

// Review submissions

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewSubmissionState {
    /// Being assembled; not sent yet.
    ReadyForReview,
    WaitingForReview,
    InReview,
    UnresolvedIssues,
    Canceling,
    Completing,
    Complete,
    #[serde(other)]
    Unknown,
}

impl ReviewSubmissionState {
    /// Whether the submission can be withdrawn from review.
    pub fn can_cancel(self) -> bool {
        matches!(
            self,
            Self::ReadyForReview | Self::WaitingForReview | Self::InReview | Self::UnresolvedIssues
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ReadyForReview => "Draft",
            Self::WaitingForReview => "Waiting for review",
            Self::InReview => "In review",
            Self::UnresolvedIssues => "Unresolved issues",
            Self::Canceling => "Canceling",
            Self::Completing => "Completing",
            Self::Complete => "Complete",
            Self::Unknown => "Unknown",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReviewSubmissionAttributes {
    platform: Option<Platform>,
    state: Option<ReviewSubmissionState>,
    submitted_date: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReviewSubmission {
    pub id: String,
    pub platform: Option<Platform>,
    pub state: ReviewSubmissionState,
    pub submitted_date: Option<String>,
}

impl From<Resource<ReviewSubmissionAttributes>> for ReviewSubmission {
    fn from(resource: Resource<ReviewSubmissionAttributes>) -> Self {
        let a = resource.attributes;
        Self {
            id: resource.id,
            platform: a.platform,
            state: a.state.unwrap_or(ReviewSubmissionState::Unknown),
            submitted_date: a.submitted_date,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemRelationships {
    app_store_version: Option<ToOne>,
}

#[derive(Debug, Deserialize)]
struct ItemResource {
    relationships: Option<ItemRelationships>,
}

#[derive(Debug, Deserialize)]
struct ItemsDocument {
    data: Vec<ItemResource>,
}

/// The App Store version IDs already in a submission.
pub(crate) fn parse_item_versions(body: &str) -> Result<Vec<String>> {
    let document: ItemsDocument = serde_json::from_str(body)?;
    Ok(document
        .data
        .into_iter()
        .filter_map(|item| item.relationships?.app_store_version?.data)
        .map(|link| link.id)
        .collect())
}

/// Everything a person should see before a version goes to App Review.
#[derive(Debug, Clone, PartialEq)]
pub struct SubmissionPlan {
    pub app_id: String,
    pub app_name: String,
    pub platform: Platform,
    pub version_id: String,
    pub version: String,
    pub version_state: String,
    pub build_id: String,
    pub build: String,
    pub release_type: ReleaseType,
    pub earliest_release_date: Option<String>,
    pub uses_non_exempt_encryption: Option<bool>,
}

impl SubmissionPlan {
    /// Label/value rows for a confirmation screen.
    pub fn summary_rows(&self) -> Vec<(&'static str, String)> {
        let mut rows = vec![
            ("App", format!("{} ({})", self.app_name, self.app_id)),
            ("Platform", format!("{:?}", self.platform)),
            ("Version", format!("{} ({})", self.version, self.version_state)),
            ("Build", self.build.clone()),
            ("Release", self.release_type.describe().to_owned()),
        ];
        if let Some(date) = &self.earliest_release_date {
            rows.push(("Release date", date.clone()));
        }
        rows.push((
            "Encryption",
            match self.uses_non_exempt_encryption {
                Some(true) => "Uses non-exempt encryption".to_owned(),
                Some(false) => "No non-exempt encryption".to_owned(),
                None => "Export compliance not answered yet".to_owned(),
            },
        ));
        rows
    }

    /// The summary as plain text, for agents to show the user verbatim.
    pub fn summary(&self) -> String {
        self.summary_rows()
            .into_iter()
            .map(|(label, value)| format!("{label}: {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// Request bodies, separate so tests can check them without the network.

pub(crate) fn build_linkage_body(build_id: &str) -> Value {
    json!({"data": {"type": "builds", "id": build_id}})
}

pub(crate) fn release_type_body(version_id: &str, release_type: ReleaseType, earliest: Option<&str>) -> Value {
    let mut attributes = json!({"releaseType": release_type});
    if let Some(date) = earliest {
        attributes["earliestReleaseDate"] = date.into();
    }
    json!({"data": {"type": "appStoreVersions", "id": version_id, "attributes": attributes}})
}

pub(crate) fn create_submission_body(app_id: &str, platform: Platform) -> Value {
    json!({"data": {
        "type": "reviewSubmissions",
        "attributes": {"platform": platform},
        "relationships": {"app": {"data": {"type": "apps", "id": app_id}}}
    }})
}

pub(crate) fn submission_item_body(submission_id: &str, version_id: &str) -> Value {
    json!({"data": {
        "type": "reviewSubmissionItems",
        "relationships": {
            "reviewSubmission": {"data": {"type": "reviewSubmissions", "id": submission_id}},
            "appStoreVersion": {"data": {"type": "appStoreVersions", "id": version_id}}
        }
    }})
}

pub(crate) fn submit_body(submission_id: &str) -> Value {
    json!({"data": {"type": "reviewSubmissions", "id": submission_id, "attributes": {"submitted": true}}})
}

pub(crate) fn cancel_body(submission_id: &str) -> Value {
    json!({"data": {"type": "reviewSubmissions", "id": submission_id, "attributes": {"canceled": true}}})
}

const BUILD_INCLUDE: &str = "include=preReleaseVersion&fields[preReleaseVersions]=version,platform";

impl Client {
    /// The app's builds, newest first.
    pub async fn builds(&self, app_id: &str) -> Result<Vec<Build>> {
        let mut next = Some(format!(
            "/v1/builds?filter[app]={app_id}&sort=-uploadedDate&limit=50&{BUILD_INCLUDE}"
        ));
        let mut all = Vec::new();
        // The newest 100 are plenty for picking a build to submit.
        while let Some(url) = next.take()
            && all.len() < 100
        {
            let (builds, more) = parse_builds(&self.send(Method::GET, &url, None).await?)?;
            all.extend(builds);
            next = more;
        }
        Ok(all)
    }

    pub async fn build(&self, build_id: &str) -> Result<Build> {
        parse_build(
            &self
                .send(Method::GET, &format!("/v1/builds/{build_id}?{BUILD_INCLUDE}"), None)
                .await?,
        )?
        .ok_or_else(|| Error::Api(format!("build {build_id} not found")))
    }

    /// The build attached to a version, if any.
    pub async fn version_build(&self, version_id: &str) -> Result<Option<Build>> {
        parse_build(
            &self
                .send(Method::GET, &format!("/v1/appStoreVersions/{version_id}/build"), None)
                .await?,
        )
    }

    pub async fn app_store_version(&self, version_id: &str) -> Result<AppStoreVersion> {
        let document: Document<Resource<AppStoreVersionAttributes>> = self
            .request(Method::GET, &format!("/v1/appStoreVersions/{version_id}"), None)
            .await?;
        Ok(document.data.into())
    }

    /// Selects the build a version ships. Reversible until the version is submitted.
    pub async fn attach_build(&self, version_id: &str, build_id: &str) -> Result<()> {
        self.request_empty(
            Method::PATCH,
            &format!("/v1/appStoreVersions/{version_id}/relationships/build"),
            Some(build_linkage_body(build_id)),
        )
        .await
    }

    pub async fn set_release_type(
        &self,
        version_id: &str,
        release_type: ReleaseType,
        earliest: Option<&str>,
    ) -> Result<()> {
        let _: Value = self
            .request(
                Method::PATCH,
                &format!("/v1/appStoreVersions/{version_id}"),
                Some(release_type_body(version_id, release_type, earliest)),
            )
            .await?;
        Ok(())
    }

    /// Recent and current review submissions for the app.
    pub async fn review_submissions(&self, app_id: &str) -> Result<Vec<ReviewSubmission>> {
        let resources = self
            .get_all::<ReviewSubmissionAttributes>(&format!("/v1/reviewSubmissions?filter[app]={app_id}&limit=20"))
            .await?;
        Ok(resources.into_iter().map(Into::into).collect())
    }

    pub async fn review_submission(&self, submission_id: &str) -> Result<ReviewSubmission> {
        let document: Document<Resource<ReviewSubmissionAttributes>> = self
            .request(Method::GET, &format!("/v1/reviewSubmissions/{submission_id}"), None)
            .await?;
        Ok(document.data.into())
    }

    /// Reads the version and build and returns what would be submitted.
    /// Changes nothing.
    pub async fn prepare_submission(
        &self,
        app_id: &str,
        app_name: &str,
        version_id: &str,
        build_id: &str,
        release_type: ReleaseType,
        earliest_release_date: Option<String>,
    ) -> Result<SubmissionPlan> {
        let version = self.app_store_version(version_id).await?;
        if !version.is_editable() {
            return Err(Error::NoEditableVersion);
        }
        let build = self.build(build_id).await?;
        if !build.is_ready() {
            return Err(Error::BuildNotReady(build.label(), build.processing_state));
        }
        Ok(SubmissionPlan {
            app_id: app_id.to_owned(),
            app_name: app_name.to_owned(),
            platform: version.platform.unwrap_or(Platform::Ios),
            version_id: version.id,
            version: version.version.unwrap_or_default(),
            version_state: version.state.unwrap_or_default(),
            build_id: build.id.clone(),
            build: build.label(),
            release_type,
            earliest_release_date: if release_type == ReleaseType::Scheduled {
                earliest_release_date
            } else {
                None
            },
            uses_non_exempt_encryption: build.uses_non_exempt_encryption,
        })
    }

    /// Sends the version in `plan` to App Review. Refuses unless
    /// `confirmed` is true, which callers may only pass after showing the
    /// user [`SubmissionPlan::summary`].
    pub async fn submit_for_review(&self, plan: &SubmissionPlan, confirmed: bool) -> Result<ReviewSubmission> {
        if !confirmed {
            return Err(Error::NotConfirmed);
        }
        // Check nothing moved between confirmation and now.
        let version = self.app_store_version(&plan.version_id).await?;
        if !version.is_editable() {
            return Err(Error::PlanOutdated(format!(
                "version is now {}",
                version.state.unwrap_or_default()
            )));
        }
        if version.version.as_deref() != Some(plan.version.as_str()) {
            return Err(Error::PlanOutdated("the version number changed".into()));
        }
        let build = self.build(&plan.build_id).await?;
        if !build.is_ready() {
            return Err(Error::BuildNotReady(build.label(), build.processing_state));
        }

        self.attach_build(&plan.version_id, &plan.build_id).await?;
        if version.release_type != Some(plan.release_type) || plan.earliest_release_date.is_some() {
            self.set_release_type(
                &plan.version_id,
                plan.release_type,
                plan.earliest_release_date.as_deref(),
            )
            .await?;
        }

        let submission = self.open_submission(&plan.app_id, plan.platform).await?;
        let items = parse_item_versions(
            &self
                .send(
                    Method::GET,
                    &format!("/v1/reviewSubmissions/{}/items?include=appStoreVersion", submission.id),
                    None,
                )
                .await?,
        )?;
        if !items.contains(&plan.version_id) {
            let _: Value = self
                .request(
                    Method::POST,
                    "/v1/reviewSubmissionItems",
                    Some(submission_item_body(&submission.id, &plan.version_id)),
                )
                .await?;
        }
        let document: Document<Resource<ReviewSubmissionAttributes>> = self
            .request(
                Method::PATCH,
                &format!("/v1/reviewSubmissions/{}", submission.id),
                Some(submit_body(&submission.id)),
            )
            .await?;
        Ok(document.data.into())
    }

    /// Withdraws a submission from review. Refuses unless `confirmed`.
    pub async fn cancel_submission(&self, submission_id: &str, confirmed: bool) -> Result<ReviewSubmission> {
        if !confirmed {
            return Err(Error::NotConfirmed);
        }
        let document: Document<Resource<ReviewSubmissionAttributes>> = self
            .request(
                Method::PATCH,
                &format!("/v1/reviewSubmissions/{submission_id}"),
                Some(cancel_body(submission_id)),
            )
            .await?;
        Ok(document.data.into())
    }

    /// The app's draft submission for this platform, or a new one.
    async fn open_submission(&self, app_id: &str, platform: Platform) -> Result<ReviewSubmission> {
        let platform_filter = serde_json::to_value(platform)?.as_str().unwrap_or("IOS").to_owned();
        let drafts = self
            .get_all::<ReviewSubmissionAttributes>(&format!(
                "/v1/reviewSubmissions?filter[app]={app_id}&filter[platform]={platform_filter}&filter[state]=READY_FOR_REVIEW"
            ))
            .await?;
        if let Some(draft) = drafts.into_iter().next() {
            return Ok(draft.into());
        }
        let document: Document<Resource<ReviewSubmissionAttributes>> = self
            .request(
                Method::POST,
                "/v1/reviewSubmissions",
                Some(create_submission_body(app_id, platform)),
            )
            .await?;
        Ok(document.data.into())
    }
}
