//! App icons from Apple's CDN, fetched once and cached on disk.
//!
//! gpui's `img()` can't fetch URLs without an HTTP client installed in the
//! app, so icons are fetched on the I/O runtime and handed to gpui as
//! decoded-on-demand `Image`s. Views observe the cache and re-render as
//! icons arrive.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::{Context, Image, ImageFormat};
use tokio::sync::Semaphore;

use crate::runtime;

enum Slot {
    Loading,
    Ready(Arc<Image>),
    Failed,
}

pub struct IconCache {
    http: reqwest::Client,
    dir: PathBuf,
    slots: HashMap<String, Slot>,
    permits: Arc<Semaphore>,
}

impl IconCache {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            http: reqwest::Client::builder()
                .user_agent(concat!("AppDoctor/", env!("CARGO_PKG_VERSION")))
                .build()
                .expect("reqwest client builds with static config"),
            dir: cache_dir,
            slots: HashMap::new(),
            permits: Arc::new(Semaphore::new(8)),
        }
    }

    /// The icon if it's ready; otherwise starts loading it and returns `None`.
    pub fn get(&mut self, url: &str, cx: &mut Context<Self>) -> Option<Arc<Image>> {
        match self.slots.get(url) {
            Some(Slot::Ready(image)) => return Some(image.clone()),
            Some(Slot::Loading | Slot::Failed) => return None,
            None => {}
        }
        self.slots.insert(url.to_owned(), Slot::Loading);
        let (http, permits, url, path) = (
            self.http.clone(),
            self.permits.clone(),
            url.to_owned(),
            self.path_for(url),
        );
        let fetch = runtime::spawn({
            let url = url.clone();
            async move {
                if let Ok(bytes) = tokio::fs::read(&path).await {
                    return Ok(bytes);
                }
                let _permit = permits.acquire().await?;
                let bytes = http.get(&url).send().await?.error_for_status()?.bytes().await?.to_vec();
                if let Some(parent) = path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                let _ = tokio::fs::write(&path, &bytes).await;
                Ok(bytes)
            }
        });
        cx.spawn(async move |this, cx| {
            let slot = match fetch.await {
                Ok(bytes) => match format_of(&bytes) {
                    Some(format) => Slot::Ready(Arc::new(Image::from_bytes(format, bytes))),
                    None => Slot::Failed,
                },
                Err(error) => {
                    tracing::debug!(%url, %error, "icon fetch failed");
                    Slot::Failed
                }
            };
            this.update(cx, |this, cx| {
                this.slots.insert(url, slot);
                cx.notify();
            })
            .ok();
        })
        .detach();
        None
    }

    fn path_for(&self, url: &str) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        self.dir.join(format!("{:016x}", hasher.finish()))
    }
}

fn format_of(bytes: &[u8]) -> Option<ImageFormat> {
    match bytes {
        [0xFF, 0xD8, ..] => Some(ImageFormat::Jpeg),
        [0x89, b'P', b'N', b'G', ..] => Some(ImageFormat::Png),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some(ImageFormat::Webp),
        _ => None,
    }
}
