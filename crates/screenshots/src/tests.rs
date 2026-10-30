use std::path::Path;

use super::*;

fn starter() -> Slide {
    let mut id = 0;
    Slide::starter(Target::Iphone, "Plan your day", &mut id)
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
    assert!((device.height / device.width - DeviceModel::IphoneIsland.aspect()).abs() < 1e-3);
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
    let device = Layer::device(Target::Iphone, 1, 600.);
    let g = device_geometry(&device, DeviceModel::IphoneIsland, Frame::Black);
    assert!(g.screen.x > g.glass.x && g.glass.x > g.body.x && g.screen.width < g.body.width);
    assert!(g.island.is_some() && g.notch.is_none() && g.home_button.is_none());
    assert_eq!(g.buttons.len(), 4);
    let bare = device_geometry(&device, DeviceModel::IphoneIsland, Frame::None);
    assert_eq!(bare.screen, bare.body);
    assert!(bare.island.is_none() && bare.buttons.is_empty());
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
    slide.layers.push(Layer::shape(Target::Iphone, 9));
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
        target: Target::Iphone,
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
    let mut first = Slide::starter(Target::Iphone, "One", &mut id);
    let second = Slide::starter(Target::Iphone, "Two", &mut id);
    // A phone straddling the edge between the slides.
    first.layers[2].x = BASE_WIDTH - 300.;
    let slides = vec![first, second];

    let on_second = visible_layers(&slides, 1, BASE_WIDTH);
    let spill: Vec<_> = on_second.iter().filter(|(owner, _)| *owner == 0).collect();
    assert_eq!(spill.len(), 1);
    assert_eq!(spill[0].1.x, -300.);
    assert_eq!(
        visible_layers(&slides, 0, BASE_WIDTH)
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

#[test]
fn every_model_fits_a_screen_of_the_screenshot_shape() {
    for model in DeviceModel::ALL {
        let mut layer = Layer::device(model.target(), 1, 0.);
        layer.height = layer.width * model.aspect();
        for frame in Frame::ALL {
            let g = device_geometry(&layer, model, frame);
            let screen_aspect = g.screen.height / g.screen.width;
            if frame != Frame::None {
                assert!(
                    (screen_aspect - model.screen_aspect()).abs() < 0.01,
                    "{model:?}: screen {screen_aspect}"
                );
                assert!(g.screen.x >= g.glass.x && g.glass.x >= g.body.x);
            }
            for part in [g.island, g.notch, g.home_button, g.speaker, g.camera].into_iter().flatten() {
                assert!(part.x >= g.body.x && part.x + part.width <= g.body.x + g.body.width, "{model:?}");
                assert!(part.y >= g.body.y && part.y + part.height <= g.body.y + g.body.height, "{model:?}");
            }
        }
        assert_eq!(DeviceModel::from_slug(model.slug()), Some(model));
        if model != DeviceModel::IphoneClassic {
            let (w, h) = model.target().canvas();
            assert!((model.screen_aspect() - h / w).abs() < 0.01, "{model:?}");
        }
    }
    for frame in Frame::ALL {
        assert_eq!(Frame::from_slug(frame.slug()), Some(frame));
    }
}

#[test]
fn ipad_designs_render_at_ipad_sizes() {
    let mut id = 0;
    let slide = Slide::starter(Target::Ipad, "Plan on the big screen", &mut id);
    assert!(matches!(
        slide.layers[2].kind,
        LayerKind::Device {
            model: DeviceModel::IpadPro,
            ..
        }
    ));
    let dir = std::env::temp_dir();
    for size in Target::Ipad.sizes() {
        let png = render_png(std::slice::from_ref(&slide), 0, *size, &dir, 0.1).unwrap();
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        assert_eq!(width, (size.pixels().0 as f32 * 0.1).round() as u32);
    }
    let svg = to_svg(std::slice::from_ref(&slide), 0, CanvasSize::Ipad13);
    assert!(svg.contains(r#"width="2064" height="2752""#));
}

#[test]
fn text_keeps_its_font_and_falls_back_when_missing() {
    let mut slide = starter();
    if let LayerKind::Text { font, italic, .. } = &mut slide.layers[0].kind {
        *font = Some("No Such Font \"x".into());
        *italic = true;
    }
    slide.layers[0].fit_text();
    assert!(!slide.layers[0].lines().is_empty());
    let svg = to_svg(std::slice::from_ref(&slide), 0, CanvasSize::Iphone69);
    assert!(svg.contains("font-family=\"'No Such Font x', 'Helvetica Neue'"));
    assert!(svg.contains(r#"font-style="italic""#));
    // Still renders, in a fallback font.
    render_png(std::slice::from_ref(&slide), 0, CanvasSize::Iphone69, &std::env::temp_dir(), 0.1).unwrap();
}

#[test]
fn measured_wrapping_follows_the_font() {
    let regular = Style {
        family: None,
        weight: 400,
        italic: false,
    };
    let lines = wrap_styled("Plan your whole day in seconds", regular, 100., 900.);
    assert!(lines.len() >= 2);
    assert!(lines.iter().all(|line| !line.is_empty()));
    // Bold is wider than regular in any real font.
    let bold = Style { weight: 800, ..regular };
    assert!(measure_for_test("Headline", bold) >= measure_for_test("Headline", regular));
}

fn measure_for_test(text: &str, style: Style) -> f32 {
    crate::text::measure(text, style)
}

#[test]
fn designs_keep_their_target() {
    let dir = std::env::temp_dir().join(format!("appdoctor-target-{}", std::process::id()));
    let mut id = 0;
    let design = Design {
        version: 2,
        target: Target::Ipad,
        slides: vec![Slide::starter(Target::Ipad, "Big", &mut id)],
    };
    design.save(&dir.join(Target::Ipad.file_name())).unwrap();
    assert_eq!(Design::load_for(&dir, Target::Ipad), design);
    assert!(Design::load_for(&dir, Target::Iphone).slides.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}
