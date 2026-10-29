//! HTTP calls to the AppDoctor API. Only these leave the machine, and they
//! carry nothing about the user's apps or keywords: the device flow, the
//! entitlement request (random device ID, device name, platform, version)
//! and the update check.

use std::time::Duration;

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

use crate::CLIENT_ID;

#[derive(Debug, thiserror::Error)]
pub enum SignInError {
    #[error("the sign-in code expired; start again")]
    Expired,
    #[error("sign-in was declined in the browser")]
    Denied,
    #[error("your session has ended; sign in again")]
    Unauthorized,
    #[error("no active AppDoctor subscription on this account")]
    NoSubscription,
    #[error("this account is already active on {max_devices} device(s)")]
    DeviceLimit {
        max_devices: u32,
        devices: Vec<KnownDevice>,
    },
    #[error("AppDoctor API: {0}")]
    Api(String),
    #[error("couldn't reach the AppDoctor API: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected response from the AppDoctor API: {0}")]
    Decode(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SignInError>;

/// What the user needs to finish signing in, from `POST /api/auth/device/code`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    /// Seconds until the code expires.
    pub expires_in: u64,
    /// Seconds to wait between polls.
    #[serde(default = "default_interval")]
    pub interval: u64,
}

fn default_interval() -> u64 {
    5
}

impl DeviceCode {
    /// The page to open: with the code filled in when the server provides it.
    pub fn browser_url(&self) -> &str {
        self.verification_uri_complete
            .as_deref()
            .unwrap_or(&self.verification_uri)
    }
}

#[derive(Debug, Deserialize)]
struct TokenSuccess {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    error_description: Option<String>,
}

/// This install, as the entitlement endpoint wants it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    /// A random UUID created on first launch; never a hardware identifier.
    pub device_id: String,
    pub device_name: String,
    pub platform: &'static str,
    pub app_version: String,
}

/// A successful `POST /v1/entitlement`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitlementGrant {
    pub token: String,
    pub expires_at: String,
    pub plan: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownDevice {
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    pub last_seen_at: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiError {
    error: String,
    max_devices: Option<u32>,
    #[serde(default)]
    devices: Vec<KnownDevice>,
}

/// A newer build, from `GET /v1/updates/{platform}/{arch}/{version}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
    pub url: String,
    pub signature: Option<String>,
    pub size: Option<u64>,
}

#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    base: String,
}

impl ApiClient {
    pub fn new(base: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("AppDoctor/", env!("CARGO_PKG_VERSION")))
                .timeout(Duration::from_secs(20))
                .build()
                .expect("reqwest client builds with static config"),
            base: base.trim_end_matches('/').to_owned(),
        }
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// Starts sign-in: the user opens [`DeviceCode::browser_url`] and
    /// confirms [`DeviceCode::user_code`].
    pub async fn start_sign_in(&self) -> Result<DeviceCode> {
        let response = self
            .http
            .post(format!("{}/api/auth/device/code", self.base))
            .json(&serde_json::json!({"client_id": CLIENT_ID}))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(SignInError::Api(
                oauth_message(&body).unwrap_or_else(|| format!("HTTP {}", status.as_u16())),
            ));
        }
        Ok(serde_json::from_str(&body)?)
    }

    /// Polls until the user approves in the browser, honouring
    /// `authorization_pending` and `slow_down`. Returns the bearer token.
    pub async fn wait_for_sign_in(&self, code: &DeviceCode) -> Result<String> {
        let mut interval = Duration::from_secs(code.interval.max(1));
        let deadline = tokio::time::Instant::now() + Duration::from_secs(code.expires_in.max(1));
        loop {
            tokio::time::sleep(interval).await;
            if tokio::time::Instant::now() >= deadline {
                return Err(SignInError::Expired);
            }
            match self.poll_sign_in(code).await? {
                Poll::Done(token) => return Ok(token),
                Poll::Pending => {}
                Poll::SlowDown => interval += Duration::from_secs(5),
            }
        }
    }

    async fn poll_sign_in(&self, code: &DeviceCode) -> Result<Poll> {
        let response = self
            .http
            .post(format!("{}/api/auth/device/token", self.base))
            .json(&serde_json::json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": code.device_code,
                "client_id": CLIENT_ID,
            }))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            let token: TokenSuccess = serde_json::from_str(&body)?;
            return Ok(Poll::Done(token.access_token));
        }
        classify_poll_error(status, &body)
    }

    /// Exchanges the bearer token for a signed entitlement for this device.
    pub async fn request_entitlement(&self, access_token: &str, device: &DeviceInfo) -> Result<EntitlementGrant> {
        let response = self
            .http
            .post(format!("{}/v1/entitlement", self.base))
            .bearer_auth(access_token)
            .json(device)
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            return Ok(serde_json::from_str(&body)?);
        }
        Err(classify_entitlement_error(status, &body))
    }

    /// `Ok(None)` when this version is current.
    pub async fn check_update(&self, platform: &str, arch: &str, version: &str) -> Result<Option<UpdateInfo>> {
        let response = self
            .http
            .get(format!("{}/v1/updates/{platform}/{arch}/{version}", self.base))
            .send()
            .await?;
        match response.status() {
            StatusCode::NO_CONTENT => Ok(None),
            status if status.is_success() => Ok(Some(serde_json::from_str(&response.text().await?)?)),
            status => Err(SignInError::Api(format!(
                "update check returned HTTP {}",
                status.as_u16()
            ))),
        }
    }
}

