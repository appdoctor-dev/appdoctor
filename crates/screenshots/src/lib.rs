//! App Store screenshot designs.
//!
//! A [`Design`] is a list of [`Slide`]s for one [`Target`] (iPhone or
//! iPad); each slide is a background and a stack of positioned [`Layer`]s
//! (text, device, image, shape) in pixels of the target's canvas.
//!
//! Slides sit edge to edge, so a layer hanging off one slide continues on
//! its neighbour ([`visible_layers`]): panoramic sets need no copies.
//!
//! The layout rules the editor draws with live are here too: [`device_geometry`]
//! for device frames and [`Layer::lines`] for text, measured with the real
//! font. [`to_svg`] and
//! [`render_png`] use the same functions for export, so the canvas and the
//! exported PNGs agree.

mod device;
pub mod edit;
mod legacy;
pub mod motion;
mod render;
mod svg;
mod text;
pub mod themes;

use std::path::Path;

pub use resvg::tiny_skia;
use resvg::usvg;
use serde::{Deserialize, Serialize};

pub use device::{DeviceGeometry, DeviceModel, Frame, device_geometry};
pub use motion::{Drift, Effect, Focus, Motion, Transition, Video};
pub use render::{ClipSource, NoClips, render_frame, render_png, render_png_with, render_slide};
pub use svg::to_svg;
pub use themes::{THEMES, Theme, theme};
pub use text::{
    DEFAULT_FONT, SUGGESTED_FONTS, Style, Word, font_families, has_font, strip_marks, warm_fonts, wrap, wrap_styled,
};

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

/// The iPhone canvas. iPad designs use [`Target::canvas`].
pub const BASE_WIDTH: f32 = 1320.;
pub const BASE_HEIGHT: f32 = 2868.;

/// What a design is for: iPhone or iPad screenshots, or the product page
/// header and search result art Apple added in iOS 27. Each has its own
/// canvas, export sizes and design file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    Iphone,
    Ipad,
    /// The 21:9 art at the top of the product page.
    Header,
    /// The 3:2 art in search results.
    Search,
}

impl Target {
    pub const ALL: [Target; 4] = [Target::Iphone, Target::Ipad, Target::Header, Target::Search];

    /// Width and height of the canvas layers are positioned on: Apple's
    /// largest required size for each.
    pub fn canvas(self) -> (f32, f32) {
        match self {
            Target::Iphone => (BASE_WIDTH, BASE_HEIGHT),
            Target::Ipad => (2064., 2752.),
            Target::Header => (3840., 1646.),
            Target::Search => (3840., 2560.),
        }
    }

    /// The sizes an export writes.
    pub fn sizes(self) -> &'static [CanvasSize] {
        match self {
            Target::Iphone => &[CanvasSize::Iphone69, CanvasSize::Iphone65],
            Target::Ipad => &[CanvasSize::Ipad13, CanvasSize::Ipad129],
            Target::Header => &[CanvasSize::Header],
            Target::Search => &[CanvasSize::Search, CanvasSize::SearchSmall],
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Target::Iphone => "iPhone",
            Target::Ipad => "iPad",
            Target::Header => "Product page header",
            Target::Search => "Search result",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Target::Iphone => "iphone",
            Target::Ipad => "ipad",
            Target::Header => "header",
            Target::Search => "search",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|target| target.slug() == slug)
    }

    /// The design's file inside the app's screenshots folder.
    pub fn file_name(self) -> &'static str {
        match self {
            Target::Iphone => "design.json",
            Target::Ipad => "design-ipad.json",
            Target::Header => "design-header.json",
            Target::Search => "design-search.json",
        }
    }

    /// The device new layers get.
    pub fn default_model(self) -> DeviceModel {
        let slug = match self {
            Target::Ipad => "ipad-pro-13",
            Target::Iphone | Target::Header | Target::Search => "iphone-18-pro-max",
        };
        DeviceModel::from_slug(slug).expect("the default models are in the catalog")
    }

    /// The size its video exports at: Apple's app preview sizes for iPhone
    /// and iPad, the header's own, and the smallest search size.
    pub fn video_size(self) -> CanvasSize {
        match self {
            Target::Iphone => CanvasSize::PreviewIphone,
            Target::Ipad => CanvasSize::PreviewIpad,
            Target::Header => CanvasSize::Header,
            Target::Search => CanvasSize::SearchSmall,
        }
    }

    /// Screenshots sit side by side in the App Store; header and search art
    /// stand alone, so each of their slides is an alternative.
    pub fn is_screenshot(self) -> bool {
        matches!(self, Target::Iphone | Target::Ipad)
    }

    /// The share of the width and height to keep clear on each side, where
    /// the App Store crops the art on some devices. Apple marks it in its
    /// templates without numbers; these come from measuring the preview.
    pub fn safe_inset(self) -> Option<(f32, f32)> {
        match self {
            Target::Header => Some((0.18, 0.15)),
            _ => None,
        }
    }
}

