//! App Store screenshot designs.
//!
//! A [`Design`] is a list of [`Slide`]s; each slide is a background and a
//! stack of positioned [`Layer`]s (text, device, image, shape) in canvas
//! pixels on the 1320 × 2868 base canvas.
//!
//! Slides sit edge to edge, so a layer hanging off one slide continues on
//! its neighbour ([`visible_layers`]): panoramic sets need no copies.
//!
//! The layout rules the editor draws with live are here too: [`device_geometry`]
//! for the phone frame and [`wrap`] for text lines. [`to_svg`] and
//! [`render_png`] use the same functions for export, so the canvas and the
//! exported PNGs agree.

pub mod edit;
mod legacy;
mod svg;

use std::path::Path;
use std::sync::{Arc, OnceLock};

use resvg::tiny_skia;
use resvg::usvg;
use serde::{Deserialize, Serialize};

pub use svg::to_svg;

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

/// The canvas layers are positioned on.
pub const BASE_WIDTH: f32 = 1320.;
pub const BASE_HEIGHT: f32 = 2868.;

/// App Store screenshot sizes (portrait). Layers scale uniformly by width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasSize {
    /// 6.9" iPhone, 1320 × 2868: Apple's required size for new submissions.
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

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Design {
    pub version: u32,
    pub slides: Vec<Slide>,
}

impl Default for Design {
    fn default() -> Self {
        Self {
            version: 2,
            slides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub background: Background,
    /// Bottom to top.
    pub layers: Vec<Layer>,
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
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub locked: bool,
    pub kind: LayerKind,
}

fn one() -> f32 {
    1.
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LayerKind {
    Text {
        text: String,
        size: f32,
        weight: u16,
        color: String,
        align: Align,
    },
    Device {
        frame: Frame,
        /// File name inside the design's folder.
        screenshot: Option<String>,
        shadow: bool,
    },
    Image {
        file: Option<String>,
        radius: f32,
        shadow: bool,
    },
    Shape {
        fill: String,
        radius: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Frame {
    Black,
    Silver,
    /// The screenshot with rounded corners, no bezel.
    None,
}

impl Frame {
    pub fn bezel_color(self) -> Option<&'static str> {
        match self {
            Frame::Black => Some("#111114"),
            Frame::Silver => Some("#d9dadf"),
            Frame::None => None,
        }
    }
}

/// iPhone screens are about 19.5:9; device layers keep this aspect.
pub const DEVICE_ASPECT: f32 = 2.167;
pub const LINE_HEIGHT: f32 = 1.16;
pub const FONT_FAMILY: &str = "Helvetica Neue";
const SVG_FONT_FAMILY: &str = "Helvetica Neue, Segoe UI, Arial, sans-serif";

impl Layer {
    pub fn label(&self) -> &'static str {
        match self.kind {
            LayerKind::Text { .. } => "Text",
            LayerKind::Device { .. } => "Device",
            LayerKind::Image { .. } => "Image",
            LayerKind::Shape { .. } => "Shape",
        }
    }

    /// Whether resizing keeps the width:height ratio.
    pub fn keeps_aspect(&self) -> bool {
        matches!(self.kind, LayerKind::Device { .. })
    }

    pub fn text(id: u64, text: &str, y: f32, size: f32, weight: u16, color: &str) -> Layer {
        let lines = wrap(text, size, BASE_WIDTH - 184.).len().max(1) as f32;
        Layer {
            id,
            name: text.chars().take(24).collect(),
            x: 92.,
            y,
            width: BASE_WIDTH - 184.,
            height: (lines * size * LINE_HEIGHT).ceil(),
            opacity: 1.,
            hidden: false,
            locked: false,
            kind: LayerKind::Text {
                text: text.into(),
                size,
                weight,
                color: color.into(),
                align: Align::Center,
            },
        }
    }

    pub fn device(id: u64, y: f32) -> Layer {
        let width = BASE_WIDTH * 0.78;
        Layer {
            id,
            name: "iPhone".into(),
            x: (BASE_WIDTH - width) / 2.,
            y,
            width,
            height: width * DEVICE_ASPECT,
            opacity: 1.,
            hidden: false,
            locked: false,
            kind: LayerKind::Device {
                frame: Frame::Black,
                screenshot: None,
                shadow: true,
            },
        }
    }

    pub fn image(id: u64, file: String, width: f32, height: f32) -> Layer {
        Layer {
            id,
            name: "Image".into(),
            x: (BASE_WIDTH - width) / 2.,
            y: (BASE_HEIGHT - height) / 2.,
            width,
            height,
            opacity: 1.,
            hidden: false,
            locked: false,
            kind: LayerKind::Image {
                file: Some(file),
                radius: 36.,
                shadow: true,
            },
        }
    }

    pub fn shape(id: u64) -> Layer {
        Layer {
            id,
            name: "Shape".into(),
            x: 260.,
            y: 1200.,
            width: 800.,
            height: 400.,
            opacity: 1.,
            hidden: false,
            locked: false,
            kind: LayerKind::Shape {
                fill: "#ffffff".into(),
                radius: 48.,
            },
        }
    }
}

impl Slide {
    /// A fresh slide: gradient, headline, subheadline and a device.
    pub fn starter(headline: &str, next_id: &mut u64) -> Slide {
        let mut id = || {
            *next_id += 1;
            *next_id
        };
        Slide {
            background: Background::Gradient {
                from: "#4f46e5".into(),
                to: "#9333ea".into(),
                angle: 160.,
            },
            layers: vec![
                Layer::text(id(), headline, 200., 104., 700, "#ffffff"),
                Layer::text(id(), "One line on why it matters", 470., 54., 400, "#e0e7ff"),
                Layer::device(id(), 640.),
            ],
        }
    }
}

impl Design {
    /// The largest layer ID in use, for minting new ones.
    pub fn max_id(&self) -> u64 {
        self.slides
            .iter()
            .flat_map(|slide| &slide.layers)
            .map(|layer| layer.id)
            .max()
            .unwrap_or(0)
    }

    /// Loads a design, upgrading older saved formats. Missing or unreadable
    /// files give an empty design.
    pub fn load(path: &Path) -> Design {
        let Ok(bytes) = std::fs::read(path) else {
            return Design::default();
        };
        legacy::parse(&bytes).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        // Write then rename so a crash never leaves half a design.
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, json)?;
        std::fs::rename(temp, path)
    }
}

/// A rectangle in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Where a device layer's parts go, for both the live canvas and export.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceGeometry {
    pub body: Rect,
    pub body_radius: f32,
    pub screen: Rect,
    pub screen_radius: f32,
    /// The Dynamic Island, when there's a bezel.
    pub island: Option<Rect>,
}

pub fn device_geometry(layer: &Layer, frame: Frame) -> DeviceGeometry {
    let body = Rect {
        x: layer.x,
        y: layer.y,
        width: layer.width,
        height: layer.height,
    };
    let body_radius = layer.width * 0.135;
    let inset = if frame.bezel_color().is_some() {
        layer.width * 0.034
    } else {
        0.
    };
    let screen = Rect {
        x: body.x + inset,
        y: body.y + inset,
        width: body.width - inset * 2.,
        height: body.height - inset * 2.,
    };
    let screen_radius = (body_radius - inset).max(body_radius * 0.6);
    let island = frame.bezel_color().map(|_| {
        let width = layer.width * 0.3;
        let height = layer.width * 0.085;
        Rect {
            x: body.x + (body.width - width) / 2.,
            y: screen.y + layer.width * 0.028,
            width,
            height,
        }
    });
    DeviceGeometry {
        body,
        body_radius,
        screen,
        screen_radius,
        island,
    }
}

/// Greedy word wrap by estimated glyph width (about half the font size for
/// Latin letters, a full em for CJK). Explicit newlines always break. The
/// canvas and the export both lay text out with this.
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
        lines.push(line);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}

