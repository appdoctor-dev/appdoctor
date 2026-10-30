//! Creative tools for agents making videos: a whole storyboard in one call,
//! themes, camera zooms, callouts, a contact sheet of the video to look at,
//! and a review that says what to fix.

use std::path::Path;

use appdoctor_screenshots::tiny_skia::{Color, Pixmap, PixmapPaint, Transform};
use appdoctor_screenshots::{
    Align, Background, Design, DeviceModel, Drift, Effect, Focus, Frame, Layer, LayerKind, Rect, Slide, THEMES, Target,
    Theme, Transition, Video, device_geometry, is_video, motion, parse_color, render_frame, theme,
};
use base64::Engine as _;
use serde_json::{Value, json};

use crate::design::{Args, Doc, Result, design_args, import_screen, text, tool, with};

/// Where a scene's parts go.
const LAYOUTS: &[&str] = &["device-bottom", "device-top", "device-right", "device-left", "full-screen", "text-only"];

pub(crate) fn tools() -> Vec<Value> {
    let point = json!({
        "at": { "type": "number", "minimum": 0, "description": "Seconds into the scene." },
        "x": { "type": "number", "description": "Across the device's screen, 0 (left) to 1 (right); or canvas pixels when the scene has no device." },
        "y": { "type": "number", "description": "Down the device's screen, 0 (top) to 1 (bottom); or canvas pixels without a device." }
    });
    let zoom = with(
        point.clone(),
        json!({
            "zoom": { "type": "number", "minimum": 1, "maximum": 4, "description": "1.9 by default." },
            "hold": { "type": "number", "minimum": 0, "description": "Seconds held there; 1.2 by default." },
            "length": { "type": "number", "minimum": 0.1, "description": "Seconds each move takes; 0.6 by default." }
        }),
    );
    let callout = with(
        point,
        json!({
            "kind": { "type": "string", "enum": ["tap", "ring", "badge", "glow"], "description": "tap: a touch ripple; ring: an outline around something; badge: a pill with text (\"New\", \"4.9 ★\"); glow: a soft light behind." },
            "text": { "type": "string", "description": "Badges: the words." },
            "width": { "type": "number", "description": "Rings: share of the screen's width (0.5 by default)." },
            "height": { "type": "number", "description": "Rings: share of the screen's height (0.08 by default)." },
            "hold": { "type": "number", "description": "Seconds it stays; depends on the kind by default." }
        }),
    );
    vec![
        tool(
            "create_video",
            "Create a video from a storyboard",
            "Builds a whole video in one call: each scene gets a headline (mark accent words *like this*), an optional subtitle, the app's screen (an image or a screen recording) in a device, a layout, tasteful motion, optional camera zooms and callouts, all in a theme. Replaces the design's slides unless keep is true. Look at the result with render_storyboard and check it with review_video, then refine with the other tools. Good app previews: 4–7 scenes, one benefit per scene, headlines of 2–5 words, 15–30 s in all.",
            with(
                design_args(),
                json!({
                    "theme": { "type": "string", "enum": THEMES.iter().map(|t| t.name).collect::<Vec<_>>(), "description": "aurora by default; list_themes describes them." },
                    "transition": { "type": "string", "enum": ["push", "fade", "cut"] },
                    "keep": { "type": "boolean", "description": "Add after the existing slides instead of replacing them." },
                    "scenes": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 12,
                        "items": {
                            "type": "object",
                            "properties": {
                                "headline": { "type": "string" },
                                "subtitle": { "type": "string" },
                                "screen": { "type": "string", "description": "Path to a screenshot or screen recording of the app on this computer." },
                                "layout": { "type": "string", "enum": LAYOUTS, "description": "device-bottom for iPhone and iPad by default, device-right for the header and search." },
                                "model": { "type": "string", "description": "A device model slug (design_options); the design's usual device by default." },
                                "duration": { "type": "number", "minimum": 1, "maximum": 15, "description": "Seconds; fits the recording (up to 8 s) or 3.5 s by default." },
                                "zoom": { "type": "array", "items": { "type": "object", "properties": zoom, "required": ["at", "x", "y"] } },
                                "callouts": { "type": "array", "items": { "type": "object", "properties": callout, "required": ["kind", "at", "x", "y"] } }
                            },
                            "required": ["headline"]
                        }
                    }
                }),
            ),
            &["app_id", "scenes"],
            false,
        ),
        tool(
            "list_themes",
            "List themes",
            "The curated looks create_video and apply_theme use: colours, fonts, device finish and glow.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "apply_theme",
            "Apply a theme",
            "Restyles every slide of a design: background, headline and subtitle colours and fonts, accent colour, device finishes and glows.",
            with(design_args(), json!({ "theme": { "type": "string", "enum": THEMES.iter().map(|t| t.name).collect::<Vec<_>>() } })),
            &["app_id", "theme"],
            false,
        ),
        tool(
            "set_camera",
            "Zoom the camera",
            "Sets a scene's camera moves: each zooms in on a point, holds, and zooms back out, the classic \"look at this button\" move. With device_id, x and y are shares of that device's screen; otherwise canvas pixels. Replaces the scene's earlier moves; an empty list removes them.",
            with(
                design_args(),
                json!({
                    "slide": { "type": "integer", "minimum": 0 },
                    "device_id": { "type": "integer", "description": "A device layer whose screen the points are on." },
                    "zooms": { "type": "array", "items": { "type": "object", "properties": zoom, "required": ["at", "x", "y"] } }
                }),
            ),
            &["app_id", "slide", "zooms"],
            false,
        ),
        tool(
            "add_callout",
            "Add a callout",
            "Points something out at a moment: a tap ripple, a ring around a control, a badge, or a soft glow. With device_id, x and y are shares of that device's screen; otherwise canvas pixels.",
            with(
                design_args(),
                with(
                    json!({
                        "slide": { "type": "integer", "minimum": 0 },
                        "device_id": { "type": "integer" },
                        "color": { "type": "string", "description": "#rrggbb; the accent by default." }
                    }),
                    callout,
                ),
            ),
            &["app_id", "slide", "kind", "at", "x", "y"],
            false,
        ),
        tool(
            "render_storyboard",
            "Look at the whole video",
            "One image with a frame every `every` seconds, left to right and top to bottom, so you can see the whole video's flow at once.",
            with(
                design_args(),
                json!({ "every": { "type": "number", "minimum": 0.25, "description": "Seconds between frames; picks about 18 frames by default." } }),
            ),
            &["app_id"],
            true,
        ),
        tool(
            "review_video",
            "Review the video",
            "Checks the video like an App Store editor would and lists what to fix: length against Apple's limits, text that isn't on screen long enough to read, low contrast, text over the device, recordings that freeze, words outside the header's safe area, scenes with no motion, layers off the canvas.",
            design_args(),
            &["app_id"],
            true,
        ),
    ]
}

