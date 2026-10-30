//! The screenshot design tools. Each call reads the design file, changes it
//! and writes it back whole, so the editor (watching the file) and the
//! agent never hold different halves of a design.

use std::path::{Path, PathBuf};

use appdoctor_screenshots::{
    Align, BACKGROUND_PRESETS, Background, DEFAULT_FONT, Design, DeviceModel, Frame, Layer, LayerKind, SUGGESTED_FONTS,
    Slide, Target, font_families, has_font, import_image, parse_color, render_png, write_atomic,
};
use appdoctor_store::Store;
use base64::Engine as _;
use serde_json::{Value, json};

type Result<T> = std::result::Result<T, String>;

/// Arguments every design tool takes.
fn design_args() -> Value {
    json!({
        "app_id": { "type": "integer", "description": "The app's App Store ID, from list_apps." },
        "device": {
            "type": "string",
            "enum": ["iphone", "ipad"],
            "description": "Which design: iPhone (canvas 1320 x 2868) or iPad (canvas 2064 x 2752). Defaults to iphone."
        }
    })
}

/// Properties a layer can be given, for add_layer and update_layer.
fn layer_properties() -> Value {
    json!({
        "name": { "type": "string", "description": "Name in the Layers list." },
        "x": { "type": "number", "description": "Left edge in canvas pixels. Past the canvas width continues onto the next slide." },
        "y": { "type": "number", "description": "Top edge in canvas pixels." },
        "width": { "type": "number", "description": "Width in canvas pixels. A device's height follows its model's shape; a text box's height follows its lines." },
        "height": { "type": "number", "description": "Height in canvas pixels, for images and shapes. For a device it sets the width instead." },
        "opacity": { "type": "number", "minimum": 0, "maximum": 1 },
        "hidden": { "type": "boolean" },
        "locked": { "type": "boolean", "description": "Locked layers can't be dragged in the editor." },
        "text": { "type": "string", "description": "Text layers: the words. \\n breaks a line; long lines wrap to the width." },
        "size": { "type": "number", "description": "Text layers: font size in canvas pixels. Headlines read well around 90-120 on iPhone, 100-140 on iPad." },
        "weight": { "type": "integer", "enum": [100, 200, 300, 400, 500, 600, 700, 800, 900] },
        "color": { "type": "string", "description": "Text colour, #rrggbb." },
        "align": { "type": "string", "enum": ["left", "center", "right"] },
        "font": { "type": "string", "description": "Text layers: an installed font family (see list_fonts). Empty for the default, Helvetica Neue." },
        "italic": { "type": "boolean" },
        "model": { "type": "string", "enum": DeviceModel::ALL.iter().map(|m| m.slug()).collect::<Vec<_>>(), "description": "Device layers: the device's shape." },
        "frame": { "type": "string", "enum": Frame::ALL.iter().map(|f| f.slug()).collect::<Vec<_>>(), "description": "Device layers: the finish, or none for just the rounded screen." },
        "screenshot": { "type": "string", "description": "Device layers: path to a PNG or JPEG of the app's screen on this computer. It's copied into the design." },
        "image": { "type": "string", "description": "Image layers: path to a PNG, JPEG or WebP on this computer. It's copied into the design." },
        "shadow": { "type": "boolean", "description": "Devices and images: a soft drop shadow." },
        "radius": { "type": "number", "description": "Images and shapes: corner radius in canvas pixels." },
        "fill": { "type": "string", "description": "Shape layers: fill colour, #rrggbb." }
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
        base.extend(extra.clone());
    }
    base
}

fn tool(name: &str, title: &str, description: &str, properties: Value, required: &[&str], read_only: bool) -> Value {
    json!({
        "name": name,
        "title": title,
        "description": description,
        "inputSchema": { "type": "object", "properties": properties, "required": required },
        "annotations": { "readOnlyHint": read_only, "destructiveHint": false, "openWorldHint": false },
    })
}

pub fn tools() -> Vec<Value> {
    let slide = json!({ "slide": { "type": "integer", "minimum": 0, "description": "The slide's index, from 0." } });
    let layer_id = json!({ "layer_id": { "type": "integer", "description": "The layer's id, from get_design." } });
    vec![
        tool(
            "list_apps",
            "List apps",
            "The apps in AppDoctor, with how many iPhone and iPad screenshots each has designed.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "get_design",
            "Get a screenshot design",
            "An app's screenshot design for iPhone or iPad: the canvas size, export sizes and every slide with its background and layers (ids, positions, text, fonts, devices). Read this before editing.",
            design_args(),
            &["app_id"],
            true,
        ),
        tool(
            "design_options",
            "Design options",
            "Allowed values: device models, frame finishes, font weights, alignments, background presets and suggested fonts that are installed.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "list_fonts",
            "List fonts",
            "Font families installed on this computer, which text layers can use. Filter with query.",
            json!({ "query": { "type": "string", "description": "Part of a family name, e.g. 'avenir'." } }),
            &[],
            true,
        ),
        tool(
            "add_slide",
            "Add a slide",
            "Adds a screenshot to the design. By default an empty slide with the previous slide's background; set starter for a headline, subtitle and device to adjust, or copy_of to duplicate a slide.",
            with(
                design_args(),
                json!({
                    "position": { "type": "integer", "minimum": 0, "description": "Index to insert at; the end by default." },
                    "copy_of": { "type": "integer", "minimum": 0, "description": "Duplicate this slide." },
                    "starter": { "type": "boolean", "description": "Start with a headline, a subtitle and a device." },
                    "headline": { "type": "string", "description": "The starter's headline." }
                }),
            ),
            &["app_id"],
            false,
        ),
        tool(
            "delete_slide",
            "Delete a slide",
            "Removes a slide and its layers.",
            with(design_args(), slide.clone()),
            &["app_id", "slide"],
            false,
        ),
        tool(
            "move_slide",
            "Move a slide",
            "Moves a slide to another position.",
            with(
                design_args(),
                json!({
                    "from": { "type": "integer", "minimum": 0 },
                    "to": { "type": "integer", "minimum": 0 }
                }),
            ),
            &["app_id", "from", "to"],
            false,
        ),
        tool(
            "set_background",
            "Set a slide's background",
            "A solid colour (color), a gradient (from, to, angle) or a named preset (see design_options). Angle follows CSS: 180 runs top to bottom, 90 left to right.",
            with(
                design_args(),
                with(
                    slide.clone(),
                    json!({
                        "color": { "type": "string", "description": "Solid colour, #rrggbb." },
                        "from": { "type": "string", "description": "Gradient start, #rrggbb." },
                        "to": { "type": "string", "description": "Gradient end, #rrggbb." },
                        "angle": { "type": "number", "description": "Gradient angle in degrees; 160 by default." },
                        "preset": { "type": "string", "description": "A preset name, e.g. Sunset." }
                    }),
                ),
            ),
            &["app_id", "slide"],
            false,
        ),
        tool(
            "add_layer",
            "Add a layer",
            "Adds a text, device, image or shape layer on top of a slide and returns it with its id. Text needs text; image needs image. Unset properties get sensible defaults.",
            with(
                design_args(),
                with(
                    slide,
                    with(
                        json!({ "kind": { "type": "string", "enum": ["text", "device", "image", "shape"] } }),
                        layer_properties(),
                    ),
                ),
            ),
            &["app_id", "slide", "kind"],
            false,
        ),
        tool(
            "update_layer",
            "Change a layer",
            "Changes only the properties given.",
            with(design_args(), with(layer_id.clone(), layer_properties())),
            &["app_id", "layer_id"],
            false,
        ),
        tool(
            "delete_layer",
            "Delete a layer",
            "Removes a layer.",
            with(design_args(), layer_id.clone()),
            &["app_id", "layer_id"],
            false,
        ),
        tool(
            "arrange_layer",
            "Arrange a layer",
            "Moves a layer in its slide's stack.",
            with(
                design_args(),
                with(
                    layer_id,
                    json!({ "to": { "type": "string", "enum": ["front", "back", "forward", "backward"] } }),
                ),
            ),
            &["app_id", "layer_id", "to"],
            false,
        ),
        tool(
            "render_slide",
            "Look at a slide",
            "Renders a slide to a PNG you can see, exactly as it exports. Use it to check your work.",
            with(
                design_args(),
                json!({
                    "slide": { "type": "integer", "minimum": 0 },
                    "scale": { "type": "number", "minimum": 0.1, "maximum": 0.5, "description": "Of the canvas size; 0.3 by default." }
                }),
            ),
            &["app_id", "slide"],
            true,
        ),
        tool(
            "export_screenshots",
            "Export screenshots",
            "Writes every slide as PNGs at the App Store sizes (iPhone 6.9\" and 6.5\", or iPad 13\" and 12.9\") into a folder.",
            with(
                design_args(),
                json!({ "folder": { "type": "string", "description": "Folder to write into; created if missing." } }),
            ),
            &["app_id", "folder"],
            false,
        ),
    ]
}

pub fn call(data_dir: &Path, name: &str, args: &Value) -> Result<Vec<Value>> {
    let args = Args(args);
    match name {
        "list_apps" => list_apps(data_dir),
        "design_options" => Ok(text(design_options())),
        "list_fonts" => Ok(text(list_fonts(args.opt_str("query")))),
        "get_design" => {
            let doc = Doc::open(data_dir, &args)?;
            Ok(text(doc.describe()))
        }
        "render_slide" => render_slide(data_dir, &args),
        "export_screenshots" => export(data_dir, &args),
        "add_slide" | "delete_slide" | "move_slide" | "set_background" | "add_layer" | "update_layer"
        | "delete_layer" | "arrange_layer" => {
            let mut doc = Doc::open(data_dir, &args)?;
            let result = match name {
                "add_slide" => add_slide(&mut doc, &args),
                "delete_slide" => delete_slide(&mut doc, &args),
                "move_slide" => move_slide(&mut doc, &args),
                "set_background" => set_background(&mut doc, &args),
                "add_layer" => add_layer(&mut doc, &args),
                "update_layer" => update_layer(&mut doc, &args),
                "delete_layer" => delete_layer(&mut doc, &args),
                _ => arrange_layer(&mut doc, &args),
            }?;
            doc.save()?;
            Ok(text(result))
        }
        _ => Err(format!("No tool called {name}.")),
    }
}

fn text(value: Value) -> Vec<Value> {
    let text = match value {
        Value::String(text) => text,
        value => serde_json::to_string_pretty(&value).unwrap_or_default(),
    };
    vec![json!({ "type": "text", "text": text })]
}

struct Args<'a>(&'a Value);

