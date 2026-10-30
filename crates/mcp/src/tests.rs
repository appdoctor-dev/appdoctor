use std::path::PathBuf;

use appdoctor_screenshots::{Design, DeviceModel, Frame, LayerKind, Target};
use appdoctor_store::{AppRecord, Store};
use serde_json::{Value, json};

use super::Server;

const APP: u64 = 6762288498;

fn data_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("appdoctor-mcp-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::open(&dir.join("appdoctor.db")).unwrap();
    store
        .upsert_app(&AppRecord {
            id: APP,
            name: "Strobia".into(),
            bundle_id: None,
            developer: None,
            icon_url: None,
            country: "us".into(),
            rating_count: 0,
            average_rating: 0.,
            added_at: 0,
            updated_at: 0,
        })
        .unwrap();
    dir
}

fn request(server: &Server, id: u64, method: &str, params: Value) -> Value {
    let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
    server.handle(&line).expect("a reply")
}

/// Calls a tool; returns its content and whether it failed.
fn call(server: &Server, name: &str, arguments: Value) -> (Vec<Value>, bool) {
    let reply = request(server, 1, "tools/call", json!({ "name": name, "arguments": arguments }));
    let result = &reply["result"];
    (
        result["content"].as_array().cloned().unwrap_or_default(),
        result["isError"].as_bool().unwrap_or(true),
    )
}