/// Runs one of these tools, or `None` when `name` isn't one.
pub(crate) fn call(data_dir: &Path, name: &str, args: &Args) -> Option<Result<Vec<Value>>> {
    let edit = |f: fn(&mut Doc, &Args) -> Result<Value>| -> Result<Vec<Value>> {
        let mut doc = Doc::open(data_dir, args)?;
        let result = f(&mut doc, args)?;
        doc.save()?;
        Ok(text(result))
    };
    Some(match name {
        "create_video" => edit(create_video),
        "apply_theme" => edit(apply_theme),
        "set_camera" => edit(set_camera),
        "add_callout" => edit(add_callout),
        "list_themes" => Ok(text(json!(
            THEMES
                .iter()
                .map(|t| json!({
                    "theme": t.name,
                    "description": t.description,
                    "background": [t.from, t.to],
                    "text": t.text,
                    "accent": t.accent,
                    "font": t.font,
                    "frame": t.frame.slug(),
                }))
                .collect::<Vec<_>>()
        ))),
        "render_storyboard" => Doc::open(data_dir, args).and_then(|doc| storyboard(&doc, args)),
        "review_video" => Doc::open(data_dir, args).map(|doc| text(review(&doc))),
        _ => return None,
    })
}

fn pick_theme(args: &Args) -> Result<&'static Theme> {
    match args.opt_str("theme") {
        None => Ok(&THEMES[0]),
        Some(name) => theme(name).ok_or_else(|| format!("No theme {name:?}; list_themes has them.")),
    }
}

fn apply_theme(doc: &mut Doc, args: &Args) -> Result<Value> {
    let theme = pick_theme(args)?;
    doc.design.apply_theme(theme);
    Ok(json!({ "theme": theme.name, "slides": doc.design.slides.len() }))
}