/// Background presets: name, gradient from and to, and the text colour
/// that reads on it.
pub const BACKGROUND_PRESETS: &[(&str, &str, &str, &str)] = &[
    ("Indigo", "#4f46e5", "#9333ea", "#ffffff"),
    ("Sunset", "#f97316", "#db2777", "#ffffff"),
    ("Ocean", "#06b6d4", "#2563eb", "#ffffff"),
    ("Mint", "#34d399", "#059669", "#ffffff"),
    ("Midnight", "#0f172a", "#334155", "#ffffff"),
    ("Paper", "#f8fafc", "#e2e8f0", "#0f172a"),
];

/// App Store screenshot sizes (portrait). Layers scale uniformly by width.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CanvasSize {
    /// 6.9" iPhone, 1320 × 2868: Apple's required size for new submissions.
    Iphone69,
    /// 6.5" iPhone, 1242 × 2688.
    Iphone65,
    /// 13" iPad, 2064 × 2752: Apple's required iPad size.
    Ipad13,
    /// 12.9" iPad Pro (2nd generation), 2048 × 2732.
    Ipad129,
    /// Product page header, 21:9.
    Header,
    /// Search result art at 3:2, largest (3840 × 2560) and smallest
    /// (1920 × 1280) Apple takes.
    Search,
    SearchSmall,
    /// App preview video, iPhone (every Face ID size takes 886 × 1920).
    PreviewIphone,
    /// App preview video, iPad, 1200 × 1600.
    PreviewIpad,
}

impl CanvasSize {
    pub const ALL: [CanvasSize; 9] = [
        CanvasSize::Iphone69,
        CanvasSize::Iphone65,
        CanvasSize::Ipad13,
        CanvasSize::Ipad129,
        CanvasSize::Header,
        CanvasSize::Search,
        CanvasSize::SearchSmall,
        CanvasSize::PreviewIphone,
        CanvasSize::PreviewIpad,
    ];

    pub fn pixels(self) -> (u32, u32) {
        match self {
            CanvasSize::Iphone69 => (1320, 2868),
            CanvasSize::Iphone65 => (1242, 2688),
            CanvasSize::Ipad13 => (2064, 2752),
            CanvasSize::Ipad129 => (2048, 2732),
            CanvasSize::Header => (3840, 1646),
            CanvasSize::Search => (3840, 2560),
            CanvasSize::SearchSmall => (1920, 1280),
            CanvasSize::PreviewIphone => (886, 1920),
            CanvasSize::PreviewIpad => (1200, 1600),
        }
    }

