//! AppDoctor accounts and licences, kept behind one seam.
//!
//! The app is open source; a subscription pays for signed installers,
//! updates and convenience. This crate is everything that touches that:
//!
//! - [`ApiClient::start_sign_in`] / [`ApiClient::wait_for_sign_in`]: Better
//!   Auth's device authorization flow (RFC 8628). The browser does the
//!   signing in; the app only ever holds the resulting bearer token.
//! - [`ApiClient::request_entitlement`]: exchanges that token, plus this
//!   install's random device ID, for a signed entitlement token.
//! - [`verify`]: checks an entitlement token offline against the Ed25519
//!   public key compiled into the app, so a licensed app keeps working
//!   offline until the token expires (the API grants about a week).
//! - [`ApiClient::check_update`]: asks whether a newer build exists.
//!
//! Building with the `no-license-check` feature makes [`evaluate`] report
//! [`Status::DevBuild`], which is entitled, without contacting anyone.

mod api;
mod token;

pub use api::{ApiClient, DeviceCode, DeviceInfo, EntitlementGrant, KnownDevice, SignInError, UpdateInfo};
pub use token::{Claims, TokenError, parse_public_key, verify};

/// The Ed25519 public key entitlement tokens are verified against:
/// base64url of the raw 32 bytes, as `npm run keys:generate` in `apps/api`
/// prints it.
// TODO(release): replace with the production key before shipping signed
// builds. Until then no token verifies, so builds without
// `no-license-check` show "Not activated".
pub const PUBLIC_KEY: &str = "REPLACE_WITH_PUBLIC_KEY_FROM_keys:generate";

/// Default AppDoctor API, overridable in Settings for self-hosting and dev.
pub const DEFAULT_API_BASE: &str = "http://localhost:8787";

/// The OAuth client ID Better Auth knows the desktop app by.
pub const CLIENT_ID: &str = "appdoctor-desktop";

/// Refresh the entitlement about daily while online.
pub const REFRESH_AFTER_SECS: i64 = 24 * 60 * 60;

/// What the app may do right now, and what to tell the user.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Built from source with `no-license-check`.
    DevBuild,
    /// No entitlement stored.
    NotActivated,
    Active {
        plan: String,
        expires_at: i64,
        needs_refresh: bool,
    },
    /// Was licensed, but the token's offline window ran out.
    Expired { plan: String },
    /// A token is stored but doesn't verify (wrong device, tampered, or the
    /// app's public key isn't set).
    Invalid(String),
}

impl Status {
    pub fn is_entitled(&self) -> bool {
        matches!(self, Status::DevBuild | Status::Active { .. })
    }
}

/// Judges a stored entitlement token for this device at `now` (Unix
/// seconds), offline.
pub fn evaluate(token: Option<&str>, device_id: &str, now: i64) -> Status {
    if cfg!(feature = "no-license-check") {
        return Status::DevBuild;
    }
    let Some(token) = token else {
        return Status::NotActivated;
    };
    let key = match public_key() {
        Ok(key) => key,
        Err(error) => return Status::Invalid(error.to_string()),
    };
    match verify(token, &key, device_id, now) {
        Ok(claims) => Status::Active {
            needs_refresh: now - claims.iat >= REFRESH_AFTER_SECS,
            plan: claims.plan,
            expires_at: claims.exp,
        },
        Err(TokenError::Expired { plan }) => Status::Expired { plan },
        Err(error) => Status::Invalid(error.to_string()),
    }
}

/// The compiled-in key, or in debug builds `APPDOCTOR_ENTITLEMENT_PUBLIC_KEY`
/// so contributors can test against a local API's dev key pair.
fn public_key() -> Result<ed25519_dalek::VerifyingKey, TokenError> {
    #[cfg(debug_assertions)]
    if let Ok(key) = std::env::var("APPDOCTOR_ENTITLEMENT_PUBLIC_KEY") {
        return parse_public_key(&key);
    }
    parse_public_key(PUBLIC_KEY)
}

/// `macos`, `windows` or `linux`, as the API names platforms.
pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

/// `arm64` or `x64`, as the update endpoint names architectures.
pub fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

#[cfg(all(test, not(feature = "no-license-check")))]
mod tests {
    use super::*;

    #[test]
    fn placeholder_key_reports_invalid_not_active() {
        // With no env override in the test process, the placeholder key must
        // never let a token through.
        if std::env::var("APPDOCTOR_ENTITLEMENT_PUBLIC_KEY").is_ok() {
            return;
        }
        assert_eq!(evaluate(None, "d", 0), Status::NotActivated);
        assert!(matches!(evaluate(Some("a.b"), "d", 0), Status::Invalid(_)));
    }

    #[test]
    fn platform_and_arch_are_api_names() {
        assert!(["macos", "windows", "linux"].contains(&platform()));
        assert!(["arm64", "x64"].contains(&arch()));
    }
}
