//! Importing an SVG drawing as separate layers, so a picture an AI drew
//! (or anyone did) can be edited piece by piece: each top-level element
//! becomes its own layer. Plain text becomes a text layer, plain rectangles
//! and circles become shapes, images become image layers, and everything
//! else (paths, groups, gradients) becomes a vector layer drawn from its
//! own SVG, sharp at any size.

use std::collections::HashMap;
use std::path::Path;

use base64::Engine as _;
use resvg::usvg::{self, roxmltree};

use crate::{Align, Layer, LayerKind, Motion, Rect, Target, text};

/// Elements that only support others.
const SUPPORT: &[&str] = &["defs", "style", "title", "desc", "metadata", "namedview", "script"];

/// Where to put the drawing on the canvas: its left, top and width in
/// canvas pixels; centred at 80% of the width when unset.
#[derive(Debug, Clone, Copy, Default)]
pub struct Placement {
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub width: Option<f32>,
}

/// Turns an SVG into layers, bottom to top, with ids from `next_id`.
/// Embedded images are written into `folder`.
pub fn import_svg(svg: &str, target: Target, placement: Placement, folder: &Path, next_id: &mut u64) -> Result<Vec<Layer>, String> {
    let document = roxmltree::Document::parse(svg).map_err(|error| format!("That isn't valid SVG: {error}"))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err("That isn't an SVG drawing.".into());
    }
    let view = view_box(&root).ok_or("The SVG has no size: give it a viewBox or width and height.")?;

    // Shared definitions (gradients, styles) go into each vector layer.
    let support: String = root
        .children()
        .filter(|node| node.is_element() && matches!(node.tag_name().name(), "defs" | "style"))
        .map(|node| svg[node.range()].to_owned())
        .collect();

    let (canvas_width, canvas_height) = target.canvas();
    let width = placement.width.unwrap_or(canvas_width * 0.8);
    let scale = width / view.width;
    let left = placement.x.unwrap_or((canvas_width - width) / 2.);
    let top = placement.y.unwrap_or((canvas_height - view.height * scale) / 2.);
    let place = |r: Rect| Rect {
        x: left + (r.x - view.x) * scale,
        y: top + (r.y - view.y) * scale,
        width: r.width * scale,
        height: r.height * scale,
    };

    let mut layers = Vec::new();
    let mut vectors = 0;
    for node in root.children().filter(|n| n.is_element()) {
        if SUPPORT.contains(&node.tag_name().name()) {
            continue;
        }
        *next_id += 1;
        let id = *next_id;
        let simple = simple_layer(&node, id, target, &place, scale, folder);
        let layer = match simple {
            Some(layer) => layer,
            None => {
                let markup = &svg[node.range()];
                let Some(bounds) = bounds(markup, &support, view) else {
                    *next_id -= 1;
                    continue;
                };
                vectors += 1;
                let rect = place(bounds);
                let mut layer = blank(id, name(&node).unwrap_or_else(|| format!("Drawing {vectors}")), rect);
                layer.kind = LayerKind::Vector {
                    svg: format!(
                        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{} {} {} {}" preserveAspectRatio="none">{support}{markup}</svg>"#,
                        bounds.x, bounds.y, bounds.width, bounds.height
                    ),
                };
                layer
            }
        };
        layers.push(layer);
    }
    if layers.is_empty() {
        return Err("The SVG has nothing to draw.".into());
    }
    Ok(layers)
}

fn blank(id: u64, name: String, rect: Rect) -> Layer {
    Layer {
        id,
        name,
        x: rect.x,
        y: rect.y,
        width: rect.width.max(1.),
        height: rect.height.max(1.),
        opacity: 1.,
        hidden: false,
        locked: false,
        motion: Motion::default(),
        kind: LayerKind::Shape {
            fill: "#ffffff".into(),
            radius: 0.,
            fill_opacity: 1.,
            stroke: None,
            stroke_width: 0.,
            soft: 0.,
        },
    }
}