impl Args<'_> {
    fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key).filter(|value| !value.is_null())
    }

    fn opt_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    fn opt_f32(&self, key: &str) -> Result<Option<f32>> {
        match self.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_f64()
                .map(|n| Some(n as f32))
                .ok_or_else(|| format!("{key} should be a number")),
        }
    }

    fn opt_bool(&self, key: &str) -> Result<Option<bool>> {
        match self.get(key) {
            None => Ok(None),
            Some(value) => value
                .as_bool()
                .map(Some)
                .ok_or_else(|| format!("{key} should be true or false")),
        }
    }

    /// A whole number, accepting one sent as a string ("6762288498").
    fn opt_u64(&self, key: &str) -> Result<Option<u64>> {
        match self.get(key) {
            None => Ok(None),
            Some(Value::String(text)) => text
                .trim()
                .parse()
                .map(Some)
                .map_err(|_| format!("{key} should be a whole number")),
            Some(value) => value
                .as_u64()
                .map(Some)
                .ok_or_else(|| format!("{key} should be a whole number")),
        }
    }

    fn u64(&self, key: &str) -> Result<u64> {
        self.opt_u64(key)?.ok_or_else(|| format!("{key} is required"))
    }

    fn index(&self, key: &str) -> Result<usize> {
        Ok(self.u64(key)? as usize)
    }

    fn color(&self, key: &str) -> Result<Option<String>> {
        match self.opt_str(key) {
            None => Ok(None),
            Some(color) if parse_color(color).is_some() => Ok(Some(color.trim().to_owned())),
            Some(color) => Err(format!("{key} should be a #rrggbb colour, not {color:?}")),
        }
    }
}

