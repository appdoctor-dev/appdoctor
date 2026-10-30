//! Rasterising designs: stills of a slide, and frames of a video at any
//! moment. Both go through the same [`Pieces`], so a frame matches the
//! still apart from its motion.

use std::path::Path;

use resvg::tiny_skia::{self, FillRule, FilterQuality, Mask, PathBuilder, Pixmap, PixmapPaint, Transform};
use resvg::usvg;

use crate::motion::{Transition, moment, scene_layers};
use crate::svg::{Piece, Pieces, document, still_layers};
use crate::{CanvasSize, Design, Error, LayerKind, Rect, Result, Slide, text};

/// Frames of screen recordings, decoded by the platform (AVFoundation on
/// macOS; see `appdoctor-video`).
pub trait ClipSource {
    /// The frame `time` seconds into the recording at `path`, at about
    /// `size` pixels (it is cover-fitted to the screen either way).
    fn frame(&self, path: &Path, time: f32, size: (u32, u32)) -> Option<Pixmap>;
}

/// No decoder: recordings show as a dark screen.
pub struct NoClips;

impl ClipSource for NoClips {
    fn frame(&self, _: &Path, _: f32, _: (u32, u32)) -> Option<Pixmap> {
        None
    }
}

/// Renders slide `index` to PNG bytes at `scale` × `size` (1.0 for export,
/// smaller for thumbnails), including layers spanning in from its
/// neighbours. The slides are laid out on `size`'s target canvas.
/// `images_dir` holds the design's images.
pub fn render_png(slides: &[Slide], index: usize, size: CanvasSize, images_dir: &Path, scale: f32) -> Result<Vec<u8>> {
    render_png_with(slides, index, size, images_dir, scale, &NoClips)
}

/// [`render_png`], with screen recordings showing their first frame.
pub fn render_png_with(
    slides: &[Slide],
    index: usize,
    size: CanvasSize,
    images_dir: &Path,
    scale: f32,
    clips: &dyn ClipSource,
) -> Result<Vec<u8>> {
    render_slide(slides, index, size, images_dir, scale, clips)?
        .encode_png()
        .map_err(|error| Error::Png(error.to_string()))
}

/// A still of slide `index` as designed, without motion.
pub fn render_slide(
    slides: &[Slide],
    index: usize,
    size: CanvasSize,
    images_dir: &Path,
    scale: f32,
    clips: &dyn ClipSource,
) -> Result<Pixmap> {
    let canvas = size.target().canvas();
    let mut pieces = Pieces::new();
    if let Some(slide) = slides.get(index) {
        pieces.background(&slide.background, 0., canvas, 1.);
    }
    for (_, layer) in still_layers(slides, index, size) {
        let start = match &layer.kind {
            LayerKind::Device { clip_start, .. } => *clip_start,
            _ => 0.,
        };
        pieces.layer(&layer, start, 1., true);
    }
    rasterize(pieces.finish(), size, images_dir, scale, clips)
}

/// What the design's video shows `time` seconds in, at `scale` × `size`.
pub fn render_frame(
    design: &Design,
    time: f32,
    size: CanvasSize,
    images_dir: &Path,
    scale: f32,
    clips: &dyn ClipSource,
) -> Result<Pixmap> {
    let video = design.video.clone().unwrap_or_default();
    let slides = &design.slides;
    let canvas = size.target().canvas();
    let spans = size.target().is_screenshot();
    let at = moment(slides, &video, time);
    let mut pieces = Pieces::new();
    let scene = |pieces: &mut Pieces, index: usize, time: f32, offset: f32, opacity: f32, skip: Option<usize>| {
        let Some(slide) = slides.get(index) else {
            return;
        };
        pieces.background(&slide.background, offset, canvas, opacity);
        for (_, mut layer, clip) in scene_layers(slides, index, time, canvas, spans, skip) {
            layer.x += offset;
            pieces.layer(&layer, clip, opacity, true);
        }
    };
    match at.next {
        // Both scenes side by side, sliding left: layers spanning them stay
        // joined, drawn once by their owner.
        Some((next, p)) if video.transition == Transition::Push => {
            scene(&mut pieces, at.scene, at.time, -p * canvas.0, 1., Some(next));
            scene(&mut pieces, next, 0., (1. - p) * canvas.0, 1., Some(at.scene));
        }
        Some((next, p)) => {
            scene(&mut pieces, at.scene, at.time, 0., 1., None);
            scene(&mut pieces, next, 0., 0., p, None);
        }
        None => scene(&mut pieces, at.scene, at.time, 0., 1., None),
    }
    rasterize(pieces.finish(), size, images_dir, scale, clips)
}