/// Builds the scenes of a storyboard.
fn create_video(doc: &mut Doc, args: &Args) -> Result<Value> {
    let scenes = args
        .get("scenes")
        .and_then(Value::as_array)
        .filter(|scenes| !scenes.is_empty())
        .ok_or("scenes is required: a list of scenes, each with at least a headline")?;
    if scenes.len() > 12 {
        return Err("At most 12 scenes.".into());
    }
    let theme = pick_theme(args)?;
    let target = doc.target;
    let keep = args.opt_bool("keep")?.unwrap_or(false);
    let mut next = if keep { doc.design.max_id() } else { 0 };
    let mut built = Vec::new();
    for (index, scene) in scenes.iter().enumerate() {
        let scene = Args(scene);
        let headline = scene.opt_str("headline").ok_or(format!("Scene {index} needs a headline."))?;
        let screen = scene
            .opt_str("screen")
            .map(|path| import_screen(path, &doc.folder))
            .transpose()?;
        let layout = scene
            .opt_str("layout")
            .unwrap_or(if target.is_screenshot() {
                "device-bottom"
            } else {
                "device-right"
            })
            .to_owned();
        let layout = layout.as_str();
        if !LAYOUTS.contains(&layout) {
            return Err(format!("Scene {index}: layout should be one of {}.", LAYOUTS.join(", ")));
        }
        let model = match scene.opt_str("model") {
            Some(slug) => DeviceModel::from_slug(slug).ok_or_else(|| format!("No model {slug:?}; design_options lists them."))?,
            None => target.default_model(),
        };
        let mut slide = lay_out(target, layout, headline, scene.opt_str("subtitle"), screen.clone(), model, &mut next);
        slide.duration = match scene.opt_f32("duration")? {
            Some(seconds) => seconds.clamp(1., 15.),
            None => match &screen {
                Some(file) if is_video(file) => appdoctor_video_length(&doc.folder.join(file)).map_or(4., |l| l.clamp(2.5, 8.)),
                _ if layout == "text-only" => 2.5,
                _ => 3.5,
            },
        };
        built.push((slide, layout.to_owned(), scene.0.clone()));
    }

    let first_new = if keep { doc.design.slides.len() } else { 0 };
    if !keep {
        doc.design.slides.clear();
    }
    doc.design.slides.extend(built.iter().map(|(slide, ..)| slide.clone()));
    // The theme styles the words and devices; callouts are added after, in
    // the theme's accent, so it leaves their colours alone.
    let themed: Vec<Slide> = {
        let mut design = Design {
            slides: doc.design.slides[first_new..].to_vec(),
            ..Design::default()
        };
        design.apply_theme(theme);
        design.slides
    };
    for (offset, slide) in themed.into_iter().enumerate() {
        doc.design.slides[first_new + offset] = slide;
    }
    let mut notes = Vec::new();
    for (offset, (_, layout, scene)) in built.iter().enumerate() {
        let index = first_new + offset;
        let slide = &mut doc.design.slides[index];
        let scene = Args(scene);
        let device = slide.layers.iter().find(|l| matches!(l.kind, LayerKind::Device { .. })).cloned();
        if let Some(zooms) = scene.get("zoom").and_then(Value::as_array) {
            slide.camera = zooms
                .iter()
                .map(|zoom| focus(&Args(zoom), device.as_ref()))
                .collect::<Result<_>>()?;
        }
        if let Some(callouts) = scene.get("callouts").and_then(Value::as_array) {
            for callout in callouts {
                let mut id = doc.design.max_id();
                let layers = make_callout(target, &Args(callout), device.as_ref(), theme.accent, &mut id)?;
                doc.design.slides[index].layers.extend(layers);
            }
        }
        if device.is_none() && layout != "text-only" {
            notes.push(format!("Scene {index} has no screen: pass screen, or it shows an empty device."));
        }
    }
    doc.design.video = Some(Video {
        transition: match args.opt_str("transition") {
            Some(slug) => Transition::from_slug(slug).ok_or("transition should be push, fade or cut")?,
            None => doc.design.video.as_ref().map_or(Transition::Push, |video| video.transition),
        },
        ..doc.design.video.clone().unwrap_or_default()
    });
    Ok(json!({
        "scenes": doc.design.slides.len(),
        "theme": theme.name,
        "length": motion::duration(&doc.design.slides),
        "length_problem": appdoctor_video::length_problem(&doc.design),
        "notes": notes,
        "next": "Look at it with render_storyboard, check it with review_video, then export_video.",
    }))
}