fn view_box(root: &roxmltree::Node) -> Option<Rect> {
    if let Some(view) = root.attribute("viewBox") {
        let n: Vec<f32> = view
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if n.len() == 4 && n[2] > 0. && n[3] > 0. {
            return Some(Rect {
                x: n[0],
                y: n[1],
                width: n[2],
                height: n[3],
            });
        }
    }
    let width = number(root.attribute("width")?)?;
    let height = number(root.attribute("height")?)?;
    (width > 0. && height > 0.).then_some(Rect {
        x: 0.,
        y: 0.,
        width,
        height,
    })
}

/// A length like `24`, `24px` or `24.5`.
fn number(value: &str) -> Option<f32> {
    value.trim().trim_end_matches("px").trim().parse().ok()
}

/// A presentation attribute, from the attribute or the inline style.
fn property<'a>(node: &'a roxmltree::Node, name: &str) -> Option<&'a str> {
    node.attribute(name).or_else(|| {
        node.attribute("style")?.split(';').find_map(|declaration| {
            let (key, value) = declaration.split_once(':')?;
            (key.trim() == name).then_some(value.trim())
        })
    })
}

/// A plain colour (not a gradient or pattern) as `#rrggbb`.
fn color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with('#') {
        return crate::parse_color(value).map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"));
    }
    let named: HashMap<&str, &str> = [
        ("white", "#ffffff"),
        ("black", "#000000"),
        ("red", "#ff0000"),
        ("green", "#008000"),
        ("blue", "#0000ff"),
        ("gray", "#808080"),
        ("grey", "#808080"),
        ("orange", "#ffa500"),
        ("yellow", "#ffff00"),
        ("purple", "#800080"),
        ("pink", "#ffc0cb"),
    ]
    .into_iter()
    .collect();
    named.get(value).map(|hex| (*hex).to_owned())
}

/// A name for the layer from the drawing's own labels.
fn name(node: &roxmltree::Node) -> Option<String> {
    node.attributes()
        .find(|a| matches!(a.name(), "label" | "data-name" | "aria-label" | "id"))
        .map(|a| a.value().replace(['-', '_'], " "))
        .filter(|n| !n.trim().is_empty())
}