fn store(data_dir: &Path) -> Result<Store> {
    Store::open(&data_dir.join("appdoctor.db")).map_err(|error| format!("Couldn't open AppDoctor's database: {error}"))
}

fn screenshots_folder(data_dir: &Path, app_id: u64) -> PathBuf {
    data_dir.join("screenshots").join(app_id.to_string())
}

fn list_apps(data_dir: &Path) -> Result<Vec<Value>> {
    let apps = store(data_dir)?.apps().map_err(|error| error.to_string())?;
    let apps: Vec<Value> = apps
        .iter()
        .map(|app| {
            let folder = screenshots_folder(data_dir, app.id);
            let count = |target| Design::load_for(&folder, target).slides.len();
            json!({
                "app_id": app.id,
                "name": app.name,
                "developer": app.developer,
                "iphone_slides": count(Target::Iphone),
                "ipad_slides": count(Target::Ipad),
            })
        })
        .collect();
    if apps.is_empty() {
        return Ok(text(Value::String(
            "No apps yet. Ask the user to add their app in AppDoctor (Apps, then Add app) first.".into(),
        )));
    }
    Ok(text(json!({ "apps": apps })))
}

fn design_options() -> Value {
    let installed = font_families();
    json!({
        "devices": {
            "iphone": { "canvas": Target::Iphone.canvas(), "exports": Target::Iphone.sizes().iter().map(|s| s.pixels()).collect::<Vec<_>>() },
            "ipad": { "canvas": Target::Ipad.canvas(), "exports": Target::Ipad.sizes().iter().map(|s| s.pixels()).collect::<Vec<_>>() },
        },
        "models": DeviceModel::ALL.iter().map(|m| json!({
            "model": m.slug(),
            "label": m.label(),
            "device": m.target().slug(),
            "height_per_width": m.aspect(),
        })).collect::<Vec<_>>(),
        "frames": Frame::ALL.iter().map(|f| json!({ "frame": f.slug(), "label": f.label(), "edge": f.edge_color() })).collect::<Vec<_>>(),
        "weights": { "regular": 400, "medium": 500, "semibold": 600, "bold": 700, "heavy": 800, "black": 900 },
        "align": ["left", "center", "right"],
        "default_font": DEFAULT_FONT,
        "suggested_fonts": SUGGESTED_FONTS.iter().filter(|f| installed.iter().any(|i| i == *f)).collect::<Vec<_>>(),
        "background_presets": BACKGROUND_PRESETS.iter().map(|(name, from, to, text)| json!({
            "name": name, "from": from, "to": to, "text_color": text,
        })).collect::<Vec<_>>(),
    })
}

