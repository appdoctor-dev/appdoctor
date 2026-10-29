use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    pub access_token: String,
    pub expires_in: Option<u64>,
}

/// Every Apple Ads response: `result` on success, `error` on failure.
#[derive(Debug, Deserialize)]
pub(crate) struct Envelope<T> {
    pub result: Option<T>,
    pub error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiError {
    pub code: String,
    pub message: Option<String>,
    #[serde(default)]
    pub details: Vec<ApiErrorDetail>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorDetail {
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AclList {
    #[serde(default)]
    pub acls: Vec<Acl>,
}

/// One ad account the API user can reach.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Acl {
    pub ad_account: AdAccount,
    #[serde(default)]
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdAccount {
    pub id: i64,
    pub name: Option<String>,
    pub org_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PhrasePopularity {
    pub phrase: String,
    /// 0–100.
    pub popularity: u8,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct KeywordSuggestion {
    pub text: String,
    pub popularity: u8,
}
