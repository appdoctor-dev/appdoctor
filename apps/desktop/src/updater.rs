//! Updates from GitHub releases.
//!
//! Every release carries `latest.json`: its version and, for each platform,
//! the download's URL, size and an Ed25519 signature of the file. The release
//! workflow signs with a key only it holds; the public half is built into the
//! app. On macOS an official build downloads the new .dmg in the background,
//! checks the signature, swaps the new AppDoctor.app in for this one and asks
//! for a restart. Everywhere else (Windows for now, builds from source) the
//! update is only announced, with a link to download it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context as _, bail};
use appdoctor_entitlement::UpdateInfo;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::Signature;
use serde::Deserialize;

const REPOSITORY: &str = "https://github.com/appdoctor-dev/appdoctor";

/// The public half of the update signing key, written by
/// `apps/desktop/scripts/update-signing-key.sh`. Empty means updates can be
/// announced but never installed.
const PUBLIC_KEY: &str = include_str!("../update-signing-key.pub");

/// Set by the release workflow. A build from source never replaces itself
/// with the official app.
const OFFICIAL_BUILD: bool = option_env!("APPDOCTOR_OFFICIAL_BUILD").is_some();

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    notes: Option<String>,
    platforms: HashMap<String, Download>,
}

#[derive(Deserialize)]
struct Download {
    url: String,
    signature: String,
    #[serde(default)]
    size: Option<u64>,
}