fn call_json(server: &Server, name: &str, arguments: Value) -> Value {
    let (content, failed) = call(server, name, arguments);
    assert!(!failed, "{name} failed: {content:?}");
    serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn speaks_the_protocol() {
    let server = Server::new(data_dir("protocol"));
    let init = request(&server, 1, "initialize", json!({ "protocolVersion": "2025-03-26" }));
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert!(init["result"]["capabilities"]["tools"].is_object());
    let newest = request(&server, 2, "initialize", json!({ "protocolVersion": "2099-01-01" }));
    assert_eq!(newest["result"]["protocolVersion"], "2025-06-18");

    assert!(server.handle(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    assert_eq!(request(&server, 3, "ping", json!({}))["result"], json!({}));
    assert_eq!(request(&server, 4, "nope", json!({}))["error"]["code"], -32601);
    assert_eq!(server.handle("{not json").unwrap()["error"]["code"], -32700);

    let tools = request(&server, 5, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    for expected in ["list_apps", "get_design", "add_layer", "update_layer", "render_slide", "export_screenshots"] {
        assert!(names.contains(&expected), "{expected} missing");
    }
    for tool in tools["result"]["tools"].as_array().unwrap() {
        assert_eq!(tool["inputSchema"]["type"], "object");
    }
}

#[test]
fn serves_lines_until_input_closes() {
    let server = Server::new(data_dir("serve"));
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\n\n",
        r#"{"jsonrpc":"2.0","id":"two","method":"tools/call","params":{"name":"list_apps"}}"#,
        "\n"
    );
    let mut output = Vec::new();
    server.serve(input.as_bytes(), &mut output).unwrap();
    let replies: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[1]["id"], "two");
    assert!(replies[1]["result"]["content"][0]["text"].as_str().unwrap().contains("Strobia"));
}

#[test]
fn designs_a_slide_the_editor_can_open() {
    let dir = data_dir("design");
    let server = Server::new(dir.clone());

    let added = call_json(&server, "add_slide", json!({ "app_id": APP, "starter": true, "headline": "Sleep better" }));
    assert_eq!(added["slide"], 0);
    let background = call_json(&server, "set_background", json!({ "app_id": APP, "slide": 0, "preset": "Sunset" }));
    assert_eq!(background["background"]["from"], "#f97316");

    let text = call_json(
        &server,
        "add_layer",
        json!({ "app_id": APP.to_string(), "slide": 0, "kind": "text", "text": "Fall asleep faster",
                "font": "Definitely Not A Font", "italic": true, "size": 80, "align": "left" }),
    );
    let text_id = text["layer"]["id"].as_u64().unwrap();
    assert!(text["notes"][0].as_str().unwrap().contains("isn't installed"));

    let design = call_json(&server, "get_design", json!({ "app_id": APP }));
    let device_id = design["slides"][0]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|layer| layer["kind"]["type"] == "device")
        .unwrap()["id"]
        .as_u64()
        .unwrap();
    call_json(
        &server,
        "update_layer",
        json!({ "app_id": APP, "layer_id": device_id, "model": "iphone-16e", "frame": "orange", "width": 900 }),
    );
    call_json(&server, "arrange_layer", json!({ "app_id": APP, "layer_id": text_id, "to": "back" }));

    // What the editor reads from disk.
    let saved = Design::load_for(&dir.join("screenshots").join(APP.to_string()), Target::Iphone);
    let slide = &saved.slides[0];
    assert_eq!(slide.layers[0].id, text_id);
    assert!(matches!(
        &slide.layers[0].kind,
        LayerKind::Text { italic: true, font: Some(font), .. } if font == "Definitely Not A Font"
    ));
    let device = slide.layers.iter().find(|layer| layer.id == device_id).unwrap();
    let sixteen_e = DeviceModel::from_slug("iphone-16e").unwrap();
    assert!(matches!(
        device.kind,
        LayerKind::Device { model, frame: Frame::Orange, .. } if model == sixteen_e
    ));
    assert_eq!(device.width, 900.);
    assert!((device.height - 900. * sixteen_e.aspect()).abs() < 0.01);

    let (content, failed) = call(&server, "render_slide", json!({ "app_id": APP, "slide": 0, "scale": 0.1 }));
    assert!(!failed);
    assert_eq!(content[0]["type"], "image");
    assert!(!content[0]["data"].as_str().unwrap().is_empty());

    let out = dir.join("export");
    let exported = call_json(
        &server,
        "export_screenshots",
        json!({ "app_id": APP, "folder": out.display().to_string() }),
    );
    assert_eq!(exported["written"].as_array().unwrap().len(), 2);
}

#[test]
fn ipad_designs_are_separate() {
    let dir = data_dir("ipad");
    let server = Server::new(dir.clone());
    call_json(&server, "add_slide", json!({ "app_id": APP, "device": "ipad", "starter": true }));
    let design = call_json(&server, "get_design", json!({ "app_id": APP, "device": "ipad" }));
    assert_eq!(design["canvas"]["width"], 2064.0);
    assert_eq!(call_json(&server, "get_design", json!({ "app_id": APP }))["slides"], json!([]));
    let apps = call_json(&server, "list_apps", json!({}));
    assert_eq!(apps["apps"][0]["ipad_slides"], 1);
    assert_eq!(apps["apps"][0]["iphone_slides"], 0);
}

#[test]
fn designs_header_and_search_art() {
    let dir = data_dir("header");
    let server = Server::new(dir.clone());
    let added = call_json(&server, "add_slide", json!({ "app_id": APP, "device": "header", "starter": true }));
    assert_eq!(added["layers"].as_array().unwrap().len(), 3);
    let design = call_json(&server, "get_design", json!({ "app_id": APP, "device": "header" }));
    assert_eq!(design["canvas"]["width"], 3840.0);
    assert_eq!(design["canvas"]["height"], 1646.0);
    let (content, failed) = call(&server, "render_slide", json!({ "app_id": APP, "device": "search", "slide": 0 }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("no slide"));
    let options = call_json(&server, "design_options", json!({}));
    assert!(options["models"].as_array().unwrap().len() > 30);
    assert_eq!(options["designs"].as_array().unwrap().len(), 4);
    assert!(dir.join("screenshots").join(APP.to_string()).join("design-header.json").exists());
}

#[test]
fn explains_mistakes() {
    let server = Server::new(data_dir("mistakes"));
    let (content, failed) = call(&server, "get_design", json!({ "app_id": 42 }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("list_apps"));
    let (_, failed) = call(&server, "add_layer", json!({ "app_id": APP, "slide": 0, "kind": "text" }));
    assert!(failed, "no slide 0 yet");
    call_json(&server, "add_slide", json!({ "app_id": APP }));
    let (content, failed) = call(&server, "add_layer", json!({ "app_id": APP, "slide": 0, "kind": "text" }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("need text"));
    let (_, failed) = call(
        &server,
        "add_layer",
        json!({ "app_id": APP, "slide": 0, "kind": "shape", "fill": "red\"/><script>" }),
    );
    assert!(failed, "colours are checked");
    let (_, failed) = call(&server, "no_such_tool", json!({}));
    assert!(failed);
}

#[cfg(target_os = "macos")]
#[test]
fn makes_a_video_with_a_screen_recording() {
    use appdoctor_screenshots::tiny_skia::{Color, Pixmap};

    let dir = data_dir("video");
    let server = Server::new(dir.clone());
    // A one-second "screen recording": solid green.
    let clip = dir.join("screen.mp4");
    let mut encoder = appdoctor_video::Encoder::create(&clip, 200, 432, 30, 1_000_000).unwrap();
    for index in 0..30 {
        let mut frame = Pixmap::new(200, 432).unwrap();
        frame.fill(Color::from_rgba8(0, 200, 0, 255));
        encoder.push(&frame, index).unwrap();
    }
    encoder.finish().unwrap();

    call_json(&server, "add_slide", json!({ "app_id": APP, "device": "search", "starter": true }));
    call_json(&server, "add_slide", json!({ "app_id": APP, "device": "search", "starter": true }));
    let video = call_json(&server, "set_video", json!({ "app_id": APP, "device": "search", "enabled": true, "fps": 10 }));
    assert!(video["length_problem"].is_null(), "6 s is fine for search");
    call_json(&server, "update_slide", json!({ "app_id": APP, "device": "search", "slide": 1, "duration": 2 }));
    let design = call_json(&server, "get_design", json!({ "app_id": APP, "device": "search" }));
    assert_eq!(design["video"]["length"], 5.0);
    let layers = design["slides"][0]["layers"].as_array().unwrap();
    let title = layers[0]["id"].as_u64().unwrap();
    let device = layers[1]["id"].as_u64().unwrap();
    call_json(
        &server,
        "update_layer",
        json!({ "app_id": APP, "device": "search", "layer_id": title, "enter": "rise", "comes_in_at": 0.5, "drift": "zoom-in" }),
    );
    call_json(
        &server,
        "update_layer",
        json!({ "app_id": APP, "device": "search", "layer_id": device, "screenshot": clip.display().to_string(), "enter": "slide" }),
    );

    let (content, failed) = call(&server, "render_frame", json!({ "app_id": APP, "device": "search", "time": 2.8, "scale": 0.1 }));
    assert!(!failed, "{content:?}");
    assert_eq!(content[0]["type"], "image");
    assert!(content[1]["text"].as_str().unwrap().contains("transition"));

    let exported = call_json(
        &server,
        "export_video",
        json!({ "app_id": APP, "device": "search", "folder": dir.join("out").display().to_string() }),
    );
    let written = exported["written"].as_str().unwrap();
    assert!(written.ends_with("search-3x2-small.mp4"));
    let length = appdoctor_video::clip_duration(std::path::Path::new(written)).unwrap();
    assert!((length - 5.).abs() < 0.2, "{length}");
}

#[test]
fn storyboards_themes_cameras_callouts_and_reviews() {
    let dir = data_dir("creative");
    let server = Server::new(dir.clone());
    // A plain screenshot to put in the devices.
    let shot = dir.join("shot.png");
    let mut image = appdoctor_screenshots::tiny_skia::Pixmap::new(300, 650).unwrap();
    image.fill(appdoctor_screenshots::tiny_skia::Color::from_rgba8(240, 240, 245, 255));
    image.save_png(&shot).unwrap();
    let shot = shot.display().to_string();

    let made = call_json(
        &server,
        "create_video",
        json!({
            "app_id": APP,
            "theme": "sunset",
            "scenes": [
                { "headline": "Sleep *better* tonight", "layout": "text-only" },
                { "headline": "Guided calm", "subtitle": "Two minutes a night", "screen": shot,
                  "zoom": [{ "at": 1, "x": 0.5, "y": 0.8 }],
                  "callouts": [{ "kind": "tap", "at": 1.2, "x": 0.5, "y": 0.8 }, { "kind": "badge", "at": 0.5, "x": 0.8, "y": 0.1, "text": "New" }] },
                { "headline": "Track your nights", "screen": shot, "layout": "device-top" },
                { "headline": "Wake up refreshed", "screen": shot, "layout": "full-screen", "duration": 4 }
            ]
        }),
    );
    assert_eq!(made["scenes"], 4);
    let design = call_json(&server, "get_design", json!({ "app_id": APP }));
    assert!(design["video"].is_object());
    let slides = design["slides"].as_array().unwrap();
    assert_eq!(slides[1]["camera"].as_array().unwrap().len(), 1);
    let kinds: Vec<&str> = slides[1]["layers"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"Tap") && kinds.contains(&"Badge"), "{kinds:?}");
    // The theme coloured the words and their accent.
    let headline = slides[0]["layers"].as_array().unwrap().iter().find(|l| l["kind"]["type"] == "text").unwrap();
    assert_eq!(headline["kind"]["accent"], "#fde047");
    assert_eq!(headline["motion"]["enter"], "words");

    call_json(&server, "apply_theme", json!({ "app_id": APP, "theme": "paper" }));
    let device_id = slides[2]["layers"].as_array().unwrap().iter().find(|l| l["kind"]["type"] == "device").unwrap()["id"]
        .as_u64()
        .unwrap();
    call_json(
        &server,
        "set_camera",
        json!({ "app_id": APP, "slide": 2, "device_id": device_id, "zooms": [{ "at": 0.5, "x": 0.3, "y": 0.4, "zoom": 2.5 }] }),
    );
    call_json(
        &server,
        "add_callout",
        json!({ "app_id": APP, "slide": 2, "device_id": device_id, "kind": "ring", "at": 0.8, "x": 0.5, "y": 0.5 }),
    );

    let (content, failed) = call(&server, "render_storyboard", json!({ "app_id": APP, "every": 1.0 }));
    assert!(!failed, "{content:?}");
    assert_eq!(content[0]["type"], "image");
    if let Ok(out) = std::env::var("APPDOCTOR_STORYBOARD_OUT") {
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD.decode(content[0]["data"].as_str().unwrap()).unwrap();
        std::fs::write(out, png).unwrap();
    }

    let review = call_json(&server, "review_video", json!({ "app_id": APP }));
    let problems: Vec<String> = review["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["problem"].as_str().unwrap().to_owned())
        .collect();
    // 13.5 s is too short for an app preview.
    assert!(problems.iter().any(|p| p.contains("short")), "{problems:?}");

    let tools = request(&server, 9, "tools/list", json!({}));
    let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in ["create_video", "render_storyboard", "review_video", "record_simulator", "simulator_open_url"] {
        assert!(names.contains(&expected), "{expected}");
    }
}
