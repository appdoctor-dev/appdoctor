//! Uploading an `.ipa` / `.pkg` through the App Store Connect API's
//! `buildUploads` and `buildUploadFiles` resources:
//!
//! 1. `POST /v1/buildUploads` with the bundle's version and build number.
//! 2. `POST /v1/buildUploadFiles` with the file's name, size and type; the
//!    response carries `uploadOperations`, pre-signed part uploads.
//! 3. Send each part as its operation says (method, URL, headers, byte range).
//! 4. `PATCH /v1/buildUploadFiles/{id}` with `uploaded: true` and the file's
//!    checksum. Apple then processes the build; poll [`Client::build_upload`].
//!
//! Uploading sends the user's binary to Apple, so it needs `confirmed` too.
//! Without the API (or if it fails), the macOS fallback is Apple's own
//! tooling: `xcrun altool --upload-package <file> --api-key <KEY_ID>
//! --api-issuer <ISSUER_ID>` (with the `.p8` in `~/.appstoreconnect/private_keys`)
//! or the Transporter app.

use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::{Path, PathBuf};

use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::model::{Document, Resource};
use crate::{Client, Error, Platform, Result};

/// A built app to upload.
#[derive(Debug, Clone)]
pub struct BuildFile {
    pub path: PathBuf,
    pub platform: Platform,
    /// `CFBundleShortVersionString`, e.g. "3.22".
    pub version: String,
    /// `CFBundleVersion`, e.g. "1234".
    pub build_number: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UploadState {
    AwaitingUpload,
    Processing,
    Failed,
    Complete,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuildUpload {
    pub id: String,
    pub state: UploadState,
    /// Apple's error and warning messages, if any.
    pub messages: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct StateDetail {
    message: Option<String>,
    code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UploadStateBlock {
    state: Option<UploadState>,
    #[serde(default)]
    errors: Vec<StateDetail>,
    #[serde(default)]
    warnings: Vec<StateDetail>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct BuildUploadAttributes {
    state: Option<UploadStateBlock>,
}

impl From<Resource<BuildUploadAttributes>> for BuildUpload {
    fn from(resource: Resource<BuildUploadAttributes>) -> Self {
        let block = resource.attributes.state;
        let messages = block
            .as_ref()
            .map(|block| {
                block
                    .errors
                    .iter()
                    .chain(&block.warnings)
                    .filter_map(|d| d.message.clone().or_else(|| d.code.clone()))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            id: resource.id,
            state: block.and_then(|b| b.state).unwrap_or(UploadState::Unknown),
            messages,
        }
    }
}

/// One pre-signed part upload.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadOperation {
    pub method: String,
    pub url: String,
    pub offset: u64,
    pub length: u64,
    #[serde(default)]
    pub request_headers: Vec<Header>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildUploadFileAttributes {
    #[serde(default)]
    upload_operations: Vec<UploadOperation>,
}

#[cfg(test)]
impl BuildUploadFileAttributes {
    pub(crate) fn upload_operations(&self) -> &[UploadOperation] {
        &self.upload_operations
    }
}

/// `com.apple.ipa` or `com.apple.pkg`, from the extension.
pub(crate) fn uti_for(path: &Path) -> Result<&'static str> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ipa") => Ok("com.apple.ipa"),
        Some("pkg") => Ok("com.apple.pkg"),
        _ => Err(Error::Upload("expected an .ipa or .pkg file".into())),
    }
}

pub(crate) fn build_upload_body(app_id: &str, file: &BuildFile) -> Value {
    json!({"data": {
        "type": "buildUploads",
        "attributes": {
            "cfBundleShortVersionString": file.version,
            "cfBundleVersion": file.build_number,
            "platform": file.platform,
        },
        "relationships": {"app": {"data": {"type": "apps", "id": app_id}}}
    }})
}

pub(crate) fn upload_file_body(upload_id: &str, file_name: &str, size: u64, uti: &str) -> Value {
    json!({"data": {
        "type": "buildUploadFiles",
        "attributes": {"assetType": "ASSET", "fileName": file_name, "fileSize": size, "uti": uti},
        "relationships": {"buildUpload": {"data": {"type": "buildUploads", "id": upload_id}}}
    }})
}

pub(crate) fn upload_done_body(file_id: &str, sha256_hex: &str) -> Value {
    // TODO(asc-upload): Apple documents SHA_256 and MD5 checksums but not
    // the hash's encoding; hex matches what its other asset uploads take.
    // Verify against a real upload.
    json!({"data": {
        "type": "buildUploadFiles",
        "id": file_id,
        "attributes": {
            "uploaded": true,
            "sourceFileChecksums": {"file": {"algorithm": "SHA_256", "hash": sha256_hex}}
        }
    }})
}

pub(crate) fn sha256_hex(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

impl Client {
    /// Uploads a build. Refuses unless `confirmed`. `progress` gets
    /// (bytes sent, total bytes) after each part.
    pub async fn upload_build(
        &self,
        app_id: &str,
        file: &BuildFile,
        confirmed: bool,
        progress: impl Fn(u64, u64),
    ) -> Result<BuildUpload> {
        if !confirmed {
            return Err(Error::NotConfirmed);
        }
        let uti = uti_for(&file.path)?;
        let size = std::fs::metadata(&file.path)?.len();
        let file_name = file
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| Error::Upload("the file name isn't valid UTF-8".into()))?;

        let upload: Document<Resource<BuildUploadAttributes>> = self
            .request(Method::POST, "/v1/buildUploads", Some(build_upload_body(app_id, file)))
            .await?;
        let upload_id = upload.data.id;
        let reservation: Document<Resource<BuildUploadFileAttributes>> = self
            .request(
                Method::POST,
                "/v1/buildUploadFiles",
                Some(upload_file_body(&upload_id, file_name, size, uti)),
            )
            .await?;
        let file_id = reservation.data.id;

        let mut sent = 0;
        let mut source = std::fs::File::open(&file.path)?;
        for operation in reservation.data.attributes.upload_operations {
            let mut part = vec![0u8; operation.length as usize];
            source.seek(SeekFrom::Start(operation.offset))?;
            source.read_exact(&mut part)?;
            let method = Method::from_bytes(operation.method.as_bytes())
                .map_err(|_| Error::Upload(format!("unknown upload method {}", operation.method)))?;
            // Pre-signed URLs: no App Store Connect token on these.
            let mut request = self.http.request(method, &operation.url).body(part);
            for header in &operation.request_headers {
                request = request.header(&header.name, &header.value);
            }
            let response = request.send().await?;
            if !response.status().is_success() {
                return Err(Error::Upload(format!(
                    "part at offset {} returned HTTP {}",
                    operation.offset,
                    response.status().as_u16()
                )));
            }
            sent += operation.length;
            progress(sent, size);
        }

        let checksum = sha256_hex(&file.path)?;
        let _: Value = self
            .request(
                Method::PATCH,
                &format!("/v1/buildUploadFiles/{file_id}"),
                Some(upload_done_body(&file_id, &checksum)),
            )
            .await?;
        self.build_upload(&upload_id).await
    }

    pub async fn build_upload(&self, upload_id: &str) -> Result<BuildUpload> {
        let document: Document<Resource<BuildUploadAttributes>> = self
            .request(Method::GET, &format!("/v1/buildUploads/{upload_id}"), None)
            .await?;
        Ok(document.data.into())
    }
}