/// `#rrggbb`/`#rgb` parsed to RGB, for the canvas.
pub fn parse_color(color: &str) -> Option<(u8, u8, u8)> {
    let hex = color.trim().trim_start_matches('#');
    let full: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_owned(),
        _ => return None,
    };
    let value = u32::from_str_radix(&full, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// What slide `index` shows: its own visible layers, then the parts of
/// other slides' layers that reach onto it, shifted into its coordinates.
/// Each comes with the index of the slide that owns it.
pub fn visible_layers(slides: &[Slide], index: usize) -> Vec<(usize, Layer)> {
    let mut layers: Vec<(usize, Layer)> = slides
        .get(index)
        .map(|slide| {
            slide
                .layers
                .iter()
                .filter(|l| !l.hidden)
                .map(|l| (index, l.clone()))
                .collect()
        })
        .unwrap_or_default();
    for (owner, slide) in slides.iter().enumerate().filter(|(owner, _)| *owner != index) {
        let shift = (owner as f32 - index as f32) * BASE_WIDTH;
        for layer in slide.layers.iter().filter(|l| !l.hidden) {
            let x = layer.x + shift;
            if x < BASE_WIDTH && x + layer.width > 0. {
                let mut part = layer.clone();
                part.x = x;
                layers.push((owner, part));
            }
        }
    }
    layers
}

/// Renders slide `index` to PNG bytes at `scale` × the canvas size (1.0 for
/// export, smaller for thumbnails), including layers spanning in from its
/// neighbours. `images_dir` holds the design's images.
pub fn render_png(slides: &[Slide], index: usize, size: CanvasSize, images_dir: &Path, scale: f32) -> Result<Vec<u8>> {
    let options = usvg::Options {
        resources_dir: Some(images_dir.to_path_buf()),
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&to_svg(slides, index, size), &options)?;
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

#[cfg(test)]
mod tests;
