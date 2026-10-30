//! Video for AppDoctor's designs: decoding screen recordings for device
//! screens, and encoding a design's frames to H.264 MP4 at the App Store's
//! sizes. Both use AVFoundation on macOS, so hardware encoding needs no
//! extra install; other platforms report [`Error::Unsupported`] for now.

use std::path::Path;

use appdoctor_screenshots::{Design, Target, motion, render_frame};

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{Clips, Encoder, clip_duration};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("making videos needs macOS for now")]
    Unsupported,
    #[error("this design isn't a video yet")]
    NotVideo,
    #[error("couldn't write the video: {0}")]
    Writer(String),
    #[error(transparent)]
    Render(#[from] appdoctor_screenshots::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The length Apple accepts for a target's videos, in seconds: app
/// previews (iPhone, iPad) run 15–30 s, header and search videos 5–30 s.
pub fn length_limits(target: Target) -> (f32, f32) {
    if target.is_screenshot() { (15., 30.) } else { (5., 30.) }
}

/// What's wrong with a design's length for the App Store, if anything.
pub fn length_problem(design: &Design) -> Option<String> {
    let (min, max) = length_limits(design.target);
    let length = motion::duration(&design.slides);
    if length < min {
        Some(format!("{length:.1} s is short: Apple wants {min:.0}–{max:.0} s"))
    } else if length > max {
        Some(format!("{length:.1} s is long: Apple wants {min:.0}–{max:.0} s"))
    } else {
        None
    }
}

/// Bits per second for a target's video: Apple's 10–12 Mbps for app
/// previews, more for the 4K-wide header.
fn bitrate(target: Target) -> u32 {
    match target {
        Target::Iphone | Target::Ipad => 11_000_000,
        Target::Header | Target::Search => 24_000_000,
    }
}

/// Writes the design's video to `out` (an .mp4): every frame rendered as
/// the export renders stills, at the target's video size. `images_dir`
/// holds the design's images and recordings. `progress` hears the share
/// done (0 to 1) after each frame, and stops the export by returning false.
pub fn export(design: &Design, images_dir: &Path, out: &Path, mut progress: impl FnMut(f32) -> bool) -> Result<()> {
    let video = design.video.clone().ok_or(Error::NotVideo)?;
    let size = design.target.video_size();
    let fps = video.fps.clamp(1, 60);
    let frames = (motion::duration(&design.slides) * fps as f32).round().max(1.) as i64;
    let (width, height) = size.pixels();
    #[cfg(target_os = "macos")]
    {
        // Frames don't depend on each other: each worker renders every
        // n-th one with its own decoder, and the encoder takes them back in
        // order, one lane at a time. A lane holds one finished frame, which
        // bounds memory even for the 4K-wide header.
        let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 8) as i64;
        let fps_f = fps as f32;
        std::thread::scope(|scope| {
            let lanes: Vec<_> = (0..workers)
                .map(|lane| {
                    let (send, receive) = std::sync::mpsc::sync_channel::<Result<_>>(1);
                    scope.spawn(move || {
                        let clips = Clips::new();
                        let mut index = lane;
                        while index < frames {
                            let frame = render_frame(design, index as f32 / fps_f, size, images_dir, 1.0, &clips)
                                .map_err(Error::from);
                            if send.send(frame).is_err() {
                                return;
                            }
                            index += workers;
                        }
                    });
                    receive
                })
                .collect();
            let mut encoder = Encoder::create(out, width, height, fps, bitrate(design.target))?;
            for index in 0..frames {
                let frame = lanes[(index % workers) as usize]
                    .recv()
                    .map_err(|_| Error::Writer("a frame renderer stopped".into()))??;
                encoder.push(&frame, index)?;
                if !progress((index + 1) as f32 / frames as f32) {
                    encoder.cancel();
                    let _ = std::fs::remove_file(out);
                    return Ok(());
                }
            }
            encoder.finish()
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (images_dir, out, &mut progress, width, height, frames);
        Err(Error::Unsupported)
    }
}

/// The decoder for screen recordings on this platform, or none.
pub fn clip_source() -> Box<dyn appdoctor_screenshots::ClipSource> {
    #[cfg(target_os = "macos")]
    {
        Box::new(Clips::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(appdoctor_screenshots::NoClips)
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use appdoctor_screenshots::tiny_skia::{Color, Pixmap};
    use appdoctor_screenshots::{ClipSource, Slide, Video};

    use super::*;

    #[test]
    fn encodes_a_video_and_reads_its_frames_back() {
        let dir = std::env::temp_dir().join(format!("appdoctor-video-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bars.mp4");

        // One second of red, then one of blue.
        let mut encoder = Encoder::create(&path, 320, 240, 30, 2_000_000).unwrap();
        for index in 0..60 {
            let mut frame = Pixmap::new(320, 240).unwrap();
            frame.fill(if index < 30 {
                Color::from_rgba8(255, 0, 0, 255)
            } else {
                Color::from_rgba8(0, 0, 255, 255)
            });
            encoder.push(&frame, index).unwrap();
        }
        encoder.finish().unwrap();

        let length = clip_duration(&path).unwrap();
        assert!((length - 2.).abs() < 0.1, "{length}");
        let clips = Clips::new();
        let red = clips.frame(&path, 0.5, (160, 120)).unwrap();
        let blue = clips.frame(&path, 1.5, (160, 120)).unwrap();
        let px = |p: &Pixmap| {
            let c = p.pixel(p.width() / 2, p.height() / 2).unwrap();
            (c.red(), c.blue())
        };
        assert!(px(&red).0 > 200 && px(&red).1 < 60, "{:?}", px(&red));
        assert!(px(&blue).1 > 200 && px(&blue).0 < 60, "{:?}", px(&blue));
        // Past the end, the last frame holds.
        assert!(clips.frame(&path, 9., (160, 120)).is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn exports_a_designs_video() {
        let dir = std::env::temp_dir().join(format!("appdoctor-export-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut id = 0;
        let design = Design {
            version: 2,
            target: Target::Search,
            video: Some(Video { fps: 10, ..Video::default() }),
            slides: vec![
                Slide {
                    duration: 1.,
                    ..Slide::starter(Target::Search, "One", &mut id)
                },
                Slide {
                    duration: 1.,
                    ..Slide::starter(Target::Search, "Two", &mut id)
                },
            ],
        };
        let out = dir.join("search.mp4");
        let mut calls = 0;
        export(&design, &dir, &out, |_| {
            calls += 1;
            true
        })
        .unwrap();
        assert_eq!(calls, 20);
        assert!((clip_duration(&out).unwrap() - 2.).abs() < 0.15);
        assert!(length_problem(&design).unwrap().contains("short"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