/// A text, rectangle, circle or image simple enough to become an editable
/// layer; `None` hands it to a vector layer.
fn simple_layer(
    node: &roxmltree::Node,
    id: u64,
    target: Target,
    place: &dyn Fn(Rect) -> Rect,
    scale: f32,
    folder: &Path,
) -> Option<Layer> {
    // Transforms and classes (styled by CSS) are left to the renderer.
    if node.attribute("transform").is_some() || node.attribute("class").is_some() || node.attribute("filter").is_some() {
        return None;
    }
    let attr = |name: &str| node.attribute(name).and_then(number);
    let opacity = property(node, "opacity").and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.);
    let mut layer = match node.tag_name().name() {
        "text" => {
            // Positioned spans would move on their own: keep those whole.
            if node.descendants().any(|d| d.tag_name().name() == "tspan" && (d.has_attribute("x") || d.has_attribute("dy"))) {
                return None;
            }
            let words: String = node
                .descendants()
                .filter(|d| d.is_text())
                .filter_map(|d| d.text())
                .collect::<Vec<_>>()
                .join("")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if words.is_empty() {
                return None;
            }
            let fill = color(property(node, "fill").unwrap_or("#000000"))?;
            let size = property(node, "font-size").and_then(number).unwrap_or(16.);
            let weight = match property(node, "font-weight").unwrap_or("400") {
                "bold" => 700,
                "normal" => 400,
                other => other.parse::<u16>().unwrap_or(400),
            };
            let family = property(node, "font-family").and_then(|f| {
                f.split(',')
                    .map(|n| n.trim().trim_matches(['\'', '"']).to_owned())
                    .find(|n| !n.is_empty() && !matches!(n.as_str(), "sans-serif" | "serif" | "system-ui" | "-apple-system"))
            });
            let anchor = property(node, "text-anchor").unwrap_or("start");
            let style = text::Style {
                family: family.as_deref(),
                weight,
                italic: property(node, "font-style") == Some("italic"),
            };
            // A little room, so the measured line doesn't wrap.
            let width = text::measure(&words, style) * size * 1.06 + size * 0.2;
            let (x, y) = (attr("x").unwrap_or(0.), attr("y").unwrap_or(0.));
            let left = match anchor {
                "middle" => x - width / 2.,
                "end" => x - width,
                _ => x,
            };
            let (ascent, _) = text::vertical_metrics(style);
            let top = y - size * ascent - size * (crate::LINE_HEIGHT - 1.) / 2.;
            let rect = place(Rect {
                x: left,
                y: top,
                width,
                height: size * crate::LINE_HEIGHT,
            });
            let mut layer = Layer::text(target, id, &words, rect.y, size * scale, weight, &fill);
            layer.x = rect.x;
            layer.width = rect.width;
            if let LayerKind::Text { align, font, .. } = &mut layer.kind {
                *align = match anchor {
                    "middle" => Align::Center,
                    "end" => Align::Right,
                    _ => Align::Left,
                };
                *font = family.filter(|f| text::has_font(f));
            }
            layer.fit_text();
            layer
        }
        "rect" | "circle" | "ellipse" => {
            let fill_value = property(node, "fill").unwrap_or("#000000");
            let (fill, fill_opacity): (String, f32) = if fill_value == "none" {
                ("#000000".to_owned(), 0.)
            } else {
                (
                    color(fill_value)?,
                    property(node, "fill-opacity").and_then(|v| v.parse().ok()).unwrap_or(1.),
                )
            };
            let stroke = match property(node, "stroke") {
                None | Some("none") => None,
                Some(value) => Some(color(value)?),
            };
            let stroke_width = property(node, "stroke-width").and_then(number).unwrap_or(1.);
            let (bounds, radius) = match node.tag_name().name() {
                "rect" => {
                    let (w, h) = (attr("width")?, attr("height")?);
                    let rx = attr("rx").or(attr("ry")).unwrap_or(0.);
                    (
                        Rect {
                            x: attr("x").unwrap_or(0.),
                            y: attr("y").unwrap_or(0.),
                            width: w,
                            height: h,
                        },
                        rx,
                    )
                }
                "circle" => {
                    let r = attr("r")?;
                    let (cx, cy) = (attr("cx").unwrap_or(0.), attr("cy").unwrap_or(0.));
                    (
                        Rect {
                            x: cx - r,
                            y: cy - r,
                            width: r * 2.,
                            height: r * 2.,
                        },
                        r,
                    )
                }
                _ => {
                    let (rx, ry) = (attr("rx")?, attr("ry")?);
                    // Only round ones: an oval isn't a rounded rectangle.
                    if (rx - ry).abs() > 0.5 {
                        return None;
                    }
                    let (cx, cy) = (attr("cx").unwrap_or(0.), attr("cy").unwrap_or(0.));
                    (
                        Rect {
                            x: cx - rx,
                            y: cy - ry,
                            width: rx * 2.,
                            height: ry * 2.,
                        },
                        rx,
                    )
                }
            };
            let mut layer = blank(id, String::new(), place(bounds));
            layer.name = if node.tag_name().name() == "rect" { "Rectangle" } else { "Circle" }.into();
            layer.kind = LayerKind::Shape {
                fill,
                radius: radius * scale,
                fill_opacity: fill_opacity.clamp(0., 1.),
                stroke_width: if stroke.is_some() { stroke_width * scale } else { 0. },
                stroke,
                soft: 0.,
            };
            layer
        }
        "image" => {
            let href = node
                .attribute("href")
                .or_else(|| node.attribute(("http://www.w3.org/1999/xlink", "href")))?;
            let (mime, data) = href.strip_prefix("data:")?.split_once(";base64,")?;
            let extension = match mime {
                "image/png" => "png",
                "image/jpeg" | "image/jpg" => "jpg",
                "image/webp" => "webp",
                _ => return None,
            };
            let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).ok()?;
            std::fs::create_dir_all(folder).ok()?;
            let file = format!("img-{}.{extension}", uuid::Uuid::new_v4().simple());
            std::fs::write(folder.join(&file), bytes).ok()?;
            let bounds = Rect {
                x: attr("x").unwrap_or(0.),
                y: attr("y").unwrap_or(0.),
                width: attr("width")?,
                height: attr("height")?,
            };
            let rect = place(bounds);
            let mut layer = Layer::image(target, id, file, rect.width, rect.height);
            layer.x = rect.x;
            layer.y = rect.y;
            if let LayerKind::Image { radius, shadow, .. } = &mut layer.kind {
                *radius = 0.;
                *shadow = false;
            }
            layer
        }
        _ => return None,
    };
    layer.opacity = opacity.clamp(0., 1.);
    if let Some(label) = name(node) {
        layer.name = label;
    }
    Some(layer)
}