fn client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("AppDoctor/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// The newest release when it's newer than this build and has a download for
/// this platform. `Ok(None)` before the first release is published.
pub async fn check() -> anyhow::Result<Option<UpdateInfo>> {
    // GitHub redirects /latest/download/ to the newest published release.
    let response = client()?
        .get(format!("{REPOSITORY}/releases/latest/download/latest.json"))
        .send()
        .await?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let mut manifest: Manifest = serde_json::from_str(&response.error_for_status()?.text().await?)?;
    if !is_newer(&manifest.version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let Some(download) = manifest.platforms.remove(appdoctor_entitlement::platform()) else {
        return Ok(None);
    };
    Ok(Some(UpdateInfo {
        version: manifest.version,
        notes: manifest.notes,
        url: download.url,
        signature: Some(download.signature),
        size: download.size,
    }))
}

/// The .app this process runs from, when it may replace itself: an official
/// macOS build, inside a bundle, with a key to check updates against.
pub fn installable_bundle() -> Option<PathBuf> {
    if !cfg!(target_os = "macos") || !OFFICIAL_BUILD || appdoctor_entitlement::parse_public_key(PUBLIC_KEY).is_err() {
        return None;
    }
    // …/AppDoctor.app/Contents/MacOS/appdoctor
    let exe = std::env::current_exe().ok()?;
    let bundle = exe.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

/// Downloads the update, checks its signature and puts it in place of
/// `bundle`. The running app carries on; the new one starts on relaunch.
pub async fn install(info: UpdateInfo, bundle: PathBuf) -> anyhow::Result<()> {
    let signature = info.signature.as_deref().context("the update isn't signed")?;
    let image = client()?.get(&info.url).send().await?.error_for_status()?.bytes().await?;
    if info.size.is_some_and(|size| size != image.len() as u64) {
        bail!("the download is incomplete");
    }
    verify(PUBLIC_KEY, &image, signature)?;
    tokio::task::spawn_blocking(move || replace(&image, &bundle)).await?
}

fn verify(public_key: &str, bytes: &[u8], signature: &str) -> anyhow::Result<()> {
    let key = appdoctor_entitlement::parse_public_key(public_key)?;
    let signature = Signature::from_slice(&STANDARD.decode(signature.trim())?)?;
    key.verify_strict(bytes, &signature)
        .context("the update's signature doesn't match AppDoctor's key")
}

/// Mounts the disk image, copies its AppDoctor.app next to `bundle` and swaps
/// the two with renames, so a failure part-way leaves the current app intact.
fn replace(image: &[u8], bundle: &Path) -> anyhow::Result<()> {
    let work = std::env::temp_dir().join(format!("appdoctor-update-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let dmg = work.join("AppDoctor.dmg");
    let mount = work.join("mount");
    std::fs::write(&dmg, image)?;
    run(Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-noautoopen", "-readonly", "-mountpoint"])
        .arg(&mount)
        .arg(&dmg))?;

    let swapped = (|| {
        let parent = bundle.parent().context("the app has no folder")?;
        let staged = parent.join(".AppDoctor-update.app");
        let old = parent.join(".AppDoctor-old.app");
        let _ = std::fs::remove_dir_all(&staged);
        let _ = std::fs::remove_dir_all(&old);
        run(Command::new("ditto").arg(mount.join("AppDoctor.app")).arg(&staged))
            .with_context(|| format!("can't write to {}", parent.display()))?;
        std::fs::rename(bundle, &old)?;
        if let Err(error) = std::fs::rename(&staged, bundle) {
            let _ = std::fs::rename(&old, bundle);
            return Err(error.into());
        }
        let _ = std::fs::remove_dir_all(&old);
        Ok(())
    })();

    let _ = Command::new("hdiutil").args(["detach", "-quiet"]).arg(&mount).status();
    let _ = std::fs::remove_dir_all(&work);
    swapped
}

fn run(command: &mut Command) -> anyhow::Result<()> {
    let output = command.output()?;
    if !output.status.success() {
        bail!("{}", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(())
}

/// Opens `bundle` again once this process has exited. The caller quits.
pub fn relaunch(bundle: &Path) -> anyhow::Result<()> {
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"while kill -0 "$0" 2>/dev/null; do sleep 0.2; done; exec open "$1""#)
        .arg(std::process::id().to_string())
        .arg(bundle)
        .spawn()?;
    Ok(())
}

/// Dotted numeric versions ("0.10.0" > "0.9.3"); a pre-release ("1.0.0-beta")
/// is older than its release.
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parse(version: &str) -> (Vec<u64>, Option<&str>) {
        let (core, pre) = match version.trim_start_matches('v').split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (version.trim_start_matches('v'), None),
        };
        (core.split('.').map(|part| part.parse().unwrap_or(0)).collect(), pre)
    }
    let (a, pre_a) = parse(candidate);
    let (b, pre_b) = parse(current);
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    match (pre_a, pre_b) {
        (None, Some(_)) => true,
        (Some(x), Some(y)) => x > y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{PUBLIC_KEY, is_newer, replace, verify};

    /// A malformed key would quietly turn off installing updates.
    #[test]
    fn the_update_key_is_a_valid_public_key() {
        assert!(appdoctor_entitlement::parse_public_key(PUBLIC_KEY).is_ok());
    }

    // A throwaway key pair; the signature is what the release workflow's
    // `openssl pkeyutl -sign -rawin` makes for "AppDoctor update".
    const KEY: &str = "icqwzC6xDLiEfZ6wkZJMzqQJJ-7D6M1mP4bgV6uiYZE\n";
    const SIGNATURE: &str = "vFeHprYAYh3seypguPfCw5MePE2Lwq6HZc3ceU793aziMSf09aAASWf55XeM1ztTFEEa5njHxb/Txzmf7Vv2AA==";

    #[test]
    fn accepts_only_signed_updates() {
        assert!(verify(KEY, b"AppDoctor update", SIGNATURE).is_ok());
        assert!(verify(KEY, b"AppDoctor updatE", SIGNATURE).is_err());
        assert!(verify("", b"AppDoctor update", SIGNATURE).is_err());
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("v1.0.0", "1.0.0-beta"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.2.0"));
        assert!(!is_newer("1.0.0-beta", "1.0.0"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn swaps_in_the_app_from_the_disk_image() {
        let root = std::env::temp_dir().join(format!("appdoctor-swap-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let write = |path: std::path::PathBuf| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        };
        write(root.join("new/AppDoctor.app/Contents/new"));
        write(root.join("Applications/AppDoctor.app/Contents/old"));
        let dmg = root.join("update.dmg");
        let created = std::process::Command::new("hdiutil")
            .args(["create", "-quiet", "-format", "UDZO", "-srcfolder"])
            .arg(root.join("new"))
            .arg(&dmg)
            .status()
            .unwrap();
        assert!(created.success());

        let bundle = root.join("Applications/AppDoctor.app");
        replace(&std::fs::read(&dmg).unwrap(), &bundle).unwrap();

        assert!(bundle.join("Contents/new").exists());
        assert!(!bundle.join("Contents/old").exists());
        let left: Vec<_> = std::fs::read_dir(root.join("Applications")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(left, ["AppDoctor.app"]);
        std::fs::remove_dir_all(&root).unwrap();
    }
}
