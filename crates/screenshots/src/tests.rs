use std::path::Path;

use super::*;

fn starter() -> Slide {
    let mut id = 0;
    Slide::starter("Plan your day", &mut id)
}

#[test]
fn wraps_by_width_and_newlines() {
    assert_eq!(wrap("Plan your day", 100., 2000.), vec!["Plan your day"]);
    assert_eq!(
        wrap("Plan your whole day in seconds", 100., 1000.),
        vec!["Plan your whole", "day in seconds"]
    );
    assert_eq!(wrap("One\nTwo", 100., 2000.), vec!["One", "Two"]);
    assert_eq!(wrap("One\n\nTwo", 100., 2000.), vec!["One", "", "Two"]);
    assert!(wrap("   ", 100., 100.).is_empty());
}

#[test]
fn starter_slide_has_text_and_device_layers() {
    let slide = starter();
    let kinds: Vec<_> = slide.layers.iter().map(Layer::label).collect();
    assert_eq!(kinds, vec!["Text", "Text", "Device"]);
    let ids: Vec<_> = slide.layers.iter().map(|l| l.id).collect();
    assert_eq!(ids, vec![1, 2, 3]);
    let device = &slide.layers[2];
    assert!((device.height / device.width - DEVICE_ASPECT).abs() < 1e-3);
}

#[test]
fn svg_escapes_text_hides_hidden_layers_and_sanitises_colors() {
    let mut slide = starter();
    if let LayerKind::Text { text, color, .. } = &mut slide.layers[0].kind {
        *text = "Tasks & <notes>".into();
        *color = "red\" onload=\"x".into();
    }
    slide.layers[1].hidden = true;
    let svg = to_svg(std::slice::from_ref(&slide), 0, CanvasSize::Iphone69);
    assert!(svg.contains("Tasks &amp; &lt;notes&gt;"));
    assert!(!svg.contains("onload"));
    assert!(!svg.contains("One line on why it matters"));
    assert!(svg.contains(r#"width="1320" height="2868""#));
}

#[test]
fn device_geometry_insets_the_screen_inside_the_bezel() {
    let device = Layer::device(1, 600.);
    let g = device_geometry(&device, Frame::Black);
    assert!(g.screen.x > g.body.x && g.screen.width < g.body.width);
    assert!(g.island.is_some());
    let bare = device_geometry(&device, Frame::None);
    assert_eq!(bare.screen, bare.body);
    assert!(bare.island.is_none());
}

#[test]
fn gradient_angles_follow_css() {
    let close = |a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)| {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4 && (a.2 - b.2).abs() < 1e-4 && (a.3 - b.3).abs() < 1e-4
    };
    assert!(
        close(svg::gradient_vector(180.), (0.5, 0., 0.5, 1.)),
        "180° runs top to bottom"
    );
    assert!(
        close(svg::gradient_vector(90.), (0., 0.5, 1., 0.5)),
        "90° runs left to right"
    );
}

#[test]
fn parses_colors() {
    assert_eq!(parse_color("#4f46e5"), Some((0x4f, 0x46, 0xe5)));
    assert_eq!(parse_color("fff"), Some((255, 255, 255)));
    assert_eq!(parse_color("nope"), None);
}

#[test]
fn renders_png_at_export_and_thumbnail_sizes() {
    let dir = std::env::temp_dir();
    let mut slide = starter();
    slide.layers.push(Layer::shape(9));
    for (size, scale) in [(CanvasSize::Iphone69, 1.0), (CanvasSize::Iphone65, 0.25)] {
        let png = render_png(std::slice::from_ref(&slide), 0, size, &dir, scale).unwrap();
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
fn design_round_trips_and_upgrades_the_first_format() {
    let dir = std::env::temp_dir().join(format!("appdoctor-design-{}", std::process::id()));
    let path = dir.join("design.json");
    let design = Design {
        version: 2,
        slides: vec![starter(), starter()],
    };
    design.save(&path).unwrap();
    assert_eq!(Design::load(&path), design);
    assert_eq!(Design::load(Path::new("/nonexistent/design.json")), Design::default());

    let v1 = r##"{"slides": [{
        "background": {"type": "solid", "color": "#000000"},
        "headline": {"text": "Old headline", "color": "#ffffff", "size": 104, "weight": 700},
        "subheadline": {"text": "Old sub", "color": "#dddddd", "size": 54, "weight": 400},
        "device": {"frame": "silver", "scale": 0.9, "offset": 0.0, "screenshot": "shot.png"}
    }]}"##;
    std::fs::write(&path, v1).unwrap();
    let upgraded = Design::load(&path);
    let layers = &upgraded.slides[0].layers;
    assert_eq!(layers.len(), 3);
    assert!(matches!(&layers[0].kind, LayerKind::Text { text, .. } if text == "Old headline"));
    assert!(matches!(
        &layers[2].kind,
        LayerKind::Device { frame: Frame::Silver, screenshot: Some(file), .. } if file == "shot.png"
    ));
    assert_eq!(upgraded.max_id(), 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn layers_hanging_off_a_slide_continue_on_its_neighbours() {
    let mut id = 0;
    let mut first = Slide::starter("One", &mut id);
    let second = Slide::starter("Two", &mut id);
    // A phone straddling the edge between the slides.
    first.layers[2].x = BASE_WIDTH - 300.;
    let slides = vec![first, second];

    let on_second = visible_layers(&slides, 1);
    let spill: Vec<_> = on_second.iter().filter(|(owner, _)| *owner == 0).collect();
    assert_eq!(spill.len(), 1);
    assert_eq!(spill[0].1.x, -300.);
    assert_eq!(
        visible_layers(&slides, 0)
            .iter()
            .filter(|(owner, _)| *owner == 1)
            .count(),
        0
    );

    let svg = to_svg(&slides, 1, CanvasSize::Iphone69);
    assert!(
        svg.contains(r#"x="-300""#),
        "the spanning phone is drawn on the second slide"
    );
}