fn appdoctor_video_length(path: &Path) -> Option<f32> {
    #[cfg(target_os = "macos")]
    {
        appdoctor_video::clip_duration(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        None
    }
}

/// One scene laid out on the canvas, with its motion.
fn lay_out(
    target: Target,
    layout: &str,
    headline: &str,
    subtitle: Option<&str>,
    screen: Option<String>,
    model: DeviceModel,
    next: &mut u64,
) -> Slide {
    let mut id = || {
        *next += 1;
        *next
    };
    let (w, h) = target.canvas();
    let unit = w.min(h * 0.62);
    let mut layers = Vec::new();
    let wide = !target.is_screenshot() || matches!(layout, "device-right" | "device-left");
    let headline_size = if wide { unit * 0.095 } else { unit * 0.085 };
    let mut title = Layer::text(target, id(), headline, 0., headline_size, 800, "#ffffff");
    title.motion.enter = Effect::Rise;
    title.motion.start = 0.1;
    let mut sub = subtitle.map(|words| {
        let mut sub = Layer::text(target, id(), words, 0., headline_size * 0.48, 500, "#e5e7eb");
        sub.motion.enter = Effect::Fade;
        sub.motion.start = 0.45;
        sub
    });
    let device = |id: u64, width: f32| {
        let mut device = Layer::device(target, id, 0.);
        device.width = width;
        device.height = width * model.aspect();
        device.name = model.target().label().into();
        if let LayerKind::Device { model: m, screenshot, .. } = &mut device.kind {
            *m = model;
            *screenshot = screen.clone();
        }
        device.motion.enter = Effect::Rise;
        device.motion.length = 0.9;
        device
    };
    let place_text = |title: &mut Layer, sub: &mut Option<Layer>, x: f32, width: f32, y: f32, align: Align| {
        for layer in std::iter::once(&mut *title).chain(sub.iter_mut()) {
            layer.x = x;
            layer.width = width;
            if let LayerKind::Text { align: a, .. } = &mut layer.kind {
                *a = align;
            }
            layer.fit_text();
        }
        title.y = y;
        if let Some(sub) = sub {
            sub.y = title.y + title.height + headline_size * 0.3;
        }
        sub.as_ref().map_or(title.y + title.height, |s| s.y + s.height)
    };
    let margin = w * 0.07;

    match layout {
        "text-only" => {
            title.motion.enter = Effect::Words;
            title.motion.length = 1.2;
            title.motion.start = 0.2;
            if let LayerKind::Text { size, .. } = &mut title.kind {
                *size = unit * 0.11;
            }
            if let Some(sub) = &mut sub {
                sub.motion.start = 1.3;
            }
            let bottom = place_text(&mut title, &mut sub, margin, w - margin * 2., h * 0.36, Align::Center);
            let shift = (h - (bottom - title.y)) / 2. - title.y;
            title.y += shift;
            if let Some(sub) = &mut sub {
                sub.y += shift;
            }
            // Two soft glows drifting behind the words.
            for (n, (x, y, drift)) in [(-0.2, 0.05, Drift::Right), (0.45, 0.55, Drift::Left)].into_iter().enumerate() {
                let mut glow = Layer::shape(target, id());
                glow.name = format!("Glow {}", n + 1);
                glow.x = w * x;
                glow.y = h * y;
                glow.width = w * 0.8;
                glow.height = w * 0.8;
                glow.motion.drift = drift;
                if let LayerKind::Shape {
                    radius,
                    fill_opacity,
                    soft,
                    ..
                } = &mut glow.kind
                {
                    *radius = w * 0.4;
                    *fill_opacity = 0.55;
                    *soft = w * 0.22;
                }
                layers.push(glow);
            }
        }
        "full-screen" => {
            let mut full = device(id(), w);
            full.x = 0.;
            full.y = 0.;
            full.height = h;
            full.motion.enter = Effect::Fade;
            full.motion.drift = Drift::ZoomIn;
            if let LayerKind::Device { frame, shadow, .. } = &mut full.kind {
                *frame = Frame::None;
                *shadow = false;
            }
            layers.push(full);
            let bottom = place_text(&mut title, &mut sub, margin, w - margin * 2., h * 0.06, Align::Center);
            let mut band = Layer::shape(target, id());
            band.name = appdoctor_screenshots::themes::SHADE.into();
            band.x = -w * 0.1;
            band.y = -h * 0.05;
            band.width = w * 1.2;
            band.height = bottom + h * 0.08;
            if let LayerKind::Shape {
                fill,
                fill_opacity,
                soft,
                radius,
                ..
            } = &mut band.kind
            {
                *fill = "#000000".into();
                *fill_opacity = 0.7;
                *soft = h * 0.05;
                *radius = 0.;
            }
            layers.push(band);
        }
        "device-right" | "device-left" => {
            let device_height = h * if target.is_screenshot() { 0.62 } else { 0.86 };
            let mut phone = device(id(), device_height / model.aspect());
            phone.y = (h - device_height) / 2.;
            let (inset, _) = target.safe_inset().unwrap_or((0.07, 0.));
            let text_width = w * (0.5 - inset) - margin * 0.5;
            let right = layout == "device-right";
            phone.x = if right { w * 0.62 } else { w * 0.38 - phone.width };
            phone.motion.enter = Effect::Slide;
            let x = if right { w * inset } else { w * 0.52 };
            let bottom = place_text(&mut title, &mut sub, x, text_width, 0., if right { Align::Left } else { Align::Right });
            let shift = (h - bottom) / 2.;
            title.y += shift;
            if let Some(sub) = &mut sub {
                sub.y += shift;
            }
            layers.push(phone);
        }
        "device-top" => {
            let mut phone = device(id(), w * if target == Target::Ipad { 0.7 } else { 0.76 });
            phone.x = (w - phone.width) / 2.;
            phone.y = -phone.height * 0.38;
            phone.motion.enter = Effect::Fade;
            phone.motion.drift = Drift::Down;
            place_text(&mut title, &mut sub, margin, w - margin * 2., phone.y + phone.height + unit * 0.09, Align::Center);
            layers.push(phone);
        }
        _ => {
            let bottom = place_text(&mut title, &mut sub, margin, w - margin * 2., h * 0.055, Align::Center);
            let mut phone = device(id(), w * if target == Target::Ipad { 0.7 } else { 0.76 });
            phone.x = (w - phone.width) / 2.;
            phone.y = bottom + unit * 0.07;
            layers.push(phone);
        }
    }
    layers.push(title);
    layers.extend(sub);
    Slide {
        background: Background::Gradient {
            from: "#4f46e5".into(),
            to: "#9333ea".into(),
            angle: 160.,
        },
        layers,
        duration: motion::DEFAULT_SCENE,
        spin: 0.,
        camera: Vec::new(),
    }
}

/// A point given as shares of a device's screen, or canvas pixels.
fn point(args: &Args, device: Option<&Layer>) -> Result<(f32, f32)> {
    let x = args.opt_f32("x")?.ok_or("x is required")?;
    let y = args.opt_f32("y")?.ok_or("y is required")?;
    Ok(match device.and_then(screen_of) {
        Some(screen) => (screen.x + x.clamp(0., 1.) * screen.width, screen.y + y.clamp(0., 1.) * screen.height),
        None => (x, y),
    })
}

fn screen_of(layer: &Layer) -> Option<Rect> {
    match &layer.kind {
        LayerKind::Device { model, frame, .. } => Some(device_geometry(layer, *model, *frame).screen),
        _ => None,
    }
}

fn focus(args: &Args, device: Option<&Layer>) -> Result<Focus> {
    let (x, y) = point(args, device)?;
    Ok(Focus {
        at: args.opt_f32("at")?.ok_or("at is required")?.max(0.),
        x,
        y,
        zoom: args.opt_f32("zoom")?.unwrap_or(1.9).clamp(1., 4.),
        hold: args.opt_f32("hold")?.unwrap_or(1.2).max(0.),
        length: args.opt_f32("length")?.unwrap_or(0.6).max(0.1),
    })
}

fn set_camera(doc: &mut Doc, args: &Args) -> Result<Value> {
    let index = args.index("slide")?;
    let device = match args.opt_u64("device_id")? {
        Some(id) => {
            let (s, l) = doc.find(id)?;
            Some(doc.design.slides[s].layers[l].clone())
        }
        None => None,
    };
    let zooms = args.get("zooms").and_then(Value::as_array).ok_or("zooms is required")?;
    let camera = zooms.iter().map(|zoom| focus(&Args(zoom), device.as_ref())).collect::<Result<Vec<_>>>()?;
    let slide = doc.slide(index)?;
    slide.camera = camera.clone();
    Ok(json!({ "slide": index, "camera": camera }))
}

fn add_callout(doc: &mut Doc, args: &Args) -> Result<Value> {
    let index = args.index("slide")?;
    doc.slide(index)?;
    let device = match args.opt_u64("device_id")? {
        Some(id) => {
            let (s, l) = doc.find(id)?;
            Some(doc.design.slides[s].layers[l].clone())
        }
        None => None,
    };
    let accent = match args.color("color")? {
        Some(color) => color,
        None => accent_of(&doc.design.slides[index]).unwrap_or_else(|| THEMES[0].accent.into()),
    };
    let mut id = doc.design.max_id();
    let layers = make_callout(doc.target, args, device.as_ref(), &accent, &mut id)?;
    let ids: Vec<u64> = layers.iter().map(|l| l.id).collect();
    doc.slide(index)?.layers.extend(layers);
    Ok(json!({ "slide": index, "layers": ids }))
}

/// The accent colour a slide's text already uses, if any.
fn accent_of(slide: &Slide) -> Option<String> {
    slide.layers.iter().find_map(|layer| match &layer.kind {
        LayerKind::Text { accent, .. } => accent.clone(),
        _ => None,
    })
}

/// The layers of a callout, timed from `at`.
fn make_callout(target: Target, args: &Args, device: Option<&Layer>, accent: &str, next: &mut u64) -> Result<Vec<Layer>> {
    let mut id = || {
        *next += 1;
        *next
    };
    let (cx, cy) = point(args, device)?;
    let at = args.opt_f32("at")?.ok_or("at is required")?.max(0.);
    let (w, h) = target.canvas();
    let unit = w.min(h * 0.62);
    let kind = args.opt_str("kind").ok_or("kind is required")?;
    let shape = |id: u64, name: &str, x: f32, y: f32, width: f32, height: f32| {
        let mut layer = Layer::shape(target, id);
        layer.name = name.into();
        layer.x = x;
        layer.y = y;
        layer.width = width;
        layer.height = height;
        layer
    };
    let style = |layer: &mut Layer, fill: &str, opacity: f32, stroke: Option<&str>, stroke_width: f32, soft: f32, radius: f32| {
        if let LayerKind::Shape {
            fill: f,
            fill_opacity,
            stroke: s,
            stroke_width: sw,
            soft: so,
            radius: r,
        } = &mut layer.kind
        {
            *f = fill.into();
            *fill_opacity = opacity;
            *s = stroke.map(Into::into);
            *sw = stroke_width;
            *so = soft;
            *r = radius;
        }
    };
    let timed = |layer: &mut Layer, enter: Effect, hold: f32, length: f32| {
        layer.motion.start = at;
        layer.motion.enter = enter;
        layer.motion.exit = Effect::Fade;
        layer.motion.leave = Some(at + hold);
        layer.motion.length = length;
    };
    Ok(match kind {
        "tap" => {
            let d = unit * 0.11;
            let mut tap = shape(id(), "Tap", cx - d / 2., cy - d / 2., d, d);
            style(&mut tap, "#ffffff", 0.45, Some("#ffffff"), d * 0.06, 0., d / 2.);
            timed(&mut tap, Effect::Pop, args.opt_f32("hold")?.unwrap_or(0.45), 0.3);
            vec![tap]
        }
        "ring" => {
            let screen = device.and_then(screen_of);
            let (sw, sh) = screen.map_or((w, h), |s| (s.width, s.height));
            let (rw, rh) = (
                sw * args.opt_f32("width")?.unwrap_or(0.5).clamp(0.05, 1.2),
                sh * args.opt_f32("height")?.unwrap_or(0.08).clamp(0.02, 1.),
            );
            let mut ring = shape(id(), "Ring", cx - rw / 2., cy - rh / 2., rw, rh);
            style(&mut ring, accent, 0., Some(accent), unit * 0.009, 0., rh.min(rw) * 0.3);
            timed(&mut ring, Effect::Zoom, args.opt_f32("hold")?.unwrap_or(1.8), 0.4);
            vec![ring]
        }
        "badge" => {
            let words = args.opt_str("text").unwrap_or("New");
            let height = unit * 0.065;
            let width = height * (0.9 + words.chars().count() as f32 * 0.42);
            let mut pill = shape(id(), "Badge", cx - width / 2., cy - height / 2., width, height);
            style(&mut pill, accent, 1., None, 0., 0., height / 2.);
            let mut label = Layer::text(target, id(), words, 0., height * 0.48, 700, ink_for(accent));
            label.x = pill.x;
            label.width = width;
            label.y = pill.y + (height - label.height) / 2.;
            let hold = args.opt_f32("hold")?.unwrap_or(f32::INFINITY);
            for layer in [&mut pill, &mut label] {
                layer.motion.start = at;
                layer.motion.enter = Effect::Pop;
                layer.motion.length = 0.45;
                if hold.is_finite() {
                    layer.motion.leave = Some(at + hold);
                    layer.motion.exit = Effect::Fade;
                }
            }
            vec![pill, label]
        }
        "glow" => {
            let d = unit * 0.7;
            let mut glow = shape(id(), "Glow", cx - d / 2., cy - d / 2., d, d);
            style(&mut glow, accent, 0.5, None, 0., d * 0.3, d / 2.);
            glow.motion.start = at;
            glow.motion.enter = Effect::Fade;
            glow.motion.length = 0.8;
            glow.motion.drift = Drift::ZoomIn;
            vec![glow]
        }
        other => return Err(format!("kind should be tap, ring, badge or glow, not {other:?}")),
    })
}

/// Black or white, whichever reads on `background`.
fn ink_for(background: &str) -> &'static str {
    if luminance(background) > 0.45 { "#111827" } else { "#ffffff" }
}

