//! The app's single entitlement seam: what the licence status is, signing
//! in, refreshing, signing out and checking for updates. Everything else in
//! the app asks [`Account::is_entitled`] and nothing more.

use std::sync::Arc;

use appdoctor_entitlement::{ApiClient, DeviceCode, DeviceInfo, SignInError, Status};
use appdoctor_secrets::Secret;

use crate::rank_check::now;
use crate::services::Services;

#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub status: Status,
    /// Set while the user is approving sign-in in the browser.
    pub pending: Option<DeviceCode>,
    pub busy: bool,
    pub error: Option<String>,
}

impl Account {
    pub fn load(services: &Services) -> Self {
        Self {
            status: current_status(services),
            pending: None,
            busy: false,
            error: None,
        }
    }

    pub fn is_entitled(&self) -> bool {
        self.status.is_entitled()
    }

    pub fn has_session(services: &Services) -> bool {
        matches!(services.vault.get_string(Secret::AccountToken), Ok(Some(_)))
    }
}

pub fn current_status(services: &Services) -> Status {
    if cfg!(feature = "no-license-check") {
        return Status::DevBuild;
    }
    let token = services.vault.get_string(Secret::Entitlement).ok().flatten();
    appdoctor_entitlement::evaluate(token.as_deref(), &services.device_id, now())
}

fn api(services: &Services) -> ApiClient {
    ApiClient::new(&services.api_base())
}

/// Step one of sign-in: get a code for the user to approve in the browser.
pub async fn start_sign_in(services: Arc<Services>) -> anyhow::Result<DeviceCode> {
    Ok(api(&services).start_sign_in().await?)
}

/// Step two: wait for approval, keep the session token, fetch a licence.
pub async fn finish_sign_in(services: Arc<Services>, code: DeviceCode) -> anyhow::Result<Status> {
    let token = api(&services).wait_for_sign_in(&code).await?;
    services.vault.set_string(Secret::AccountToken, &token)?;
    refresh(services).await
}

/// Asks the API for a fresh licence for this device and stores it.
pub async fn refresh(services: Arc<Services>) -> anyhow::Result<Status> {
    let Some(token) = services.vault.get_string(Secret::AccountToken)? else {
        return Ok(current_status(&services));
    };
    let device = DeviceInfo {
        device_id: services.device_id.clone(),
        device_name: computer_name(),
        platform: appdoctor_entitlement::platform(),
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    match api(&services).request_entitlement(&token, &device).await {
        Ok(grant) => {
            services.vault.set_string(Secret::Entitlement, &grant.token)?;
            Ok(current_status(&services))
        }
        Err(SignInError::Unauthorized) => {
            services.vault.delete(Secret::AccountToken)?;
            Err(SignInError::Unauthorized.into())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn sign_out(services: &Services) -> anyhow::Result<()> {
    services.vault.delete(Secret::AccountToken)?;
    services.vault.delete(Secret::Entitlement)?;
    Ok(())
}

/// A human name for this computer, shown in the account's device list.
fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    let name = std::process::Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok());
    #[cfg(target_os = "windows")]
    let name = std::env::var("COMPUTERNAME").ok();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let name = std::env::var("HOSTNAME").ok();

    name.map(|name| name.trim().chars().take(100).collect::<String>())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Computer".into())
}