fn list_fonts(query: Option<&str>) -> Value {
    let query = query.map(str::to_lowercase);
    let fonts: Vec<String> = font_families()
        .into_iter()
        .filter(|family| query.as_ref().is_none_or(|q| family.to_lowercase().contains(q)))
        .collect();
    let total = fonts.len();
    json!({ "fonts": fonts.into_iter().take(300).collect::<Vec<_>>(), "total": total })
}

/// One app's design for one device, read from disk.
struct Doc {
    app_id: u64,
    folder: PathBuf,
    target: Target,
    design: Design,
}

impl Doc {
    fn open(data_dir: &Path, args: &Args) -> Result<Doc> {
        let app_id = args.u64("app_id")?;
        if store(data_dir)?.app(app_id).map_err(|error| error.to_string())?.is_none() {
            return Err(format!(
                "AppDoctor has no app {app_id}. Call list_apps for the apps you can design."
            ));
        }
        let target = match args.opt_str("device") {
            None => Target::Iphone,
            Some(slug) => Target::from_slug(slug).ok_or_else(|| format!("device should be iphone or ipad, not {slug:?}"))?,
        };
        let folder = screenshots_folder(data_dir, app_id);
        let design = Design::load_for(&folder, target);
        Ok(Doc {
            app_id,
            folder,
            target,
            design,
        })
    }

    fn save(&self) -> Result<()> {
        write_atomic(&self.folder.join(self.target.file_name()), &self.design.to_json())
            .map_err(|error| format!("Couldn't save the design: {error}"))
    }

    fn next_id(&self) -> u64 {
        self.design.max_id() + 1
    }

    fn slide(&mut self, index: usize) -> Result<&mut Slide> {
        let count = self.design.slides.len();
        self.design
            .slides
            .get_mut(index)
            .ok_or_else(|| format!("There's no slide {index}; the design has {count} (numbered from 0)."))
    }

    /// (slide index, layer index) of a layer.
    fn find(&self, id: u64) -> Result<(usize, usize)> {
        self.design
            .slides
            .iter()
            .enumerate()
            .find_map(|(s, slide)| slide.layers.iter().position(|l| l.id == id).map(|l| (s, l)))
            .ok_or_else(|| format!("There's no layer {id}. get_design lists the layers."))
    }