/// Where an element draws, in the drawing's own units: render it alone on
/// the whole drawing and measure, so transforms and strokes count.
fn bounds(markup: &str, support: &str, view: Rect) -> Option<Rect> {
    let document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="{} {} {} {}" width="{}" height="{}">{support}{markup}</svg>"#,
        view.x, view.y, view.width, view.height, view.width, view.height
    );
    let options = usvg::Options {
        fontdb: text::fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(&document, &options).ok()?;
    let b = tree.root().abs_stroke_bounding_box();
    (b.width() > 0. && b.height() > 0.).then(|| Rect {
        x: b.x() + view.x,
        y: b.y() + view.y,
        width: b.width(),
        height: b.height(),
    })
}

/// A vector layer's drawing at `width` × `height` pixels, as PNG, for the
/// editor's canvas.
pub fn rasterize_svg(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let options = usvg::Options {
        fontdb: text::fonts(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
    let size = tree.size();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(width as f32 / size.width(), height as f32 / size.height()),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DRAWING: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#f97316"/><stop offset="1" stop-color="#db2777"/></linearGradient></defs>
      <rect id="card" x="20" y="20" width="360" height="200" rx="24" fill="#ffffff"/>
      <circle cx="80" cy="90" r="30" fill="url(#g)"/>
      <text id="title" x="200" y="160" font-size="28" font-weight="bold" fill="#111827" text-anchor="middle">Sleep better</text>
      <path id="wave" d="M20 250 C 120 200, 280 300, 380 250" stroke="#db2777" stroke-width="6" fill="none"/>
      <g id="stars"><circle cx="300" cy="70" r="6" fill="#facc15"/><circle cx="330" cy="90" r="4" fill="#facc15"/></g>
    </svg>"##;

    #[test]
    fn splits_a_drawing_into_editable_layers() {
        let mut id = 0;
        let folder = std::env::temp_dir();
        let layers = import_svg(DRAWING, Target::Iphone, Placement::default(), &folder, &mut id).unwrap();
        let kinds: Vec<&str> = layers.iter().map(|l| l.label()).collect();
        // The rectangle and the text stay editable; the gradient circle,
        // the path and the group become vector layers.
        assert_eq!(kinds, ["Shape", "Vector", "Text", "Vector", "Vector"]);
        assert_eq!(layers[0].name, "card");
        assert_eq!(layers[3].name, "wave");
        assert!(matches!(&layers[2].kind, LayerKind::Text { text, align: Align::Center, weight: 700, .. } if text == "Sleep better"));
        // Placed at 80% of the canvas width: the 400-wide drawing scales up.
        let scale = Target::Iphone.canvas().0 * 0.8 / 400.;
        assert!((layers[0].width - 360. * scale).abs() < 0.5);
        // The vector carries the gradient it needs, and renders.
        let LayerKind::Vector { svg } = &layers[1].kind else { panic!() };
        assert!(svg.contains("linearGradient"));
        assert!(rasterize_svg(svg, 60, 60).is_some());
        // The group's two stars measure as one piece.
        assert!((layers[4].width - 42. * scale).abs() < 2. * scale);
        // The export draws it all.
        let mut slide = crate::Slide::starter(Target::Iphone, "x", &mut id);
        slide.layers = layers;
        let png = crate::render_png(std::slice::from_ref(&slide), 0, crate::CanvasSize::Iphone69, &folder, 0.3).unwrap();
        if let Ok(out) = std::env::var("APPDOCTOR_IMPORT_OUT") {
            std::fs::write(out, png).unwrap();
        }
    }

    #[test]
    fn refuses_what_isnt_a_drawing() {
        let mut id = 0;
        let folder = std::env::temp_dir();
        assert!(import_svg("<html/>", Target::Iphone, Placement::default(), &folder, &mut id).is_err());
        assert!(import_svg("<svg xmlns='http://www.w3.org/2000/svg'/>", Target::Iphone, Placement::default(), &folder, &mut id).is_err());
    }
}
