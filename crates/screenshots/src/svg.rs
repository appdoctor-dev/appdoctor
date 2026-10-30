//! A slide as SVG, for rasterising with resvg.

use crate::text::vertical_metrics;
use crate::{Align, Background, CanvasSize, DEFAULT_FONT, LINE_HEIGHT, Layer, LayerKind, Slide, device_geometry, visible_layers};

/// After the layer's own font: the default, then what Windows and Linux have.
const FALLBACK_FAMILIES: &str = "'Segoe UI', Arial, sans-serif";

/// Slide `index` as SVG for `size`, with layers spanning in from its
/// neighbours. Image `href`s are file names, resolved against the design's
/// folder when rendering.
pub fn to_svg(slides: &[Slide], index: usize, size: CanvasSize) -> String {
    let background = slides
        .get(index)
        .map(|slide| slide.background.clone())
        .unwrap_or(Background::Solid {
            color: "#000000".into(),
        });
    let (w, h) = size.pixels();
    let (canvas_width, _) = size.target().canvas();
    let k = w as f32 / canvas_width;
    let mut defs = String::new();
    let mut body = String::new();

    let fill = match &background {
        Background::Solid { color } => color_attr(color),
        Background::Gradient { from, to, angle } => {
            let (x1, y1, x2, y2) = gradient_vector(*angle);
            defs.push_str(&format!(
                r#"<linearGradient id="bg" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}"><stop offset="0" stop-color="{}"/><stop offset="1" stop-color="{}"/></linearGradient>"#,
                color_attr(from),
                color_attr(to)
            ));
            "url(#bg)".into()
        }
    };
    defs.push_str(r##"<filter id="shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="24" stdDeviation="44" flood-color="#000000" flood-opacity="0.35"/></filter>"##);
    body.push_str(&format!(
        r#"<rect width="{canvas_width}" height="{}" fill="{fill}"/>"#,
        h as f32 / k
    ));

    for (ix, (_, layer)) in visible_layers(slides, index, canvas_width).iter().enumerate() {
        body.push_str(&format!(r#"<g opacity="{}">"#, layer.opacity.clamp(0., 1.)));
        layer_svg(layer, ix, &mut defs, &mut body);
        body.push_str("</g>");
    }

    // Everything is laid out on the base canvas and scaled to the export size.
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><defs>{defs}</defs><g transform="scale({k})">{body}</g></svg>"#
    )
}

fn layer_svg(layer: &Layer, index: usize, defs: &mut String, body: &mut String) {
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
        } => {
            let g = device_geometry(layer, *model, *frame);
            let s = g.screen;
            let filter = if *shadow { r#" filter="url(#shadow)""# } else { "" };
            let rect = |r: crate::Rect, radius: f32, fill: &str, extra: &str| {
                format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{radius}" fill="{fill}"{extra}/>"#,
                    r.x, r.y, r.width, r.height
                )
            };
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
                Some(file) => {
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
                None => body.push_str(&rect(s, g.screen_radius, "#1c1c1f", "")),
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