    fn describe(&self) -> Value {
        let (width, height) = self.target.canvas();
        json!({
            "app_id": self.app_id,
            "device": self.target.slug(),
            "canvas": { "width": width, "height": height },
            "exports": self.target.sizes().iter().map(|s| s.label()).collect::<Vec<_>>(),
            "slides": self.design.slides.iter().enumerate().map(|(index, slide)| json!({
                "slide": index,
                "background": slide.background,
                "layers": slide.layers,
            })).collect::<Vec<_>>(),
            "note": if self.design.slides.is_empty() {
                "No slides yet: add_slide starts one."
            } else {
                "Layers are listed bottom to top. Positions are canvas pixels from the slide's top-left."
            },
        })
    }
}

fn add_slide(doc: &mut Doc, args: &Args) -> Result<Value> {
    let mut next = doc.design.max_id();
    let slide = if let Some(source) = args.opt_u64("copy_of")? {
        let mut copy = doc.slide(source as usize)?.clone();
        for layer in &mut copy.layers {
            next += 1;
            layer.id = next;
        }
        copy
    } else if args.opt_bool("starter")?.unwrap_or(false) || args.opt_str("headline").is_some() {
        Slide::starter(doc.target, args.opt_str("headline").unwrap_or("Your headline here"), &mut next)
    } else {
        Slide {
            background: doc
                .design
                .slides
                .last()
                .map(|slide| slide.background.clone())
                .unwrap_or_else(|| Slide::starter(doc.target, "", &mut 0).background),
            layers: Vec::new(),
        }
    };
    let count = doc.design.slides.len();
    let at = args.opt_u64("position")?.map_or(count, |p| (p as usize).min(count));
    doc.design.slides.insert(at, slide.clone());
    Ok(json!({ "slide": at, "slides": doc.design.slides.len(), "background": slide.background, "layers": slide.layers }))
}

fn delete_slide(doc: &mut Doc, args: &Args) -> Result<Value> {
    let index = args.index("slide")?;
    doc.slide(index)?;
    doc.design.slides.remove(index);
    Ok(json!({ "deleted": index, "slides": doc.design.slides.len() }))
}

fn move_slide(doc: &mut Doc, args: &Args) -> Result<Value> {
    let (from, to) = (args.index("from")?, args.index("to")?);
    doc.slide(from)?;
    let slide = doc.design.slides.remove(from);
    let to = to.min(doc.design.slides.len());
    doc.design.slides.insert(to, slide);
    Ok(json!({ "moved": from, "to": to }))
}

fn set_background(doc: &mut Doc, args: &Args) -> Result<Value> {
    let index = args.index("slide")?;
    let background = if let Some(name) = args.opt_str("preset") {
        let (_, from, to, _) = BACKGROUND_PRESETS
            .iter()
            .find(|(preset, ..)| preset.eq_ignore_ascii_case(name))
            .ok_or_else(|| format!("No preset called {name:?}; design_options lists them."))?;
        Background::Gradient {
            from: (*from).into(),
            to: (*to).into(),
            angle: args.opt_f32("angle")?.unwrap_or(160.),
        }
    } else if let Some(color) = args.color("color")? {
        Background::Solid { color }
    } else {
        match (args.color("from")?, args.color("to")?) {
            (Some(from), Some(to)) => Background::Gradient {
                from,
                to,
                angle: args.opt_f32("angle")?.unwrap_or(160.),
            },
            _ => return Err("Give color, from and to, or preset.".into()),
        }
    };
    doc.slide(index)?.background = background.clone();
    Ok(json!({ "slide": index, "background": background }))
}

fn add_layer(doc: &mut Doc, args: &Args) -> Result<Value> {
    let index = args.index("slide")?;
    doc.slide(index)?;
    let id = doc.next_id();
    let target = doc.target;
    let (width, height) = target.canvas();
    let mut layer = match args.opt_str("kind").unwrap_or_default() {
        "text" => {
            let words = args.opt_str("text").ok_or("Text layers need text.")?;
            let size = args.opt_f32("size")?.unwrap_or(96. * width / 1320.);
            Layer::text(target, id, words, height * 0.08, size, 700, "#ffffff")
        }
        "device" => Layer::device(target, id, height * 0.22),
        "image" => {
            let path = args.opt_str("image").ok_or("Image layers need image: a file path.")?;
            let (name, (w, h)) = import(path, &doc.folder)?;
            let fit = (width * 0.6).min(w.max(1.));
            Layer::image(target, id, name, fit, fit * h / w.max(1.))
        }
        "shape" => Layer::shape(target, id),
        other => return Err(format!("kind should be text, device, image or shape, not {other:?}")),
    };
    let notes = apply(&mut layer, args, &doc.folder, true)?;
    let added = layer.clone();
    doc.slide(index)?.layers.push(layer);
    Ok(json!({ "slide": index, "layer": added, "notes": notes }))
}

