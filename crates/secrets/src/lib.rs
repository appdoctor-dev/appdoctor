//! Secrets in the OS keychain: macOS Keychain, Windows Credential Manager
//! (and the Secret Service on Linux), via the `keyring` crate.
//!
//! Each secret is one keychain item under the service
//! [`SERVICE`], holding JSON. Nothing here is ever written to the database
//! or to disk by AppDoctor itself.

use std::collections::HashMap;
use std::sync::Mutex;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const SERVICE: &str = "app.appdoctor.desktop";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("keychain: {0}")]
    Keychain(String),
    #[error("stored secret is corrupt: {0}")]
    Corrupt(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The secrets AppDoctor keeps, one keychain item each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// [`AscKey`]
    AppStoreConnect,
    /// [`AppleAdsKey`]
    AppleAds,
    /// The AppDoctor account bearer token from the device sign-in flow.
    AccountToken,
    /// The signed entitlement token. Not secret (it is bound to this
    /// device and verified offline), but it is a credential, so it lives
    /// with the others.
    Entitlement,
}

impl Secret {
    fn account(self) -> &'static str {
        match self {
            Secret::AppStoreConnect => "app-store-connect-key",
            Secret::AppleAds => "apple-ads-key",
            Secret::AccountToken => "account-token",
            Secret::Entitlement => "entitlement",
        }
    }
}

/// An App Store Connect API key.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AscKey {
    pub issuer_id: String,
    pub key_id: String,
    /// Contents of the `.p8` file.
    pub private_key_pem: String,
}

impl std::fmt::Debug for AscKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AscKey")
            .field("issuer_id", &self.issuer_id)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

/// The Apple Ads key pair AppDoctor generated, and the IDs Apple returned
/// once the public half was uploaded (empty until the user enters them).
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AppleAdsKey {
    pub private_key_pem: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub team_id: String,
    #[serde(default)]
    pub key_id: String,
    #[serde(default)]
    pub ad_account_id: Option<i64>,
}

impl AppleAdsKey {
    pub fn is_complete(&self) -> bool {
        !self.client_id.trim().is_empty() && !self.team_id.trim().is_empty() && !self.key_id.trim().is_empty()
    }
}

impl std::fmt::Debug for AppleAdsKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppleAdsKey")
            .field("client_id", &self.client_id)
            .field("team_id", &self.team_id)
            .field("key_id", &self.key_id)
            .field("ad_account_id", &self.ad_account_id)
            .finish_non_exhaustive()
    }
}

/// Where secrets are kept.
pub trait Backend: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>>;
    fn set(&self, account: &str, value: &str) -> Result<()>;
    fn delete(&self, account: &str) -> Result<()>;
}

/// The OS keychain.
pub struct OsKeychain;

impl OsKeychain {
    fn entry(account: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, account).map_err(|e| Error::Keychain(e.to_string()))
    }
}

impl Backend for OsKeychain {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match Self::entry(account)?.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }

    fn set(&self, account: &str, value: &str) -> Result<()> {
        Self::entry(account)?
            .set_password(value)
            .map_err(|e| Error::Keychain(e.to_string()))
    }

    fn delete(&self, account: &str) -> Result<()> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(Error::Keychain(e.to_string())),
        }
    }
}

/// An in-process store, for tests and throwaway sessions.
#[derive(Default)]
pub struct Memory(Mutex<HashMap<String, String>>);

impl Backend for Memory {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.0.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(account.to_owned(), value.to_owned());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        self.0.lock().unwrap().remove(account);
        Ok(())
    }
}

/// Typed access to AppDoctor's secrets.
pub struct Vault {
    backend: Box<dyn Backend>,
}

impl Vault {
    pub fn os() -> Self {
        Self::new(OsKeychain)
    }

    pub fn new(backend: impl Backend + 'static) -> Self {
        Self {
            backend: Box::new(backend),
        }
    }

    pub fn get<T: DeserializeOwned>(&self, secret: Secret) -> Result<Option<T>> {
        self.backend
            .get(secret.account())?
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(Error::from)
    }

    pub fn set<T: Serialize>(&self, secret: Secret, value: &T) -> Result<()> {
        self.backend.set(secret.account(), &serde_json::to_string(value)?)
    }

    pub fn delete(&self, secret: Secret) -> Result<()> {
        self.backend.delete(secret.account())
    }

    pub fn get_string(&self, secret: Secret) -> Result<Option<String>> {
        self.get(secret)
    }

    pub fn set_string(&self, secret: Secret, value: &str) -> Result<()> {
        self.set(secret, &value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_round_trip() {
        let vault = Vault::new(Memory::default());
        assert!(vault.get::<AscKey>(Secret::AppStoreConnect).unwrap().is_none());
        let key = AscKey {
            issuer_id: "issuer".into(),
            key_id: "KEY".into(),
            private_key_pem: "-----BEGIN PRIVATE KEY-----\n...".into(),
        };
        vault.set(Secret::AppStoreConnect, &key).unwrap();
        assert_eq!(vault.get::<AscKey>(Secret::AppStoreConnect).unwrap(), Some(key));
        vault.set_string(Secret::Entitlement, "a.b").unwrap();
        assert_eq!(vault.get_string(Secret::Entitlement).unwrap().as_deref(), Some("a.b"));
        vault.delete(Secret::Entitlement).unwrap();
        vault.delete(Secret::Entitlement).unwrap();
        assert_eq!(vault.get_string(Secret::Entitlement).unwrap(), None);
    }

    #[test]
    fn apple_ads_key_fills_in_later() {
        let vault = Vault::new(Memory::default());
        let mut key = AppleAdsKey {
            private_key_pem: "pem".into(),
            ..Default::default()
        };
        assert!(!key.is_complete());
        vault.set(Secret::AppleAds, &key).unwrap();
        key.client_id = "SEARCHADS.c".into();
        key.team_id = "SEARCHADS.t".into();
        key.key_id = "k".into();
        assert!(key.is_complete());
        vault.set(Secret::AppleAds, &key).unwrap();
        assert_eq!(vault.get::<AppleAdsKey>(Secret::AppleAds).unwrap(), Some(key));
    }

    #[test]
    fn debug_never_prints_private_keys() {
        let key = AscKey {
            issuer_id: "i".into(),
            key_id: "k".into(),
            private_key_pem: "SECRET".into(),
        };
        assert!(!format!("{key:?}").contains("SECRET"));
        let key = AppleAdsKey {
            private_key_pem: "SECRET".into(),
            ..Default::default()
        };
        assert!(!format!("{key:?}").contains("SECRET"));
    }

    #[test]
    fn corrupt_items_error_instead_of_panicking() {
        let memory = Memory::default();
        memory.set("app-store-connect-key", "not json").unwrap();
        let vault = Vault::new(memory);
        assert!(matches!(
            vault.get::<AscKey>(Secret::AppStoreConnect),
            Err(Error::Corrupt(_))
        ));
    }
}