    pub fn target(self) -> Target {
        match self {
            CanvasSize::Iphone69 | CanvasSize::Iphone65 | CanvasSize::PreviewIphone => Target::Iphone,
            CanvasSize::Ipad13 | CanvasSize::Ipad129 | CanvasSize::PreviewIpad => Target::Ipad,
            CanvasSize::Header => Target::Header,
            CanvasSize::Search | CanvasSize::SearchSmall => Target::Search,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            CanvasSize::Iphone69 => "iPhone 6.9\"",
            CanvasSize::Iphone65 => "iPhone 6.5\"",
            CanvasSize::Ipad13 => "iPad 13\"",
            CanvasSize::Ipad129 => "iPad 12.9\"",
            CanvasSize::Header => "Header 21:9",
            CanvasSize::Search => "Search 3:2",
            CanvasSize::SearchSmall => "Search 3:2, small",
            CanvasSize::PreviewIphone => "iPhone app preview",
            CanvasSize::PreviewIpad => "iPad app preview",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            CanvasSize::Iphone69 => "iphone-6.9",
            CanvasSize::Iphone65 => "iphone-6.5",
            CanvasSize::Ipad13 => "ipad-13",
            CanvasSize::Ipad129 => "ipad-12.9",
            CanvasSize::Header => "header-21x9",
            CanvasSize::Search => "search-3x2",
            CanvasSize::SearchSmall => "search-3x2-small",
            CanvasSize::PreviewIphone => "app-preview-iphone",
            CanvasSize::PreviewIpad => "app-preview-ipad",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Design {
    pub version: u32,
    pub target: Target,
    /// Set when the design is also a video: its slides play as scenes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<Video>,
    pub slides: Vec<Slide>,
}

impl Default for Design {
    fn default() -> Self {
        Self {
            version: 2,
            target: Target::Iphone,
            video: None,
            slides: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slide {
    pub background: Background,
    /// Bottom to top.
    pub layers: Vec<Layer>,
    /// Seconds the slide shows as a scene, when the design is a video.
    #[serde(default = "default_scene", skip_serializing_if = "is_default_scene")]
    pub duration: f32,
    /// Degrees a gradient background turns each second of the scene.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spin: f32,
    /// Camera moves over the scene: zooms in on a point and back out.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub camera: Vec<Focus>,
}

fn default_scene() -> f32 {
    motion::DEFAULT_SCENE
}

fn is_default_scene(duration: &f32) -> bool {
    *duration == motion::DEFAULT_SCENE
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
    /// When it enters, leaves and drifts, in a video.
    #[serde(default, skip_serializing_if = "Motion::is_still")]
    pub motion: Motion,
    pub kind: LayerKind,
}

fn one() -> f32 {
    1.
}

fn is_zero(value: &f32) -> bool {
    *value == 0.
}

fn is_one(value: &f32) -> bool {
    *value == 1.
}

/// Whether a file is a video (a screen recording) rather than an image.
pub fn is_video(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".mov", ".mp4", ".m4v"].iter().any(|ext| lower.ends_with(ext))
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
        /// A font family; [`DEFAULT_FONT`] when unset.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        italic: bool,
        /// The colour of words marked `*like this*` in the text.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        accent: Option<String>,
    },
    Device {
        #[serde(default)]
        model: DeviceModel,
        frame: Frame,
        /// File name inside the design's folder: an image, or a screen
        /// recording (.mov, .mp4, .m4v) that plays in videos.
        screenshot: Option<String>,
        shadow: bool,
        /// Seconds into the screen recording that it starts playing.
        #[serde(default, skip_serializing_if = "is_zero")]
        clip_start: f32,
        /// How fast the recording plays: 2 is twice as fast.
        #[serde(default = "one", skip_serializing_if = "is_one")]
        clip_speed: f32,
    },
    Image {
        file: Option<String>,
        radius: f32,
        shadow: bool,
    },
    Shape {
        fill: String,
        radius: f32,
        /// How much of the fill shows: 0 for an outline only.
        #[serde(default = "one", skip_serializing_if = "is_one")]
        fill_opacity: f32,
        /// An outline in this colour, `stroke_width` canvas pixels wide.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke: Option<String>,
        #[serde(default, skip_serializing_if = "is_zero")]
        stroke_width: f32,
        /// Blur in canvas pixels: a soft glow rather than a hard shape.
        #[serde(default, skip_serializing_if = "is_zero")]
        soft: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

pub const LINE_HEIGHT: f32 = 1.16;

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

    /// How a text layer is set, for measuring; `None` for other layers.
    pub fn text_style(&self) -> Option<Style<'_>> {
        match &self.kind {
            LayerKind::Text {
                font, weight, italic, ..
            } => Some(Style {
                family: font.as_deref(),
                weight: *weight,
                italic: *italic,
            }),
            _ => None,
        }
    }

    /// A text layer's lines, wrapped to its width with its font. The canvas
    /// and the export both draw these.
    pub fn lines(&self) -> Vec<String> {
        match (&self.kind, self.text_style()) {
            (LayerKind::Text { text, size, .. }, Some(style)) => {
                wrap_styled(&strip_marks(text), style, *size, self.width)
            }
            _ => Vec::new(),
        }
    }

    /// [`Layer::lines`] as words, with which are accented and their order.
    pub fn words(&self) -> Vec<Vec<Word>> {
        match &self.kind {
            LayerKind::Text { text, .. } => text::words_by_line(text, &self.lines()),
            _ => Vec::new(),
        }
    }

    pub fn text(target: Target, id: u64, text: &str, y: f32, size: f32, weight: u16, color: &str) -> Layer {
        let (canvas_width, _) = target.canvas();
        let margin = canvas_width * 0.07;
        let mut layer = Layer {
            id,
            name: text.chars().take(24).collect(),
            x: margin,
            y,
            width: canvas_width - margin * 2.,
            height: 0.,
            opacity: 1.,
            hidden: false,
            locked: false,
            motion: Motion::default(),
            kind: LayerKind::Text {
                text: text.into(),
                size,
                weight,
                color: color.into(),
                align: Align::Center,
                font: None,
                italic: false,
                accent: None,
            },
        };
        layer.fit_text();
        layer
    }

    /// The target's usual device, centred at `y`.
    pub fn device(target: Target, id: u64, y: f32) -> Layer {
        let model = target.default_model();
        let (canvas_width, _) = target.canvas();
        let width = canvas_width
            * match target {
                Target::Iphone => 0.78,
                Target::Ipad => 0.8,
                Target::Header => 0.14,
                Target::Search => 0.22,
            };
        Layer {
            id,
            name: model.target().label().into(),
            x: (canvas_width - width) / 2.,
            y,
            width,
            height: width * model.aspect(),
            opacity: 1.,
            hidden: false,
            locked: false,
            motion: Motion::default(),
            kind: LayerKind::Device {
                model,
                frame: Frame::Black,
                screenshot: None,
                shadow: true,
                clip_start: 0.,
                clip_speed: 1.,
            },
        }
    }

    pub fn image(target: Target, id: u64, file: String, width: f32, height: f32) -> Layer {
        let (canvas_width, canvas_height) = target.canvas();
        Layer {
            id,
            name: "Image".into(),
            x: (canvas_width - width) / 2.,
            y: (canvas_height - height) / 2.,
            width,
            height,
            opacity: 1.,
            hidden: false,
            locked: false,
            motion: Motion::default(),
            kind: LayerKind::Image {
                file: Some(file),
                radius: 36.,
                shadow: true,
            },
        }
    }

    pub fn shape(target: Target, id: u64) -> Layer {
        let (canvas_width, canvas_height) = target.canvas();
        let (width, height) = (canvas_width * 0.6, canvas_width * 0.3);
        Layer {
            id,
            name: "Shape".into(),
            x: (canvas_width - width) / 2.,
            y: canvas_height * 0.42,
            width,
            height,
            opacity: 1.,
            hidden: false,
            locked: false,
            motion: Motion::default(),
            kind: LayerKind::Shape {
                fill: "#ffffff".into(),
                radius: 48.,
                fill_opacity: 1.,
                stroke: None,
                stroke_width: 0.,
                soft: 0.,
            },
        }
    }
}

impl Slide {
    /// A fresh slide: gradient, headline, subheadline and a device.
    pub fn starter(target: Target, headline: &str, next_id: &mut u64) -> Slide {
        let mut id = || {
            *next_id += 1;
            *next_id
        };
        let gradient = Background::Gradient {
            from: "#4f46e5".into(),
            to: "#9333ea".into(),
            angle: 160.,
        };
        let (width, height) = target.canvas();
        match target {
            Target::Iphone | Target::Ipad => {
                // Caption sizes and the device's top, per canvas.
                let (headline_y, headline_size, sub_y, sub_size, device_y) = match target {
                    Target::Iphone => (200., 104., 470., 54., 640.),
                    _ => (170., 112., 330., 58., 520.),
                };
                Slide {
                    background: gradient,
                    layers: vec![
                        Layer::text(target, id(), headline, headline_y, headline_size, 700, "#ffffff"),
                        Layer::text(target, id(), "One line on why it matters", sub_y, sub_size, 400, "#e0e7ff"),
                        Layer::device(target, id(), device_y),
                    ],
                    duration: motion::DEFAULT_SCENE,
                    spin: 0.,
                    camera: Vec::new(),
                }
            }
            // Wide: the words on the left, a phone on the right, all inside
            // the safe area.
            Target::Header => {
                let mut title = Layer::text(target, id(), headline, height * 0.34, 150., 800, "#ffffff");
                let mut sub = Layer::text(target, id(), "One line on why it matters", 0., 72., 400, "#e0e7ff");
                for layer in [&mut title, &mut sub] {
                    layer.x = width * 0.19;
                    layer.width = width * 0.38;
                    if let LayerKind::Text { align, .. } = &mut layer.kind {
                        *align = Align::Left;
                    }
                    layer.fit_text();
                }
                sub.y = title.y + title.height + 36.;
                let mut device = Layer::device(target, id(), height * 0.16);
                device.x = width * 0.64;
                Slide {
                    background: gradient,
                    layers: vec![title, sub, device],
                    duration: motion::DEFAULT_SCENE,
                    spin: 0.,
                    camera: Vec::new(),
                }
            }
            Target::Search => {
                let title = Layer::text(target, id(), headline, height * 0.07, 170., 800, "#ffffff");
                let device = Layer::device(target, id(), title.y + title.height + 90.);
                Slide {
                    background: gradient,
                    layers: vec![title, device],
                    duration: motion::DEFAULT_SCENE,
                    spin: 0.,
                    camera: Vec::new(),
                }
            }
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

    /// Parses a saved design, upgrading older formats.
    pub fn from_json(bytes: &[u8]) -> Option<Design> {
        legacy::parse(bytes)
    }

    /// Loads the design for `target` from an app's screenshots folder.
    pub fn load_for(folder: &Path, target: Target) -> Design {
        let mut design = Design::load(&folder.join(target.file_name()));
        design.target = target;
        design
    }

    /// The saved form, as [`Design::save`] writes it.
    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        write_atomic(path, &self.to_json())
    }
}

/// Writes then renames, so a crash (or a reader in another process, such
/// as the editor watching for an agent's edits) never sees half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(temp, path)
}

/// A rectangle in canvas pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Copies an image into a design's folder under a fresh name; returns the
/// name and the image's pixel size.
pub fn import_image(source: &Path, folder: &Path) -> std::io::Result<(String, (f32, f32))> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp"))
        .ok_or_else(|| std::io::Error::other("choose a PNG, JPEG or WebP image"))?;
    let size = imagesize::size(source).map_err(std::io::Error::other)?;
    std::fs::create_dir_all(folder)?;
    let name = format!("img-{}.{extension}", uuid::Uuid::new_v4().simple());
    std::fs::copy(source, folder.join(&name))?;
    Ok((name, (size.width as f32, size.height as f32)))
}

/// Copies an image or a screen recording into a design's folder under a
/// fresh name; returns the name.
pub fn import_media(source: &Path, folder: &Path) -> std::io::Result<String> {
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !is_video(&name) {
        return import_image(source, folder).map(|(name, _)| name);
    }
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| "mov".into());
    std::fs::create_dir_all(folder)?;
    let name = format!("clip-{}.{extension}", uuid::Uuid::new_v4().simple());
    std::fs::copy(source, folder.join(&name))?;
    Ok(name)
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
/// Each comes with the index of the slide that owns it. Slides are
/// `width` wide.
pub fn visible_layers(slides: &[Slide], index: usize, width: f32) -> Vec<(usize, Layer)> {
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
        let shift = (owner as f32 - index as f32) * width;
        for layer in slide.layers.iter().filter(|l| !l.hidden) {
            let x = layer.x + shift;
            if x < width && x + layer.width > 0. {
                let mut part = layer.clone();
                part.x = x;
                layers.push((owner, part));
            }
        }
    }
    layers
}

#[cfg(test)]
mod tests;