fn update_layer(doc: &mut Doc, args: &Args) -> Result<Value> {
    let (s, l) = doc.find(args.u64("layer_id")?)?;
    let folder = doc.folder.clone();
    let layer = &mut doc.design.slides[s].layers[l];
    let notes = apply(layer, args, &folder, false)?;
    Ok(json!({ "slide": s, "layer": layer, "notes": notes }))
}

fn delete_layer(doc: &mut Doc, args: &Args) -> Result<Value> {
    let id = args.u64("layer_id")?;
    let (s, l) = doc.find(id)?;
    doc.design.slides[s].layers.remove(l);
    Ok(json!({ "deleted": id, "slide": s }))
}

fn arrange_layer(doc: &mut Doc, args: &Args) -> Result<Value> {
    let (s, l) = doc.find(args.u64("layer_id")?)?;
    let layers = &mut doc.design.slides[s].layers;
    let last = layers.len() - 1;
    let to = match args.opt_str("to").unwrap_or_default() {
        "front" => last,
        "back" => 0,
        "forward" => (l + 1).min(last),
        "backward" => l.saturating_sub(1),
        other => return Err(format!("to should be front, back, forward or backward, not {other:?}")),
    };
    let layer = layers.remove(l);
    layers.insert(to, layer);
    Ok(json!({ "slide": s, "order": layers.iter().map(|l| l.id).collect::<Vec<_>>() }))
}

fn import(path: &str, folder: &Path) -> Result<(String, (f32, f32))> {
    let path = match path.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(path)),
        None => PathBuf::from(path),
    };
    if !path.is_file() {
        return Err(format!("There's no file at {}.", path.display()));
    }
    import_image(&path, folder).map_err(|error| format!("Couldn't use {}: {error}", path.display()))
}