#[derive(Debug, PartialEq)]
enum Poll {
    Done(String),
    Pending,
    SlowDown,
}

fn oauth_message(body: &str) -> Option<String> {
    let error: OAuthError = serde_json::from_str(body).ok()?;
    Some(error.error_description.unwrap_or(error.error))
}

fn classify_poll_error(status: StatusCode, body: &str) -> Result<Poll> {
    let Ok(error) = serde_json::from_str::<OAuthError>(body) else {
        return Err(SignInError::Api(format!("HTTP {}", status.as_u16())));
    };
    match error.error.as_str() {
        "authorization_pending" => Ok(Poll::Pending),
        "slow_down" => Ok(Poll::SlowDown),
        "expired_token" => Err(SignInError::Expired),
        "access_denied" => Err(SignInError::Denied),
        _ => Err(SignInError::Api(error.error_description.unwrap_or(error.error))),
    }
}

fn classify_entitlement_error(status: StatusCode, body: &str) -> SignInError {
    if status == StatusCode::UNAUTHORIZED {
        return SignInError::Unauthorized;
    }
    match serde_json::from_str::<ApiError>(body) {
        Ok(error) if error.error == "no_subscription" => SignInError::NoSubscription,
        Ok(error) if error.error == "device_limit" => SignInError::DeviceLimit {
            max_devices: error.max_devices.unwrap_or(1),
            devices: error.devices,
        },
        Ok(error) => SignInError::Api(error.error),
        Err(_) => SignInError::Api(format!("HTTP {}", status.as_u16())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_code() {
        let body = r#"{"device_code": "dc", "user_code": "WDJB-MJHT",
            "verification_uri": "http://localhost:3000/device",
            "verification_uri_complete": "http://localhost:3000/device?user_code=WDJB-MJHT",
            "expires_in": 1800, "interval": 5}"#;
        let code: DeviceCode = serde_json::from_str(body).unwrap();
        assert_eq!(code.user_code, "WDJB-MJHT");
        assert_eq!(code.browser_url(), "http://localhost:3000/device?user_code=WDJB-MJHT");
        let minimal: DeviceCode = serde_json::from_str(
            r#"{"device_code": "dc", "user_code": "X", "verification_uri": "https://a/device", "expires_in": 60}"#,
        )
        .unwrap();
        assert_eq!(minimal.interval, 5);
        assert_eq!(minimal.browser_url(), "https://a/device");
    }

    #[test]
    fn classifies_poll_responses() {
        let pending = r#"{"error": "authorization_pending", "error_description": "waiting"}"#;
        assert_eq!(
            classify_poll_error(StatusCode::BAD_REQUEST, pending).unwrap(),
            Poll::Pending
        );
        let slow = r#"{"error": "slow_down"}"#;
        assert_eq!(
            classify_poll_error(StatusCode::BAD_REQUEST, slow).unwrap(),
            Poll::SlowDown
        );
        let expired = r#"{"error": "expired_token"}"#;
        assert!(matches!(
            classify_poll_error(StatusCode::BAD_REQUEST, expired),
            Err(SignInError::Expired)
        ));
        let denied = r#"{"error": "access_denied"}"#;
        assert!(matches!(
            classify_poll_error(StatusCode::BAD_REQUEST, denied),
            Err(SignInError::Denied)
        ));
    }

    #[test]
    fn classifies_entitlement_errors() {
        assert!(matches!(
            classify_entitlement_error(StatusCode::PAYMENT_REQUIRED, r#"{"error": "no_subscription"}"#),
            SignInError::NoSubscription
        ));
        let limit = r#"{"error": "device_limit", "maxDevices": 2, "devices": [
            {"deviceId": "a", "deviceName": "Work Mac", "lastSeenAt": "2026-10-01T00:00:00.000Z"},
            {"deviceId": "b", "deviceName": "", "lastSeenAt": null}]}"#;
        match classify_entitlement_error(StatusCode::CONFLICT, limit) {
            SignInError::DeviceLimit { max_devices, devices } => {
                assert_eq!(max_devices, 2);
                assert_eq!(devices[0].device_name, "Work Mac");
                assert_eq!(devices.len(), 2);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            classify_entitlement_error(StatusCode::UNAUTHORIZED, ""),
            SignInError::Unauthorized
        ));
    }

    #[test]
    fn parses_grant_and_update() {
        let grant: EntitlementGrant = serde_json::from_str(
            r#"{"token": "a.b", "expiresAt": "2026-11-05T00:00:00.000Z", "plan": "pro", "status": "active"}"#,
        )
        .unwrap();
        assert_eq!(grant.plan, "pro");
        let update: UpdateInfo = serde_json::from_str(
            r#"{"version": "0.2.0", "notes": "Faster", "pubDate": "2026-10-28", "url": "http://localhost:8787/v1/downloads/macos/arm64",
                "signature": "sig", "size": 12345}"#,
        )
        .unwrap();
        assert_eq!(update.version, "0.2.0");
        assert_eq!(update.size, Some(12345));
    }

    #[test]
    fn device_info_uses_api_field_names() {
        let info = DeviceInfo {
            device_id: "id".into(),
            device_name: "Mac".into(),
            platform: "macos",
            app_version: "0.1.0".into(),
        };
        assert_eq!(
            serde_json::to_value(&info).unwrap(),
            serde_json::json!({"deviceId": "id", "deviceName": "Mac", "platform": "macos", "appVersion": "0.1.0"})
        );
    }
}