fn rasterize(pieces: Vec<Piece>, size: CanvasSize, images_dir: &Path, scale: f32, clips: &dyn ClipSource) -> Result<Pixmap> {
    let (w, h) = size.pixels();
    let (pw, ph) = (((w as f32) * scale).round() as u32, ((h as f32) * scale).round() as u32);
    let mut pixmap = Pixmap::new(pw.max(1), ph.max(1)).ok_or(Error::Canvas(pw, ph))?;
    let options = usvg::Options {
        resources_dir: Some(images_dir.to_path_buf()),
        fontdb: text::fonts(),
        ..Default::default()
    };
    // Canvas pixels to output pixels.
    let k = w as f32 / size.target().canvas().0 * scale;
    for piece in pieces {
        match piece {
            Piece::Svg { defs, body } => {
                let tree = usvg::Tree::from_str(&document(&defs, &body, size), &options)?;
                resvg::render(&tree, Transform::from_scale(scale, scale), &mut pixmap.as_mut());
            }
            Piece::Clip {
                file,
                time,
                screen,
                radius,
                opacity,
            } => {
                let screen = Rect {
                    x: screen.x * k,
                    y: screen.y * k,
                    width: screen.width * k,
                    height: screen.height * k,
                };
                let want = (screen.width.ceil().max(1.) as u32, screen.height.ceil().max(1.) as u32);
                if let Some(frame) = clips.frame(&images_dir.join(&file), time, want) {
                    composite(&mut pixmap, &frame, screen, radius * k, opacity);
                }
            }
        }
    }
    Ok(pixmap)
}

/// Draws `frame` cover-fitted into `screen`, clipped to its rounded corners.
fn composite(pixmap: &mut Pixmap, frame: &Pixmap, screen: Rect, radius: f32, opacity: f32) {
    let (fw, fh) = (frame.width() as f32, frame.height() as f32);
    let s = (screen.width / fw).max(screen.height / fh);
    let (tx, ty) = (
        screen.x + (screen.width - fw * s) / 2.,
        screen.y + (screen.height - fh * s) / 2.,
    );
    let Some(path) = rounded_rect(screen, radius) else {
        return;
    };
    let Some(mut mask) = Mask::new(pixmap.width(), pixmap.height()) else {
        return;
    };
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    pixmap.draw_pixmap(
        0,
        0,
        frame.as_ref(),
        &PixmapPaint {
            opacity,
            quality: FilterQuality::Bicubic,
            ..Default::default()
        },
        Transform::from_row(s, 0., 0., s, tx, ty),
        Some(&mask),
    );
}

