//! Slides as SVG, for rasterising with resvg.
//!
//! A frame is drawn as [`Piece`]s in order: SVG markup, and between them
//! the frames of screen recordings, which resvg can't decode and the
//! renderer composites itself. A still with no recordings is one piece.

use crate::text::vertical_metrics;
use crate::{
    Align, Background, CanvasSize, DEFAULT_FONT, LINE_HEIGHT, Layer, LayerKind, Rect, Slide, device_geometry,
    is_video, visible_layers,
};

/// After the layer's own font: the default, then what Windows and Linux have.
const FALLBACK_FAMILIES: &str = "'Segoe UI', Arial, sans-serif";

const SHADOW: &str = r##"<filter id="shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="24" stdDeviation="44" flood-color="#000000" flood-opacity="0.35"/></filter>"##;

/// One pass of drawing.
pub(crate) enum Piece {
    /// Markup in canvas units.
    Svg { defs: String, body: String },
    /// A screen recording's frame, cover-fitted into a rounded screen.
    Clip {
        file: String,
        time: f32,
        screen: Rect,
        radius: f32,
        opacity: f32,
    },
}

/// Which parts of a device to draw: all of it, what lies under its screen,
/// or what sits over the screen (island, notch, camera, home button).
#[derive(Clone, Copy, PartialEq)]
enum Part {
    All,
    Base,
    Overlay,
}

/// Collects pieces, starting a new SVG piece after each recording.
pub(crate) struct Pieces {
    done: Vec<Piece>,
    defs: String,
    body: String,
    /// Makes gradient and clip ids unique across scenes in one frame.
    next_id: usize,
}

impl Pieces {
    pub(crate) fn new() -> Pieces {
        Pieces {
            done: Vec::new(),
            defs: String::new(),
            body: String::new(),
            next_id: 0,
        }
    }

    fn id(&mut self) -> usize {
        self.next_id += 1;
        self.next_id
    }

    /// A slide's background, `offset` canvas pixels to the right.
    pub(crate) fn background(&mut self, background: &Background, offset: f32, canvas: (f32, f32), opacity: f32) {
        let fill = match background {
            Background::Solid { color } => color_attr(color),
            Background::Gradient { from, to, angle } => {
                let id = self.id();
                let (x1, y1, x2, y2) = gradient_vector(*angle);
                self.defs.push_str(&format!(
                    r#"<linearGradient id="bg{id}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient>"#,
                    color_attr(from),
                    color_attr(to)
                ));
                format!("url(#bg{id})")
            }
        };
        self.body.push_str(&format!(
            r#"<rect x="{offset}" width="{}" height="{}" fill="{fill}" opacity="{opacity}"/>"#,
            canvas.0, canvas.1
        ));
    }

    /// A layer, faded by `opacity` on top of its own. A device playing a
    /// recording is split around the recording's frame when `clips`;
    /// otherwise its screen is left dark.
    pub(crate) fn layer(&mut self, layer: &Layer, clip_time: f32, opacity: f32, clips: bool) {
        let opacity = (layer.opacity * opacity).clamp(0., 1.);
        let recording = match &layer.kind {
            LayerKind::Device {
                screenshot: Some(file),
                model,
                frame,
                ..
            } if clips && is_video(file) => Some((file.clone(), device_geometry(layer, *model, *frame))),
            _ => None,
        };
        match recording {
            Some((file, geometry)) => {
                self.markup(layer, Part::Base, opacity);
                self.flush();
                self.done.push(Piece::Clip {
                    file,
                    time: clip_time,
                    screen: geometry.screen,
                    radius: geometry.screen_radius,
                    opacity,
                });
                self.markup(layer, Part::Overlay, opacity);
            }
            None => self.markup(layer, Part::All, opacity),
        }
    }