/// Relative luminance (WCAG) of a colour.
fn luminance(color: &str) -> f32 {
    let (r, g, b) = parse_color(color).unwrap_or((0, 0, 0));
    let channel = |c: u8| {
        let c = c as f32 / 255.;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast(a: &str, b: &str) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// A grid of frames from the whole video, for an agent to look at.
fn storyboard(doc: &Doc, args: &Args) -> Result<Vec<Value>> {
    if doc.design.slides.is_empty() {
        return Err("The design has no slides yet.".into());
    }
    let mut design = doc.design.clone();
    design.video.get_or_insert_with(Video::default);
    let length = motion::duration(&design.slides);
    let every = args.opt_f32("every")?.unwrap_or((length / 18.).max(0.5)).max(0.25);
    let times: Vec<f32> = (0..)
        .map(|n| n as f32 * every)
        .take_while(|t| *t < length)
        .take(36)
        .collect();
    let size = doc.target.video_size();
    let (vw, vh) = size.pixels();
    let tile = if vw < vh { 200. } else { 360. };
    let scale = tile / vw as f32;
    let (tw, th) = ((vw as f32 * scale).round() as u32, (vh as f32 * scale).round() as u32);
    let columns = if vw < vh { 6 } else { 3 }.min(times.len()) as u32;
    let rows = (times.len() as u32).div_ceil(columns);
    let gap = 10;
    let mut sheet = Pixmap::new(columns * (tw + gap) + gap, rows * (th + gap) + gap).ok_or("The storyboard is too big.")?;
    sheet.fill(Color::from_rgba8(24, 24, 27, 255));
    let clips = appdoctor_video::clip_source();
    for (n, time) in times.iter().enumerate() {
        let frame = render_frame(&design, *time, size, &doc.folder, scale, clips.as_ref()).map_err(|e| e.to_string())?;
        let (col, row) = (n as u32 % columns, n as u32 / columns);
        sheet.draw_pixmap(
            (gap + col * (tw + gap)) as i32,
            (gap + row * (th + gap)) as i32,
            frame.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    let png = sheet.encode_png().map_err(|e| e.to_string())?;
    let labels: Vec<String> = times
        .iter()
        .enumerate()
        .map(|(n, t)| {
            let at = motion::moment(&design.slides, design.video.as_ref().unwrap_or(&Video::default()), *t);
            format!("{}: {t:.1} s (scene {})", n + 1, at.scene)
        })
        .collect();
    Ok(vec![
        json!({ "type": "image", "data": base64::engine::general_purpose::STANDARD.encode(png), "mimeType": "image/png" }),
        json!({ "type": "text", "text": format!("{:.1} s in all. Frames left to right, top to bottom: {}", length, labels.join(", ")) }),
    ])
}

/// What an App Store editor would flag.
fn review(doc: &Doc) -> Value {
    let design = &doc.design;
    let canvas = doc.target.canvas();
    let mut issues: Vec<Value> = Vec::new();
    let mut issue = |severity: &str, scene: Option<usize>, problem: String, fix: &str| {
        issues.push(json!({ "severity": severity, "scene": scene, "problem": problem, "fix": fix }));
    };
    if design.video.is_none() {
        issue(
            "info",
            None,
            "This design isn't a video yet.".into(),
            "Call set_video or create_video.",
        );
    } else if let Some(problem) = appdoctor_video::length_problem(design) {
        issue("error", None, problem, "Change scene lengths with update_slide, or add or remove scenes.");
    }
    for (index, slide) in design.slides.iter().enumerate() {
        let colors: Vec<String> = match &slide.background {
            Background::Solid { color } => vec![color.clone()],
            Background::Gradient { from, to, .. } => vec![from.clone(), to.clone()],
        };
        let mut moving = !slide.camera.is_empty() || slide.spin != 0.;
        let devices: Vec<Rect> = slide.layers.iter().filter(|l| !l.hidden).filter_map(screen_of).collect();
        for layer in slide.layers.iter().filter(|l| !l.hidden) {
            let m = &layer.motion;
            moving |= m.enter != Effect::None || m.drift != Drift::None || m.exit != Effect::None;
            let on_screen = m.leave.unwrap_or(slide.duration).min(slide.duration) - m.start;
            if layer.x + layer.width < 0. || layer.x > canvas.0 || layer.y + layer.height < 0. || layer.y > canvas.1 {
                issue("warning", Some(index), format!("\"{}\" is entirely off the canvas.", layer.name), "Move it on, or delete it.");
            }
            match &layer.kind {
                LayerKind::Text { text, color, size, .. } => {
                    let words = text.split_whitespace().count() as f32;
                    let needed = 0.8 + words / 3.;
                    if on_screen < needed {
                        issue(
                            "warning",
                            Some(index),
                            format!("\"{}\" is on screen {on_screen:.1} s; {words:.0} words need about {needed:.1} s.", layer.name),
                            "Lengthen the scene, bring the words in sooner, or use fewer words.",
                        );
                    }
                    let worst = colors.iter().map(|bg| contrast(color, bg)).fold(f32::INFINITY, f32::min);
                    let needed = if *size >= 90. { 3. } else { 4.5 };
                    if worst < needed {
                        issue(
                            "warning",
                            Some(index),
                            format!("\"{}\" has a contrast of {worst:.1}:1 against the background (aim for {needed}:1).", layer.name),
                            "Change the text colour or the background, or apply a theme.",
                        );
                    }
                    let text_rect = Rect { x: layer.x, y: layer.y, width: layer.width, height: layer.height };
                    if devices.iter().any(|screen| overlaps(text_rect, *screen, 0.25)) {
                        issue(
                            "warning",
                            Some(index),
                            format!("\"{}\" sits over a device's screen, which makes both hard to read.", layer.name),
                            "Move the words clear of the device, or put a shade behind them.",
                        );
                    }
                    if let Some((ix, iy)) = doc.target.safe_inset() {
                        let (left, top) = (canvas.0 * ix, canvas.1 * iy);
                        if layer.x < left - 1. || layer.x + layer.width > canvas.0 - left + 1. || layer.y < top - 1. || layer.y + layer.height > canvas.1 - top + 1. {
                            issue(
                                "warning",
                                Some(index),
                                format!("\"{}\" reaches outside the header's safe area, so some devices crop it.", layer.name),
                                "Keep words inside the middle: 18% in from the sides, 15% from top and bottom.",
                            );
                        }
                    }
                }
                LayerKind::Device {
                    screenshot: Some(file),
                    clip_start,
                    clip_speed,
                    ..
                } if is_video(file) => {
                    moving = true;
                    if let Some(length) = appdoctor_video_length(&doc.folder.join(file)) {
                        let plays = clip_start + on_screen * clip_speed;
                        if plays > length + 0.2 {
                            issue(
                                "warning",
                                Some(index),
                                format!("The recording is {length:.1} s but plays for {plays:.1} s, so it freezes on its last frame."),
                                "Shorten the scene, raise clip_speed, or record longer.",
                            );
                        }
                    }
                }
                LayerKind::Device { screenshot: None, .. } => issue(
                    "warning",
                    Some(index),
                    format!("\"{}\" has no screen.", layer.name),
                    "Give it a screenshot or recording (update_layer screenshot).",
                ),
                _ => {}
            }
        }
        if design.video.is_some() && !moving {
            issue(
                "info",
                Some(index),
                "Nothing moves in this scene.".into(),
                "Give the words an enter effect, the device a drift, or the scene a camera zoom.",
            );
        }
    }
    let errors = issues.iter().filter(|i| i["severity"] == "error").count();
    json!({
        "verdict": if issues.is_empty() { "Looks ready to export." } else if errors > 0 { "Fix the errors before exporting." } else { "Worth a pass over the warnings." },
        "issues": issues,
    })
}

/// Whether `a` covers more than `share` of `b`.
fn overlaps(a: Rect, b: Rect, share: f32) -> bool {
    let w = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let h = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    w > 0. && h > 0. && w * h > share * a.width * a.height
}
