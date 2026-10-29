//! App Store screenshot designs.
//!
//! A [`Design`] is a list of [`Slide`]s: a background, a headline and
//! subheadline, and a device frame holding one of the app's screenshots.
//! [`to_svg`] lays a slide out for a canvas size; [`render_png`] rasterises
//! it with resvg. The designer's live preview and the export both go
//! through `render_png`, so what the user sees is what they upload.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia;
use resvg::usvg;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("couldn't lay out the slide: {0}")]
    Svg(#[from] usvg::Error),
    #[error("couldn't allocate a {0}×{1} image")]
    Canvas(u32, u32),
    #[error("couldn't encode PNG: {0}")]
    Png(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// App Store screenshot sizes (portrait).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasSize {
    /// 6.9" iPhone (iPhone 16 Pro Max class), 1320 × 2868. Apple's
    /// required size for new submissions.
    Iphone69,
    /// 6.5" iPhone, 1242 × 2688.
    Iphone65,
}

impl CanvasSize {
    pub const ALL: [CanvasSize; 2] = [CanvasSize::Iphone69, CanvasSize::Iphone65];

    pub fn pixels(self) -> (u32, u32) {
        match self {
            CanvasSize::Iphone69 => (1320, 2868),
            CanvasSize::Iphone65 => (1242, 2688),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CanvasSize::Iphone69 => "iPhone 6.9\"",
            CanvasSize::Iphone65 => "iPhone 6.5\"",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            CanvasSize::Iphone69 => "iphone-6.9",
            CanvasSize::Iphone65 => "iphone-6.5",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Design {
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub background: Background,
    pub headline: Caption,
    pub subheadline: Caption,
    pub device: Device,
}

impl Default for Slide {
    fn default() -> Self {
        Self {
            background: Background::Gradient {
                from: "#4f46e5".into(),
                to: "#9333ea".into(),
                angle: 160.,
            },
            headline: Caption {
                text: "Your headline here".into(),
                color: "#ffffff".into(),
                size: 104.,
                weight: 700,
            },
            subheadline: Caption {
                text: "One line on why it matters".into(),
                color: "#e0e7ff".into(),
                size: 54.,
                weight: 400,
            },
            device: Device::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Background {
    Solid {
        color: String,
    },
    /// CSS-style angle: 0° runs bottom to top, 90° left to right, 180° top to bottom.
    Gradient {
        from: String,
        to: String,
        angle: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Caption {
    pub text: String,
    pub color: String,
    /// Font size in pixels on the 1320-wide canvas; scaled for other sizes.
    pub size: f32,
    pub weight: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frame {
    Black,
    Silver,
    /// The screenshot with rounded corners and no bezel.
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub frame: Frame,
    /// Device width as a share of the canvas width at 1.0; 0.5–1.1.
    pub scale: f32,
    /// Extra vertical shift as a share of canvas height (negative moves up).
    pub offset: f32,
    /// File name of the screenshot inside the design's folder.
    pub screenshot: Option<String>,
}

impl Default for Device {
    fn default() -> Self {
        Self {
            frame: Frame::Black,
            scale: 1.,
            offset: 0.,
            screenshot: None,
        }
    }
}

/// The base canvas width designs are authored at.
const BASE_WIDTH: f32 = 1320.;
/// iPhone screens are about 19.5:9.
const DEVICE_ASPECT: f32 = 2.167;
const FONT_FAMILY: &str = "Helvetica Neue, Segoe UI, Arial, sans-serif";

/// The slide as SVG for `size`. Screenshot `href`s are file names, resolved
/// against the design's folder when rendering.
pub fn to_svg(slide: &Slide, size: CanvasSize) -> String {
    let (w, h) = size.pixels();
    let (w, h) = (w as f32, h as f32);
    let k = w / BASE_WIDTH;
    let mut svg = String::with_capacity(4096);
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><defs>"#
    ));

    // Background.
    let fill = match &slide.background {
        Background::Solid { color } => color_attr(color),
        Background::Gradient { from, to, angle } => {
            let (x1, y1, x2, y2) = gradient_vector(*angle);
            svg.push_str(&format!(
                r#"<linearGradient id="bg" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient>"#,
                color_attr(from),
                color_attr(to)
            ));
            "url(#bg)".into()
        }
    };

    // Text block from the top.
    let margin_x = w * 0.07;
    let text_width = w - margin_x * 2.;
    let mut y = h * 0.075;
    let mut text = String::new();
    for caption in [&slide.headline, &slide.subheadline] {
        let font_size = caption.size * k;
        let line_height = font_size * 1.16;
        let lines = wrap(&caption.text, font_size, text_width);
        if lines.is_empty() {
            continue;
        }
        y += font_size;
        for (i, line) in lines.iter().enumerate() {
            text.push_str(&format!(
                r#"<text x="{}" y="{}" font-family="{FONT_FAMILY}" font-size="{font_size}" font-weight="{}" fill="{}" text-anchor="middle">{}</text>"#,
                w / 2.,
                y + i as f32 * line_height,
                caption.weight,
                color_attr(&caption.color),
                escape(line)
            ));
        }
        y += (lines.len() - 1) as f32 * line_height + font_size * 0.45;
    }

    // Device.
    let device_w = w * 0.78 * slide.device.scale.clamp(0.5, 1.1);
    let device_h = device_w * DEVICE_ASPECT;
    let device_x = (w - device_w) / 2.;
    let device_y = y + h * 0.03 + slide.device.offset * h;
    let radius = device_w * 0.135;
    let (bezel, inset) = match slide.device.frame {
        Frame::Black => (Some("#111114"), device_w * 0.034),
        Frame::Silver => (Some("#d9dadf"), device_w * 0.034),
        Frame::None => (None, 0.),
    };
    let screen_x = device_x + inset;
    let screen_y = device_y + inset;
    let screen_w = device_w - inset * 2.;
    let screen_h = device_h - inset * 2.;
    let screen_r = (radius - inset).max(radius * 0.6);
    svg.push_str(&format!(
        r##"<clipPath id="screen"><rect x="{screen_x}" y="{screen_y}" width="{screen_w}" height="{screen_h}" rx="{screen_r}"/></clipPath><filter id="shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="{}" stdDeviation="{}" flood-color="#000000" flood-opacity="0.35"/></filter></defs>"##,
        24. * k,
        44. * k
    ));
    svg.push_str(&format!(r#"<rect width="{w}" height="{h}" fill="{fill}"/>"#));
    svg.push_str(&text);

    let outline = match bezel {
        Some(color) => format!(
            r#"<rect x="{device_x}" y="{device_y}" width="{device_w}" height="{device_h}" rx="{radius}" fill="{color}" filter="url(#shadow)"/>"#
        ),
        None => format!(
            r##"<rect x="{screen_x}" y="{screen_y}" width="{screen_w}" height="{screen_h}" rx="{screen_r}" fill="#000000" filter="url(#shadow)"/>"##
        ),
    };
    svg.push_str(&outline);
    match &slide.device.screenshot {
        Some(file) => svg.push_str(&format!(
            r#"<image x="{screen_x}" y="{screen_y}" width="{screen_w}" height="{screen_h}" preserveAspectRatio="xMidYMid slice" clip-path="url(#screen)" xlink:href="{}"/>"#,
            escape(file)
        )),
        None => svg.push_str(&format!(
            r##"<rect x="{screen_x}" y="{screen_y}" width="{screen_w}" height="{screen_h}" rx="{screen_r}" fill="#1c1c1f"/><text x="{}" y="{}" font-family="{FONT_FAMILY}" font-size="{}" fill="#8e8e93" text-anchor="middle">Add a screenshot</text>"##,
            w / 2.,
            screen_y + screen_h * 0.45,
            44. * k
        )),
    }
    if bezel.is_some() {
        // The Dynamic Island.
        let island_w = device_w * 0.3;
        let island_h = device_w * 0.085;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{island_w}" height="{island_h}" rx="{}" fill="#000000"/>"##,
            (w - island_w) / 2.,
            screen_y + device_w * 0.028,
            island_h / 2.
        ));
    }
    svg.push_str("</svg>");
    svg
}

/// Renders the slide to PNG bytes at `scale` × the canvas size (1.0 for
/// export, smaller for previews). `images_dir` holds the screenshots.
pub fn render_png(slide: &Slide, size: CanvasSize, images_dir: &Path, scale: f32) -> Result<Vec<u8>> {
    let options = usvg::Options {
        resources_dir: Some(images_dir.to_path_buf()),
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&to_svg(slide, size), &options)?;
    let (w, h) = size.pixels();
    let (pw, ph) = (((w as f32) * scale).round() as u32, ((h as f32) * scale).round() as u32);
    let mut pixmap = tiny_skia::Pixmap::new(pw.max(1), ph.max(1)).ok_or(Error::Canvas(pw, ph))?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().map_err(|error| Error::Png(error.to_string()))
}

/// System fonts, loaded once: scanning them takes a moment.
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

/// The gradient vector in objectBoundingBox units for a CSS angle.
fn gradient_vector(angle: f32) -> (f32, f32, f32, f32) {
    let radians = angle.to_radians();
    let (dx, dy) = (radians.sin() / 2., -radians.cos() / 2.);
    (0.5 - dx, 0.5 - dy, 0.5 + dx, 0.5 + dy)
}

/// Greedy word wrap by estimated glyph width (about half the font size for
/// Latin letters, a full em for CJK). Explicit newlines always break.
pub fn wrap(text: &str, font_size: f32, max_width: f32) -> Vec<String> {
    let char_width = |c: char| {
        if c.is_ascii() {
            0.54
        } else if c.len_utf8() >= 3 {
            1.0
        } else {
            0.6
        }
    };
    let width = |s: &str| s.chars().map(char_width).sum::<f32>() * font_size;
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if width(&candidate) > max_width && !line.is_empty() {
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    lines
}

/// `#rrggbb` or `#rgb`, else a safe default, so user input can't break the SVG.
fn color_attr(color: &str) -> String {
    let hex = color.trim().trim_start_matches('#');
    if (hex.len() == 6 || hex.len() == 3) && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{hex}")
    } else {
        "#000000".into()
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

impl Design {
    pub fn load(path: &Path) -> Design {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        // Write then rename, so a crash never leaves half a design.
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json)?;
        std::fs::rename(temp, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_by_width_and_newlines() {
        assert_eq!(wrap("Plan your day", 100., 2000.), vec!["Plan your day"]);
        assert_eq!(
            wrap("Plan your whole day in seconds", 100., 1000.),
            vec!["Plan your whole", "day in seconds"]
        );
        assert_eq!(wrap("One\nTwo", 100., 2000.), vec!["One", "Two"]);
        assert!(wrap("   ", 100., 100.).is_empty());
    }

    #[test]
    fn svg_escapes_text_and_sanitises_colors() {
        let mut slide = Slide::default();
        slide.headline.text = "Tasks & <notes>".into();
        slide.headline.color = "red\" onload=\"x".into();
        let svg = to_svg(&slide, CanvasSize::Iphone69);
        assert!(svg.contains("Tasks &amp; &lt;notes&gt;"));
        assert!(!svg.contains("onload"));
        assert!(svg.contains(r#"width="1320" height="2868""#));
        assert!(svg.contains("Add a screenshot"));
    }

    #[test]
    fn gradient_angles_follow_css() {
        let close = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| {
            (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4 && (a.2 - b.2).abs() < 1e-4 && (a.3 - b.3).abs() < 1e-4
        };
        assert!(
            close(gradient_vector(180.), (0.5, 0., 0.5, 1.)),
            "180° runs top to bottom"
        );
        assert!(
            close(gradient_vector(90.), (0., 0.5, 1., 0.5)),
            "90° runs left to right"
        );
    }

    #[test]
    fn renders_png_at_export_and_preview_sizes() {
        let dir = std::env::temp_dir();
        let slide = Slide::default();
        for (size, scale) in [(CanvasSize::Iphone69, 1.0), (CanvasSize::Iphone65, 0.25)] {
            let png = render_png(&slide, size, &dir, scale).unwrap();
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            let (w, h) = size.pixels();
            let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
            assert_eq!(
                (width, height),
                (((w as f32) * scale).round() as u32, ((h as f32) * scale).round() as u32)
            );
        }
    }

    #[test]
    fn design_round_trips_to_disk() {
        let path = std::env::temp_dir().join(format!("appdoctor-design-{}/design.json", std::process::id()));
        let design = Design {
            slides: vec![Slide::default(), Slide::default()],
        };
        design.save(&path).unwrap();
        assert_eq!(Design::load(&path), design);
        assert_eq!(Design::load(Path::new("/nonexistent/design.json")), Design::default());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
