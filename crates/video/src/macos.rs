//! AVFoundation: `AVAssetWriter` encodes H.264 on the hardware encoder,
//! and `AVAssetImageGenerator` reads frames out of screen recordings.
//! Neither is `Send`: make and use them on one thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc;
use std::time::Duration;

use appdoctor_screenshots::ClipSource;
use appdoctor_screenshots::tiny_skia::Pixmap;
use block2::RcBlock;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetImageGenerator, AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor,
    AVAssetWriterStatus, AVFileTypeMPEG4, AVMediaTypeVideo, AVURLAsset, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoCompressionPropertiesKey, AVVideoExpectedSourceFrameRateKey, AVVideoHeightKey,
    AVVideoMaxKeyFrameIntervalKey, AVVideoProfileLevelH264HighAutoLevel, AVVideoProfileLevelKey, AVVideoWidthKey,
};
use objc2_core_foundation::{CFRetained, CFString, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGColorSpace, CGContext, CGImage, CGImageAlphaInfo, CGImageByteOrderInfo,
};
use objc2_core_media::{CMTime, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferPool, CVPixelBufferUnlockBaseAddress, kCVPixelBufferHeightKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSDictionary, NSError, NSNumber, NSString, NSURL};

use crate::{Error, Result};

fn ns_error(error: Retained<NSError>) -> Error {
    Error::Writer(error.localizedDescription().to_string())
}

fn url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

