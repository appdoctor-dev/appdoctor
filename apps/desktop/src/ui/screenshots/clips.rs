//! Frames of screen recordings for the canvas. Decoding runs on a thread of
//! its own (AVFoundation's objects stay on the thread that made them), and
//! frames come back as images the canvas draws; until a frame arrives, the
//! nearest one already decoded stands in.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use futures::StreamExt as _;
use futures::channel::mpsc as async_mpsc;
use gpui_kit::{Context, Image, ImageFormat, Task};

use super::ScreenshotsView;

/// Frames per second of recording kept for the canvas.
const STEPS: f32 = 15.;
/// The canvas never needs more than this; smaller decodes faster.
const SIZE: (u32, u32) = (480, 1040);
const KEEP: usize = 600;

type Key = (PathBuf, u32);

pub(super) struct ClipFrames {
    frames: RefCell<HashMap<Key, Arc<Image>>>,
    /// The last frame decoded per recording, shown while the next decodes.
    latest: RefCell<HashMap<PathBuf, Arc<Image>>>,
    pending: RefCell<HashSet<Key>>,
    requests: mpsc::Sender<Key>,
}

impl ClipFrames {
    /// Starts the decoding thread; the task feeds frames back to the view.
    pub(super) fn new(cx: &mut Context<ScreenshotsView>) -> (ClipFrames, Task<()>) {
        let (requests, wanted) = mpsc::channel::<Key>();
        let (done, mut decoded) = async_mpsc::unbounded::<(Key, Option<Vec<u8>>)>();
        std::thread::Builder::new()
            .name("clip-frames".into())
            .spawn(move || {
                let clips = appdoctor_video::clip_source();
                while let Ok(key) = wanted.recv() {
                    let frame = clips
                        .frame(&key.0, key.1 as f32 / STEPS, SIZE)
                        .and_then(|frame| frame.encode_png().ok());
                    if done.unbounded_send((key, frame)).is_err() {
                        break;
                    }
                }
            })
            .ok();
        let task = cx.spawn(async move |this, cx| {
            while let Some((key, png)) = decoded.next().await {
                let updated = this.update(cx, |this, cx| {
                    let clips = &this.clips;
                    clips.pending.borrow_mut().remove(&key);
                    if let Some(png) = png {
                        let image = Arc::new(Image::from_bytes(ImageFormat::Png, png));
                        clips.latest.borrow_mut().insert(key.0.clone(), image.clone());
                        let mut frames = clips.frames.borrow_mut();
                        if frames.len() > KEEP {
                            frames.clear();
                        }
                        frames.insert(key, image);
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        });
        (
            ClipFrames {
                frames: RefCell::default(),
                latest: RefCell::default(),
                pending: RefCell::default(),
                requests,
            },
            task,
        )
    }

    /// The frame `time` seconds into the recording at `path`, or the
    /// nearest decoded so far while it decodes.
    pub(super) fn get(&self, path: &Path, time: f32) -> Option<Arc<Image>> {
        let key = (path.to_path_buf(), (time.max(0.) * STEPS).round() as u32);
        if let Some(frame) = self.frames.borrow().get(&key) {
            return Some(frame.clone());
        }
        if self.pending.borrow_mut().insert(key.clone()) {
            let _ = self.requests.send(key);
        }
        self.latest.borrow().get(path).cloned()
    }
}