    fn markup(&mut self, layer: &Layer, part: Part, opacity: f32) {
        let id = self.id();
        self.body.push_str(&format!(r#"<g opacity="{opacity}">"#));
        layer_svg(layer, id, part, &mut self.defs, &mut self.body);
        self.body.push_str("</g>");
    }

    fn flush(&mut self) {
        if !self.body.is_empty() {
            self.done.push(Piece::Svg {
                defs: std::mem::take(&mut self.defs),
                body: std::mem::take(&mut self.body),
            });
        }
    }

    pub(crate) fn finish(mut self) -> Vec<Piece> {
        self.flush();
        self.done
    }
}

/// A piece's markup as a whole SVG document at `size`, laid out on the
/// target's canvas and scaled to the export size.
pub(crate) fn document(defs: &str, body: &str, size: CanvasSize) -> String {
    let (w, h) = size.pixels();
    let k = w as f32 / size.target().canvas().0;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><defs>{SHADOW}{defs}</defs><g transform="scale({k})">{body}</g></svg>"#
    )
}

/// Slide `index` as one SVG for `size`, as designed, with layers spanning
/// in from its neighbours (screenshots only). Image `href`s are file names,
/// resolved against the design's folder when rendering; screen recordings
/// show as a dark screen.
pub fn to_svg(slides: &[Slide], index: usize, size: CanvasSize) -> String {
    let canvas = size.target().canvas();
    let mut pieces = Pieces::new();
    let background = slides.get(index).map_or(
        Background::Solid {
            color: "#000000".into(),
        },
        |slide| slide.background.clone(),
    );
    pieces.background(&background, 0., canvas, 1.);
    for (_, layer) in still_layers(slides, index, size) {
        pieces.layer(&layer, 0., 1., false);
    }
    let (defs, body) = (pieces.defs, pieces.body);
    document(&defs, &body, size)
}

/// What a still of slide `index` shows: its own visible layers, plus the
/// parts of neighbours' layers spanning in for screenshots.
pub(crate) fn still_layers(slides: &[Slide], index: usize, size: CanvasSize) -> Vec<(usize, Layer)> {
    if size.target().is_screenshot() {
        visible_layers(slides, index, size.target().canvas().0)
    } else {
        slides
            .get(index)
            .map(|slide| slide.layers.iter().filter(|l| !l.hidden).map(|l| (index, l.clone())).collect())
            .unwrap_or_default()
    }
}

fn layer_svg(layer: &Layer, index: usize, part: Part, defs: &mut String, body: &mut String) {
    let (x, y, width, height) = (layer.x, layer.y, layer.width, layer.height);
    match &layer.kind {
        LayerKind::Text {
            size,
            weight,
            color,
            align,
            font,
            italic,
            ..
        } => {
            let (anchor, tx) = match align {
                Align::Left => ("start", x),
                Align::Center => ("middle", x + width / 2.),
                Align::Right => ("end", x + width),
            };
            let family = font
                .as_deref()
                .map(|name| format!("'{}', ", escape(&name.replace(['\'', '"'], ""))))
                .unwrap_or_default();
            let style = if *italic { r#" font-style="italic""# } else { "" };
            // Baselines as gpui places them: the font's ascent and descent
            // centred in each line box.
            let (ascent, descent) = layer.text_style().map(vertical_metrics).unwrap_or((0.8, 0.2));
            let line = size * LINE_HEIGHT;
            for (i, text) in layer.lines().iter().enumerate() {
                let baseline = y + i as f32 * line + (line - (ascent + descent) * size) / 2. + ascent * size;
                body.push_str(&format!(
                    r#"<text x="{tx}" y="{baseline}" font-family="{family}'{DEFAULT_FONT}', {FALLBACK_FAMILIES}" font-size="{size}" font-weight="{weight}"{style} fill="{}" text-anchor="{anchor}">{}</text>"#,
                    color_attr(color),
                    escape(text)
                ));
            }
        }
        LayerKind::Device {
            model,
            frame,
            screenshot,
            shadow,
            ..
        } => {
            let g = device_geometry(layer, *model, *frame);
            let s = g.screen;
            let filter = if *shadow { r#" filter="url(#shadow)""# } else { "" };
            let rect = |r: Rect, radius: f32, fill: &str, extra: &str| {
                format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{radius}" fill="{fill}"{extra}/>"#,
                    r.x, r.y, r.width, r.height
                )
            };
            if part != Part::Overlay {
                match (frame.edge_color(), frame.button_color()) {
                    (Some(edge), Some(button)) => {
                        for b in &g.buttons {
                            body.push_str(&rect(*b, b.width.min(b.height) / 2., button, ""));
                        }
                        body.push_str(&rect(g.body, g.body_radius, edge, filter));
                        body.push_str(&rect(g.glass, g.glass_radius, frame.glass_color(*model), ""));
                    }
                    _ => body.push_str(&rect(s, g.screen_radius, "#000000", filter)),
                }
                match screenshot {
                    Some(file) if !is_video(file) => {
                        defs.push_str(&format!(
                            r#"<clipPath id="clip{index}"><rect x="{}" y="{}" width="{}" height="{}" rx="{}"/></clipPath>"#,
                            s.x, s.y, s.width, s.height, g.screen_radius
                        ));
                        body.push_str(&format!(
                            r#"<image x="{}" y="{}" width="{}" height="{}" preserveAspectRatio="xMidYMid slice" clip-path="url(#clip{index})" xlink:href="{}"/>"#,
                            s.x,
                            s.y,
                            s.width,
                            s.height,
                            escape(file)
                        ));
                    }
                    _ => body.push_str(&rect(s, g.screen_radius, "#1c1c1f", "")),
                }
            }
            if part == Part::Base {
                return;
            }
            if let Some(crease) = g.crease {
                body.push_str(&rect(crease, 0., "#000000", r#" fill-opacity="0.22""#));
            }
            if let Some(island) = g.island {
                body.push_str(&rect(island, island.height / 2., "#000000", ""));
            }
            if let Some(n) = g.notch {
                let (r, bottom, right) = (g.notch_radius, n.y + n.height, n.x + n.width);
                body.push_str(&format!(
                    r#"<path d="M{} {} H{right} V{} Q{right} {bottom} {} {bottom} H{} Q{} {bottom} {} {} Z" fill="{}"/>"#,
                    n.x,
                    n.y,
                    bottom - r,
                    right - r,
                    n.x + r,
                    n.x,
                    n.x,
                    bottom - r,
                    frame.glass_color(*model)
                ));
            }
            if let Some(speaker) = g.speaker {
                body.push_str(&rect(speaker, speaker.height / 2., "#2b2b2e", ""));
            }
            if let Some(camera) = g.camera {
                body.push_str(&rect(camera, camera.width / 2., "#16161a", ""));
            }
            if let (Some(button), Some(ring)) = (g.home_button, frame.button_color()) {
                body.push_str(&rect(
                    button,
                    button.width / 2.,
                    frame.glass_color(*model),
                    &format!(r#" stroke="{ring}" stroke-width="{}""#, button.width * 0.05),
                ));
            }
        }
        LayerKind::Image { file, radius, shadow } => {
            let filter = if *shadow { r#" filter="url(#shadow)""# } else { "" };
            body.push_str(&format!(
                r##"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}" fill="#000000" fill-opacity="0.001"{filter}/>"##
            ));
            if let Some(file) = file {
                defs.push_str(&format!(
                    r#"<clipPath id="clip{index}"><rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}"/></clipPath>"#
                ));
                body.push_str(&format!(
                    r#"<image x="{x}" y="{y}" width="{width}" height="{height}" preserveAspectRatio="xMidYMid slice" clip-path="url(#clip{index})" xlink:href="{}"/>"#,
                    escape(file)
                ));
            }
        }
        LayerKind::Shape { fill, radius } => body.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}" fill="{}"/>"#,
            color_attr(fill)
        )),
    }
}

/// The gradient vector in objectBoundingBox units for a CSS angle.
pub(crate) fn gradient_vector(angle: f32) -> (f32, f32, f32, f32) {
    let radians = angle.to_radians();
    let (dx, dy) = (radians.sin() / 2., -radians.cos() / 2.);
    (0.5 - dx, 0.5 - dy, 0.5 + dx, 0.5 + dy)
}

/// `#rrggbb` or `#rgb`, else black, so user input can't break the SVG.
fn color_attr(color: &str) -> String {
    match crate::parse_color(color) {
        Some((r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
        None => "#000000".into(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