/// CoreFoundation strings are NSStrings (toll-free bridged).
fn bridged(key: &'static CFString) -> &'static NSString {
    // SAFETY: CFString and NSString are the same object.
    unsafe { &*(key as *const CFString as *const NSString) }
}

fn settings(pairs: &[(&NSString, &AnyObject)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<&NSString> = pairs.iter().map(|(key, _)| *key).collect();
    let values: Vec<&AnyObject> = pairs.iter().map(|(_, value)| *value).collect();
    NSDictionary::from_slices(&keys, &values)
}

/// Encodes frames (premultiplied RGBA, as tiny-skia draws them) to an
/// H.264 MP4.
pub struct Encoder {
    writer: Retained<AVAssetWriter>,
    input: Retained<AVAssetWriterInput>,
    adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
    fps: i32,
    size: (u32, u32),
}

impl Encoder {
    /// A new MP4 at `path`, replacing any file there.
    pub fn create(path: &Path, width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Encoder> {
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        unsafe {
            let missing = || Error::Writer("AVFoundation is missing a setting".into());
            let writer = AVAssetWriter::assetWriterWithURL_fileType_error(&url(path), AVFileTypeMPEG4.ok_or_else(missing)?)
                .map_err(ns_error)?;
            let (bitrate, fps_number, keyframes) =
                (NSNumber::new_u32(bitrate), NSNumber::new_u32(fps), NSNumber::new_u32(fps * 2));
            let compression = settings(&[
                (AVVideoAverageBitRateKey.ok_or_else(missing)?, bitrate.as_ref()),
                (AVVideoExpectedSourceFrameRateKey.ok_or_else(missing)?, fps_number.as_ref()),
                (AVVideoMaxKeyFrameIntervalKey.ok_or_else(missing)?, keyframes.as_ref()),
                (
                    AVVideoProfileLevelKey.ok_or_else(missing)?,
                    AVVideoProfileLevelH264HighAutoLevel.ok_or_else(missing)?.as_ref(),
                ),
            ]);
            let (w, h) = (NSNumber::new_u32(width), NSNumber::new_u32(height));
            let output = settings(&[
                (AVVideoCodecKey.ok_or_else(missing)?, AVVideoCodecTypeH264.ok_or_else(missing)?.as_ref()),
                (AVVideoWidthKey.ok_or_else(missing)?, w.as_ref()),
                (AVVideoHeightKey.ok_or_else(missing)?, h.as_ref()),
                (AVVideoCompressionPropertiesKey.ok_or_else(missing)?, compression.as_ref()),
            ]);
            let input = AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                AVMediaTypeVideo.ok_or_else(missing)?,
                Some(&output),
            );
            input.setExpectsMediaDataInRealTime(false);
            let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
            let source = settings(&[
                (bridged(kCVPixelBufferPixelFormatTypeKey), format.as_ref()),
                (bridged(kCVPixelBufferWidthKey), w.as_ref()),
                (bridged(kCVPixelBufferHeightKey), h.as_ref()),
            ]);
            let adaptor = AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &input,
                Some(&source),
            );
            if !writer.canAddInput(&input) {
                return Err(Error::Writer("the encoder doesn't take this size".into()));
            }
            writer.addInput(&input);
            if !writer.startWriting() {
                return Err(writer.error().map_or_else(|| Error::Writer("couldn't start".into()), ns_error));
            }
            writer.startSessionAtSourceTime(kCMTimeZero);
            Ok(Encoder {
                writer,
                input,
                adaptor,
                fps: fps as i32,
                size: (width, height),
            })
        }
    }

    fn failed(&self) -> Error {
        unsafe { self.writer.error() }.map_or_else(|| Error::Writer("the encoder stopped".into()), ns_error)
    }

    /// Adds frame `index` (shown at index / fps seconds).
    pub fn push(&mut self, frame: &Pixmap, index: i64) -> Result<()> {
        if (frame.width(), frame.height()) != self.size {
            return Err(Error::Writer(format!(
                "frame is {}×{}, the video {}×{}",
                frame.width(),
                frame.height(),
                self.size.0,
                self.size.1
            )));
        }
        autoreleasepool(|_| unsafe {
            while !self.input.isReadyForMoreMediaData() {
                if self.writer.status() == AVAssetWriterStatus::Failed {
                    return Err(self.failed());
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            let pool: Retained<CVPixelBufferPool> = self.adaptor.pixelBufferPool().ok_or_else(|| self.failed())?;
            let mut raw: *mut CVPixelBuffer = std::ptr::null_mut();
            let status = CVPixelBufferPool::create_pixel_buffer(None, &pool, NonNull::from(&mut raw));
            let buffer: CFRetained<CVPixelBuffer> = match NonNull::new(raw) {
                Some(raw) if status == 0 => CFRetained::from_raw(raw),
                _ => return Err(Error::Writer(format!("no pixel buffer ({status})"))),
            };
            CVPixelBufferLockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            let base = CVPixelBufferGetBaseAddress(&buffer) as *mut u8;
            let row = CVPixelBufferGetBytesPerRow(&buffer);
            let (width, height) = (self.size.0 as usize, self.size.1 as usize);
            let source = frame.data();
            for y in 0..height {
                let dst = std::slice::from_raw_parts_mut(base.add(y * row), width * 4);
                let src = &source[y * width * 4..(y + 1) * width * 4];
                // RGBA to BGRA; frames are opaque, so premultiplying changes nothing.
                for (d, s) in dst.as_chunks_mut::<4>().0.iter_mut().zip(src.as_chunks::<4>().0) {
                    d[0] = s[2];
                    d[1] = s[1];
                    d[2] = s[0];
                    d[3] = 255;
                }
            }
            CVPixelBufferUnlockBaseAddress(&buffer, CVPixelBufferLockFlags(0));
            if self
                .adaptor
                .appendPixelBuffer_withPresentationTime(&buffer, CMTime::new(index, self.fps))
            {
                Ok(())
            } else {
                Err(self.failed())
            }
        })
    }

    /// Finishes the file and waits for it to be written.
    pub fn finish(self) -> Result<()> {
        unsafe {
            self.input.markAsFinished();
            let (done, wait) = mpsc::channel();
            let block = RcBlock::new(move || {
                let _ = done.send(());
            });
            self.writer.finishWritingWithCompletionHandler(&block);
            let _ = wait.recv_timeout(Duration::from_secs(120));
            if self.writer.status() == AVAssetWriterStatus::Completed {
                Ok(())
            } else {
                Err(self.failed())
            }
        }
    }

    /// Stops without finishing the file.
    pub fn cancel(self) {
        unsafe { self.writer.cancelWriting() }
    }
}

/// A recording's frame generator and its length, or `None` when the file
/// couldn't be read.
type Generator = Option<(Retained<AVAssetImageGenerator>, f32)>;

/// Frames from screen recordings, one generator per file.
pub struct Clips {
    generators: RefCell<HashMap<PathBuf, Generator>>,
}

impl Default for Clips {
    fn default() -> Self {
        Clips::new()
    }
}

impl Clips {
    pub fn new() -> Clips {
        Clips {
            generators: RefCell::new(HashMap::new()),
        }
    }
}

/// A recording's length in seconds.
pub fn clip_duration(path: &Path) -> Option<f32> {
    if !path.is_file() {
        return None;
    }
    autoreleasepool(|_| unsafe {
        let asset = AVURLAsset::URLAssetWithURL_options(&url(path), None);
        let seconds = asset.duration().seconds();
        (seconds.is_finite() && seconds > 0.).then_some(seconds as f32)
    })
}

impl ClipSource for Clips {
    fn frame(&self, path: &Path, time: f32, (width, height): (u32, u32)) -> Option<Pixmap> {
        autoreleasepool(|_| unsafe {
            let mut generators = self.generators.borrow_mut();
            let entry = generators.entry(path.to_path_buf()).or_insert_with(|| {
                let length = clip_duration(path)?;
                let asset = AVURLAsset::URLAssetWithURL_options(&url(path), None);
                let generator = AVAssetImageGenerator::assetImageGeneratorWithAsset(&asset);
                generator.setAppliesPreferredTrackTransform(true);
                // Close to the asked time, not the nearest keyframe.
                let tolerance = CMTime::new(1, 60);
                generator.setRequestedTimeToleranceBefore(tolerance);
                generator.setRequestedTimeToleranceAfter(tolerance);
                Some((generator, length))
            });
            let (generator, length) = entry.as_ref()?;
            generator.setMaximumSize(CGSize {
                width: width.max(1) as f64,
                height: height.max(1) as f64,
            });
            // Past the end, the last frame holds.
            let at = time.clamp(0., (length - 1. / 30.).max(0.));
            // The asynchronous replacement wants a callback per frame; this
            // runs on a worker thread, where waiting is what we want.
            #[allow(deprecated)]
            let image = generator
                .copyCGImageAtTime_actualTime_error(CMTime::new((at * 600.) as i64, 600), std::ptr::null_mut())
                .ok()?;
            pixmap(&image)
        })
    }
}

/// A CGImage drawn into premultiplied RGBA, tiny-skia's layout.
fn pixmap(image: &CGImage) -> Option<Pixmap> {
    let (width, height) = (CGImage::width(Some(image)), CGImage::height(Some(image)));
    let mut pixmap = Pixmap::new(width as u32, height as u32)?;
    unsafe {
        let space = CGColorSpace::new_device_rgb()?;
        let context: CFRetained<CGContext> = CGBitmapContextCreate(
            pixmap.data_mut().as_mut_ptr() as *mut c_void,
            width,
            height,
            8,
            width * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0 | CGImageByteOrderInfo::Order32Big.0,
        )?;
        CGContext::draw_image(
            Some(&context),
            CGRect {
                origin: CGPoint { x: 0., y: 0. },
                size: CGSize {
                    width: width as f64,
                    height: height as f64,
                },
            },
            Some(image),
        );
    }
    Some(pixmap)
}
