//! A slide as SVG, for rasterising with resvg.

use crate::{
    Align, BASE_WIDTH, Background, CanvasSize, LINE_HEIGHT, Layer, LayerKind, SVG_FONT_FAMILY, Slide, device_geometry,
    visible_layers, wrap,
};

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
    let k = w as f32 / BASE_WIDTH;
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
        r#"<rect width="{BASE_WIDTH}" height="{}" fill="{fill}"/>"#,
        h as f32 / k
    ));

    for (ix, (_, layer)) in visible_layers(slides, index).iter().enumerate() {
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
            text,
            size,
            weight,
            color,
            align,
        } => {
            let (anchor, tx) = match align {
                Align::Left => ("start", x),
                Align::Center => ("middle", x + width / 2.),
                Align::Right => ("end", x + width),
            };
            for (i, line) in wrap(text, *size, width).iter().enumerate() {
                body.push_str(&format!(
                    r#"<text x="{tx}" y="{}" font-family="{SVG_FONT_FAMILY}" font-size="{size}" font-weight="{weight}" fill="{}" text-anchor="{anchor}">{}</text>"#,
                    // Baseline: one ascent (about 0.8 em) into each line box.
                    y + (i as f32) * size * LINE_HEIGHT + size * (LINE_HEIGHT - 1.) / 2. + size * 0.8,
                    color_attr(color),
                    escape(line)
                ));
            }
        }
        LayerKind::Device {
            frame,
            screenshot,
            shadow,
        } => {
            let g = device_geometry(layer, *frame);
            let s = g.screen;
            let filter = if *shadow { r#" filter="url(#shadow)""# } else { "" };
            match frame.bezel_color() {
                Some(bezel) => body.push_str(&format!(
                    r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{bezel}"{filter}/>"#,
                    g.body.x, g.body.y, g.body.width, g.body.height, g.body_radius
                )),
                None => body.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="#000000"{filter}/>"##,
                    s.x, s.y, s.width, s.height, g.screen_radius
                )),
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
                None => body.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="#1c1c1f"/>"##,
                    s.x, s.y, s.width, s.height, g.screen_radius
                )),
            }
            if let Some(island) = g.island {
                body.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="#000000"/>"##,
                    island.x,
                    island.y,
                    island.width,
                    island.height,
                    island.height / 2.
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