fn rounded_rect(r: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let radius = radius.min(r.width / 2.).min(r.height / 2.).max(0.);
    if radius == 0. {
        return Some(PathBuilder::from_rect(tiny_skia::Rect::from_xywh(r.x, r.y, r.width, r.height)?));
    }
    // Cubic quarter circles (0.5523 of the radius places the handles).
    let c = radius * 0.5523;
    let (left, top, right, bottom) = (r.x, r.y, r.x + r.width, r.y + r.height);
    let mut path = PathBuilder::new();
    path.move_to(left + radius, top);
    path.line_to(right - radius, top);
    path.cubic_to(right - radius + c, top, right, top + radius - c, right, top + radius);
    path.line_to(right, bottom - radius);
    path.cubic_to(right, bottom - radius + c, right - radius + c, bottom, right - radius, bottom);
    path.line_to(left + radius, bottom);
    path.cubic_to(left + radius - c, bottom, left, bottom - radius + c, left, bottom - radius);
    path.line_to(left, top + radius);
    path.cubic_to(left, top + radius - c, left + radius - c, top, left + radius, top);
    path.close();
    path.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Effect, Target, Video};

    /// A recording that is solid red, whatever the time.
    struct Red;

    impl ClipSource for Red {
        fn frame(&self, _: &Path, _: f32, (w, h): (u32, u32)) -> Option<Pixmap> {
            let mut frame = Pixmap::new(w, h)?;
            frame.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));
            Some(frame)
        }
    }

    fn design() -> Design {
        let mut id = 0;
        let mut first = Slide::starter(Target::Iphone, "One", &mut id);
        if let LayerKind::Device { screenshot, .. } = &mut first.layers[2].kind {
            *screenshot = Some("screen.mov".into());
        }
        let second = Slide::starter(Target::Iphone, "Two", &mut id);
        Design {
            version: 2,
            target: Target::Iphone,
            video: Some(Video::default()),
            slides: vec![first, second],
        }
    }

    fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> (u8, u8, u8) {
        let p = pixmap.pixel(x, y).unwrap();
        (p.red(), p.green(), p.blue())
    }

    #[test]
    fn recordings_play_inside_the_device_screen() {
        let design = design();
        let dir = std::env::temp_dir();
        let frame = render_frame(&design, 1., CanvasSize::Iphone69, &dir, 0.25, &Red).unwrap();
        // The middle of the phone's screen is the recording.
        let device = &design.slides[0].layers[2];
        let (cx, cy) = (
            ((device.x + device.width / 2.) * 0.25) as u32,
            ((device.y + device.height / 2.) * 0.25) as u32,
        );
        assert_eq!(pixel(&frame, cx, cy), (255, 0, 0));
        // Without a decoder the screen stays dark, and stills still render.
        let dark = render_slide(&design.slides, 0, CanvasSize::Iphone69, &dir, 0.25, &NoClips).unwrap();
        assert_ne!(pixel(&dark, cx, cy), (255, 0, 0));
    }

    #[test]
    fn pushes_slide_the_next_scene_in_from_the_right() {
        let mut design = design();
        if let LayerKind::Device { screenshot, .. } = &mut design.slides[0].layers[2].kind {
            *screenshot = None;
        }
        design.slides[1].background = crate::Background::Solid {
            color: "#00ff00".into(),
        };
        let dir = std::env::temp_dir();
        // Halfway through the push, the right edge shows the next scene.
        let frame = render_frame(&design, 2.75, CanvasSize::Iphone69, &dir, 0.1, &NoClips).unwrap();
        let (w, h) = (frame.width(), frame.height());
        // Green, give or take the phone's shadow.
        let green = |(r, g, b): (u8, u8, u8)| g > 200 && r < 30 && b < 30;
        assert!(green(pixel(&frame, w - 2, h - 2)));
        assert!(!green(pixel(&frame, 1, h - 2)));
    }

    #[test]
    fn entering_layers_are_hidden_before_their_time() {
        let mut design = design();
        let headline = &mut design.slides[0].layers[0];
        headline.motion.start = 1.;
        headline.motion.enter = Effect::Fade;
        let dir = std::env::temp_dir();
        let before = render_frame(&design, 0.5, CanvasSize::Iphone69, &dir, 0.1, &NoClips).unwrap();
        let after = render_frame(&design, 2., CanvasSize::Iphone69, &dir, 0.1, &NoClips).unwrap();
        assert_ne!(before.data(), after.data());
    }
}
