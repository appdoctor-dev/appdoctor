//! Long-lived handles shared by every view: the database, the keychain,
//! the App Store client and this install's identity.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use appdoctor_appstore::{ITunesClient, NativeSearch, SearchSource, ThrottleConfig};
use appdoctor_secrets::{AscKey, Memory, Secret, Vault};
use appdoctor_store::Store;

pub struct Services {
    pub store: Store,
    pub vault: Vault,
    pub itunes: ITunesClient,
    /// App Store–native rankings, falling back to `itunes`.
    pub search: NativeSearch,
    pub data_dir: PathBuf,
    /// A random UUID made on first launch. Never derived from hardware.
    pub device_id: String,
}

const DEVICE_ID: &str = "device_id";
const API_BASE: &str = "api_base";

impl Services {
    /// Opens the data directory (`APPDOCTOR_DATA_DIR`, else the OS data
    /// dir: `~/Library/Application Support/AppDoctor` on macOS,
    /// `%APPDATA%\AppDoctor` on Windows) and the database in it.
    pub fn open() -> anyhow::Result<Arc<Self>> {
        let data_dir = std::env::var_os("APPDOCTOR_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| dirs::data_dir().map(|dir| dir.join("AppDoctor")))
            .context("no data directory on this system")?;
        let store = Store::open(&data_dir.join("appdoctor.db")).context("opening the database")?;
        let device_id = match store.setting(DEVICE_ID)? {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                store.set_setting(DEVICE_ID, &id)?;
                id
            }
        };
        // Unsigned dev builds change identity on every rebuild, which makes
        // macOS ask for keychain access each time; this keeps secrets in
        // memory for such sessions instead.
        let vault = if std::env::var_os("APPDOCTOR_EPHEMERAL_SECRETS").is_some() {
            Vault::new(Memory::default())
        } else {
            Vault::os()
        };
        let itunes = ITunesClient::new(ThrottleConfig::default());
        Ok(Arc::new(Self {
            store,
            vault,
            search: NativeSearch::new(itunes.clone(), ThrottleConfig::default()),
            itunes,
            data_dir,
            device_id,
        }))
    }

    /// Where rank checks get search results. The seam for a better source.
    pub fn search_source(&self) -> &dyn SearchSource {
        &self.search
    }

    /// An App Store Connect client from the key in the keychain.
    pub fn asc_client(&self) -> anyhow::Result<appdoctor_asc::Client> {
        let key: AscKey = self
            .vault
            .get(Secret::AppStoreConnect)?
            .context("add an App Store Connect key in Settings first")?;
        let credentials = appdoctor_asc::Credentials::new(key.issuer_id, key.key_id, &key.private_key_pem)?;
        Ok(appdoctor_asc::Client::new(credentials))
    }

    pub fn has_asc_key(&self) -> bool {
        matches!(self.vault.get::<AscKey>(Secret::AppStoreConnect), Ok(Some(_)))
    }

    pub fn api_base(&self) -> String {
        self.store
            .setting(API_BASE)
            .ok()
            .flatten()
            .unwrap_or_else(|| appdoctor_entitlement::DEFAULT_API_BASE.into())
    }

    pub fn set_api_base(&self, base: &str) -> anyhow::Result<()> {
        let base = base.trim().trim_end_matches('/');
        anyhow::ensure!(
            base.starts_with("http://") || base.starts_with("https://"),
            "the API address must start with http:// or https://"
        );
        self.store.set_setting(API_BASE, base)?;
        Ok(())
    }
}
