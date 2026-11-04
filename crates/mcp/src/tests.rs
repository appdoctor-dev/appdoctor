use std::path::PathBuf;

use appdoctor_screenshots::{Design, DeviceModel, Frame, LayerKind, Target};
use appdoctor_store::{AppRecord, Listing, ListingText, Popularity, Snapshot, Store};
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
fn imports_a_drawing_as_layers() {
    let dir = data_dir("drawing");
    let server = Server::new(dir.clone());
    call_json(&server, "add_slide", json!({ "app_id": APP }));
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300">
        <defs><linearGradient id="g"><stop offset="0" stop-color="#f0a"/><stop offset="1" stop-color="#0af"/></linearGradient></defs>
        <rect x="20" y="20" width="360" height="260" rx="30" fill="#1b1340"/>
        <path d="M60 220 Q200 40 340 220" stroke="url(#g)" stroke-width="18" fill="none"/>
        <text x="200" y="150" font-size="40" font-weight="700" fill="#ffffff" text-anchor="middle">Calm</text>
    </svg>"##;
    let added = call_json(&server, "import_svg", json!({ "app_id": APP, "slide": 0, "svg": svg, "x": 100, "width": 800 }));
    let kinds: Vec<&str> = added["layers"].as_array().unwrap().iter().map(|l| l["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["Shape", "Vector", "Text"]);
    assert_eq!(added["layers"][0]["x"], 140.0);
    let design = call_json(&server, "get_design", json!({ "app_id": APP }));
    let layers = design["slides"][0]["layers"].as_array().unwrap();
    assert!(layers[1]["kind"]["svg"].as_str().unwrap().starts_with("(part of an imported drawing"));
    let saved = Design::load_for(&dir.join("screenshots").join(APP.to_string()), Target::Iphone);
    assert!(matches!(&saved.slides[0].layers[1].kind, LayerKind::Vector { svg } if svg.contains("linearGradient")));
    let (content, failed) = call(&server, "import_svg", json!({ "app_id": APP, "slide": 0, "svg": "<p>hi</p>" }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("SVG"));
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

#[test]
fn reads_keywords_and_listing_and_suggests_edits() {
    let dir = data_dir("listing");
    let store = Store::open(&dir.join("appdoctor.db")).unwrap();
    let sleep = store.add_keyword(APP, "sleep sounds", "us", 10).unwrap();
    store.add_keyword(APP, "light meditation", "us", 10).unwrap();
    store.add_keyword(APP, "meditation", "gb", 10).unwrap();
    store
        .record_snapshot(&Snapshot {
            keyword_id: sleep,
            checked_at: 20,
            rank: Some(14),
            results_count: 200,
            difficulty: Some(61.4),
            source: "itunes-search".into(),
            top_apps: Vec::new(),
            ranked_ids: Vec::new(),
        })
        .unwrap();
    store
        .set_popularity(&Popularity {
            term: "sleep sounds".into(),
            country: "us".into(),
            popularity: 48,
            source: "apple-ads".into(),
            fetched_at: 20,
        })
        .unwrap();
    store
        .replace_listings(
            APP,
            &[Listing {
                locale: "en-US".into(),
                text: ListingText {
                    name: Some("Strobia".into()),
                    subtitle: Some("Light meditation".into()),
                    keywords: Some("light,calm,relax".into()),
                    ..Default::default()
                },
                version: "1.2 (PREPARE_FOR_SUBMISSION)".into(),
                info_editable: true,
                version_editable: true,
                pulled_at: 30,
            }],
        )
        .unwrap();
    let server = Server::new(dir.clone());

    let keywords = call_json(&server, "get_keywords", json!({ "app_id": APP }));
    assert_eq!(keywords["countries"], json!(["us", "gb"]));
    let first = &keywords["keywords"][1];
    assert_eq!(first["keyword"], "sleep sounds");
    assert_eq!(first["rank"], 14);
    assert_eq!(first["popularity"], 48);
    assert_eq!(first["difficulty"], 61.0);

    let listing = call_json(&server, "get_listing", json!({ "app_id": APP }));
    let english = &listing["languages"][0];
    assert_eq!(english["fields"]["subtitle"]["length"], 16);
    assert_eq!(english["tracked_keywords_covered"], json!(["light meditation"]));
    assert_eq!(english["tracked_keywords_missing"], json!(["sleep sounds"]));
    assert_eq!(english["keyword_field_repeats"], json!(["light"]));
    assert_eq!(listing["tracked_countries_without_their_language"][0]["locale"], "en-GB");

    let suggested = call_json(
        &server,
        "suggest_listing",
        json!({ "app_id": APP, "locale": "en-US", "keywords": "sleep, sounds, calm", "subtitle": "Light meditation", "note": "Targets sleep sounds." }),
    );
    assert_eq!(suggested["changes"].as_array().unwrap().len(), 1, "an unchanged subtitle isn't a change");
    assert_eq!(suggested["changes"][0]["after"], "sleep,sounds,calm");
    let draft = &store.drafts(APP).unwrap()[0];
    assert_eq!(draft.text.keywords.as_deref(), Some("sleep,sounds,calm"));
    assert_eq!(draft.text.subtitle, None);

    let uk = call_json(&server, "suggest_listing", json!({ "app_id": APP, "locale": "en-GB", "subtitle": "Calm colour meditation", "note": "For the UK." }));
    assert!(uk["notes"][0].as_str().unwrap().contains("adds the language"));
    let (content, failed) = call(&server, "suggest_listing", json!({ "app_id": APP, "locale": "en-US", "name": "x".repeat(31), "note": "" }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("limit is 30"));
    let (content, failed) = call(&server, "suggest_listing", json!({ "app_id": APP, "locale": "english", "name": "x", "note": "" }));
    assert!(failed && content[0]["text"].as_str().unwrap().contains("isn't an App Store Connect language"));
}

#[test]
fn a_design_can_go_back_to_an_earlier_version() {
    let dir = data_dir("versions");
    let server = Server::new(dir.clone());
    let store = Store::open(&dir.join("appdoctor.db")).unwrap();
    call_json(&server, "add_slide", json!({ "app_id": APP }));
    call_json(&server, "save_version", json!({ "app_id": APP, "name": "One slide" }));
    call_json(&server, "add_slide", json!({ "app_id": APP }));
    call_json(&server, "add_slide", json!({ "app_id": APP }));

    // The agent's edits kept what was there before them.
    let listed = call_json(&server, "list_versions", json!({ "app_id": APP }));
    let versions = listed["versions"].as_array().unwrap();
    assert!(versions.iter().any(|v| v["label"] == appdoctor_store::BEFORE_AGENT), "{listed}");
    let named = versions.iter().find(|v| v["named"] == true).unwrap();
    assert_eq!(named["label"], "One slide");
    assert_eq!(named["slides"], 1);

    let restored = call_json(&server, "restore_version", json!({ "app_id": APP, "version": named["id"] }));
    assert_eq!(restored["slides"], 1);
    assert_eq!(call_json(&server, "get_design", json!({ "app_id": APP }))["slides"].as_array().unwrap().len(), 1);

    // What was there before the restore is still there to go back to.
    let versions = store.design_versions(APP, "iphone").unwrap();
    assert_eq!(versions[0].label, "Before restoring");
    assert_eq!(Design::from_json(&versions[0].design).unwrap().slides.len(), 3);

    let (content, failed) = call(&server, "restore_version", json!({ "app_id": APP, "version": 99999 }));
    assert!(failed);
    assert!(content[0]["text"].as_str().unwrap().contains("list_versions"));
}

#[test]
fn a_listings_versions_can_be_read() {
    let dir = data_dir("listing-versions");
    let server = Server::new(dir.clone());
    let store = Store::open(&dir.join("appdoctor.db")).unwrap();
    let text = ListingText {
        name: Some("Strobia".into()),
        ..ListingText::default()
    };
    store.save_listing_version(APP, "en-US", &text, "From App Store Connect", false, 10).unwrap();
    let listed = call_json(&server, "list_listing_versions", json!({ "app_id": APP, "locale": "en-US" }));
    assert_eq!(listed["versions"].as_array().unwrap().len(), 1);
    assert_eq!(listed["versions"][0]["label"], "From App Store Connect");
    assert_eq!(listed["versions"][0]["fields"]["name"]["text"], "Strobia");
}

#[test]
fn a_design_has_a_version_per_language() {
    let dir = data_dir("languages");
    let server = Server::new(dir.clone());
    let folder = dir.join("screenshots").join(APP.to_string());
    call_json(&server, "add_slide", json!({ "app_id": APP }));
    let added = call_json(
        &server,
        "add_layer",
        json!({ "app_id": APP, "slide": 0, "kind": "text", "text": "Sleep better" }),
    );
    let layer = added["layer"]["id"].clone();

    // A new language starts as a copy, and doesn't touch the default.
    call_json(
        &server,
        "update_layer",
        json!({ "app_id": APP, "locale": "de-DE", "layer_id": layer, "text": "Besser schlafen" }),
    );
    let words = |design: &Design| match &design.slides[0].layers[0].kind {
        LayerKind::Text { text, .. } => text.clone(),
        _ => panic!("expected words"),
    };
    assert_eq!(words(&Design::load_for(&folder, Target::Iphone)), "Sleep better");
    assert_eq!(
        words(&Design::load_language(&folder, Target::Iphone, Some("de-DE"))),
        "Besser schlafen"
    );

    let german = call_json(&server, "get_design", json!({ "app_id": APP, "locale": "de-DE" }));
    assert_eq!(german["locale"], "de-DE");
    assert_eq!(german["languages"], json!(["de-DE"]));

    // Its history is its own.
    call_json(&server, "save_version", json!({ "app_id": APP, "locale": "de-DE", "name": "Erste" }));
    let store = Store::open(&dir.join("appdoctor.db")).unwrap();
    assert!(store.design_versions(APP, "iphone/de-DE").unwrap().iter().any(|v| v.label == "Erste"));
    assert!(!store.design_versions(APP, "iphone").unwrap().iter().any(|v| v.label == "Erste"));

    let (content, failed) = call(&server, "get_design", json!({ "app_id": APP, "locale": "klingon" }));
    assert!(failed);
    assert!(content[0]["text"].as_str().unwrap().contains("de-DE"));
}