/// Sets the properties `args` gives. Returns notes for the agent (a font
/// that isn't installed, say).
fn apply(layer: &mut Layer, args: &Args, folder: &Path, new: bool) -> Result<Vec<String>> {
    let mut notes = Vec::new();
    if let Some(name) = args.opt_str("name") {
        layer.name = name.to_owned();
    }
    if let Some(opacity) = args.opt_f32("opacity")? {
        layer.opacity = opacity.clamp(0., 1.);
    }
    if let Some(hidden) = args.opt_bool("hidden")? {
        layer.hidden = hidden;
    }
    if let Some(locked) = args.opt_bool("locked")? {
        layer.locked = locked;
    }

    match &mut layer.kind {
        LayerKind::Text {
            text,
            size,
            weight,
            color,
            align,
            font,
            italic,
        } => {
            if let Some(words) = args.opt_str("text") {
                *text = words.to_owned();
                if new || args.opt_str("name").is_none() {
                    layer.name = words.lines().next().unwrap_or("Text").chars().take(24).collect();
                }
            }
            if let Some(value) = args.opt_f32("size")? {
                *size = value.clamp(8., 600.);
            }
            if let Some(value) = args.opt_u64("weight")? {
                *weight = (value.clamp(100, 900) / 100 * 100) as u16;
            }
            if let Some(value) = args.color("color")? {
                *color = value;
            }
            if let Some(value) = args.opt_str("align") {
                *align = match value {
                    "left" => Align::Left,
                    "center" | "centre" => Align::Center,
                    "right" => Align::Right,
                    other => return Err(format!("align should be left, center or right, not {other:?}")),
                };
            }
            if let Some(value) = args.opt_str("font") {
                let value = value.trim();
                *font = (!value.is_empty() && value != DEFAULT_FONT).then(|| value.to_owned());
                if font.is_some() && !has_font(value) {
                    notes.push(format!(
                        "{value} isn't installed here, so it shows in {DEFAULT_FONT}. list_fonts has what is."
                    ));
                }
            }
            if let Some(value) = args.opt_bool("italic")? {
                *italic = value;
            }
        }
        LayerKind::Device {
            model,
            frame,
            screenshot,
            shadow,
        } => {
            if let Some(slug) = args.opt_str("model") {
                let next = DeviceModel::from_slug(slug)
                    .ok_or_else(|| format!("No model {slug:?}; design_options lists them."))?;
                if layer.name == model.target().label() {
                    layer.name = next.target().label().into();
                }
                *model = next;
                layer.height = layer.width * next.aspect();
            }
            if let Some(slug) = args.opt_str("frame") {
                *frame = Frame::from_slug(slug).ok_or_else(|| format!("No frame {slug:?}; design_options lists them."))?;
            }
            if let Some(path) = args.opt_str("screenshot") {
                *screenshot = Some(import(path, folder)?.0);
            }
            if let Some(value) = args.opt_bool("shadow")? {
                *shadow = value;
            }
        }
        LayerKind::Image { file, radius, shadow } => {
            if !new && let Some(path) = args.opt_str("image") {
                let (name, (w, h)) = import(path, folder)?;
                *file = Some(name);
                layer.height = layer.width * h / w.max(1.);
            }
            if let Some(value) = args.opt_f32("radius")? {
                *radius = value.max(0.);
            }
            if let Some(value) = args.opt_bool("shadow")? {
                *shadow = value;
            }
        }
        LayerKind::Shape { fill, radius } => {
            if let Some(value) = args.color("fill")? {
                *fill = value;
            }
            if let Some(value) = args.opt_f32("radius")? {
                *radius = value.max(0.);
            }
        }
    }

    // Geometry last: a device's height follows its (possibly new) model.
    if let Some(x) = args.opt_f32("x")? {
        layer.x = x;
    }
    if let Some(y) = args.opt_f32("y")? {
        layer.y = y;
    }
    let aspect = match &layer.kind {
        LayerKind::Device { model, .. } => Some(model.aspect()),
        _ => None,
    };
    if let Some(width) = args.opt_f32("width")? {
        layer.width = width.max(1.);
        if let Some(aspect) = aspect {
            layer.height = layer.width * aspect;
        }
    }
    if let Some(height) = args.opt_f32("height")? {
        match aspect {
            Some(aspect) if args.opt_f32("width")?.is_none() => {
                layer.height = height.max(1.);
                layer.width = layer.height / aspect;
            }
            Some(_) => {}
            None => layer.height = height.max(1.),
        }
    }
    layer.fit_text();
    Ok(notes)
}

fn render_slide(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let doc = Doc::open(data_dir, args)?;
    let index = args.index("slide")?;
    if index >= doc.design.slides.len() {
        return Err(format!(
            "There's no slide {index}; the design has {}.",
            doc.design.slides.len()
        ));
    }
    let scale = args.opt_f32("scale")?.unwrap_or(0.3).clamp(0.1, 0.5);
    let size = doc.target.sizes()[0];
    let png = render_png(&doc.design.slides, index, size, &doc.folder, scale).map_err(|error| error.to_string())?;
    let (w, h) = size.pixels();
    Ok(vec![
        json!({
            "type": "image",
            "data": base64::engine::general_purpose::STANDARD.encode(png),
            "mimeType": "image/png",
        }),
        json!({
            "type": "text",
            "text": format!(
                "Slide {index} of {}, {} at {:.0}% ({} x {}). Layers reaching in from neighbouring slides are included.",
                doc.design.slides.len(),
                size.label(),
                scale * 100.,
                (w as f32 * scale).round(),
                (h as f32 * scale).round()
            ),
        }),
    ])
}

fn export(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let doc = Doc::open(data_dir, args)?;
    let folder = args.opt_str("folder").ok_or("folder is required")?;
    let destination = PathBuf::from(folder);
    std::fs::create_dir_all(&destination).map_err(|error| format!("Couldn't create {folder}: {error}"))?;
    let mut written = Vec::new();
    for size in doc.target.sizes() {
        for index in 0..doc.design.slides.len() {
            let png = render_png(&doc.design.slides, index, *size, &doc.folder, 1.0).map_err(|error| error.to_string())?;
            let path = destination.join(format!("{:02}-{}.png", index + 1, size.slug()));
            std::fs::write(&path, png).map_err(|error| format!("Couldn't write {}: {error}", path.display()))?;
            written.push(path.display().to_string());
        }
    }
    Ok(text(json!({ "written": written })))
}
