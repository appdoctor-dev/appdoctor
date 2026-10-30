//! The iOS Simulator, for agents that can't run a terminal (Claude
//! Desktop, say): which simulators are running, a screenshot to look at, a
//! screen recording for a device layer, and deep links to reach a screen.
//! Everything goes through `xcrun simctl` on this Mac; files land in
//! AppDoctor's data folder under `recordings`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use appdoctor_screenshots::tiny_skia::{Pixmap, PixmapPaint, Transform};
use base64::Engine as _;
use serde_json::{Value, json};

use crate::design::{Args, Result, text, tool};

pub(crate) fn tools() -> Vec<Value> {
    let device = json!({ "device": { "type": "string", "description": "A simulator's UDID or name; the booted one by default." } });
    let with_device = |extra: Value| {
        let mut properties = device.clone();
        if let (Some(base), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
            base.extend(extra.clone());
        }
        properties
    };
    vec![
        tool(
            "simulator_devices",
            "List running simulators",
            "The iOS Simulators that are booted, with their names and UDIDs. Boot one in Xcode or with `xcrun simctl boot` first.",
            json!({}),
            &[],
            true,
        ),
        tool(
            "simulator_screenshot",
            "Screenshot the simulator",
            "Saves what the simulator shows and returns it to look at, plus its path for a device layer's screenshot.",
            with_device(json!({})),
            &[],
            false,
        ),
        tool(
            "record_simulator",
            "Record the simulator",
            "Records the simulator's screen for some seconds while the app runs (start the app and anything that should happen first, for example with simulator_open_url) and returns the recording's path for a device layer's screenshot.",
            with_device(json!({ "seconds": { "type": "number", "minimum": 1, "maximum": 60 } })),
            &["seconds"],
            false,
        ),
        tool(
            "simulator_open_url",
            "Open a link in the simulator",
            "Opens a URL or the app's deep link in the simulator, to reach the screen to show.",
            with_device(json!({ "url": { "type": "string" } })),
            &["url"],
            false,
        ),
    ]
}

pub(crate) fn call(data_dir: &Path, name: &str, args: &Args) -> Option<Result<Vec<Value>>> {
    Some(match name {
        "simulator_devices" => booted().map(|devices| text(json!({ "booted": devices }))),
        "simulator_screenshot" => screenshot(data_dir, args),
        "record_simulator" => record(data_dir, args),
        "simulator_open_url" => open_url(args),
        _ => return None,
    })
}

fn simctl(args: &[&str]) -> Result<String> {
    let output = Command::new("xcrun")
        .arg("simctl")
        .args(args)
        .output()
        .map_err(|error| format!("Couldn't run xcrun simctl ({error}). Is Xcode installed?"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!("simctl: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

fn booted() -> Result<Vec<Value>> {
    let listing: Value = serde_json::from_str(&simctl(&["list", "devices", "booted", "--json"])?)
        .map_err(|error| format!("Couldn't read simctl's list: {error}"))?;
    Ok(listing["devices"]
        .as_object()
        .into_iter()
        .flat_map(|runtimes| runtimes.iter())
        .flat_map(|(runtime, devices)| {
            devices.as_array().into_iter().flatten().map(move |device| {
                json!({
                    "name": device["name"],
                    "udid": device["udid"],
                    "runtime": runtime.rsplit('.').next().unwrap_or(runtime),
                })
            })
        })
        .collect())
}

/// The simulator to use: the one asked for, else the booted one.
fn device(args: &Args) -> Result<String> {
    if let Some(device) = args.opt_str("device") {
        return Ok(device.to_owned());
    }
    match booted()?.len() {
        0 => Err("No simulator is running. Boot one in Xcode (or `xcrun simctl boot <name>`) and run the app.".into()),
        _ => Ok("booted".into()),
    }
}

fn recordings(data_dir: &Path) -> Result<PathBuf> {
    let folder = data_dir.join("recordings");
    std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
    Ok(folder)
}

fn stamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
        .to_string()
}

fn screenshot(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let device = device(args)?;
    let path = recordings(data_dir)?.join(format!("screen-{}.png", stamp()));
    simctl(&["io", &device, "screenshot", &path.to_string_lossy()])?;
    let mut content = Vec::new();
    // A small copy to look at; the full one is for the design.
    if let Ok(full) = Pixmap::load_png(&path) {
        let scale = (360. / full.width() as f32).min(1.);
        if let Some(mut small) = Pixmap::new(
            (full.width() as f32 * scale).round() as u32,
            (full.height() as f32 * scale).round() as u32,
        ) {
            small.draw_pixmap(0, 0, full.as_ref(), &PixmapPaint::default(), Transform::from_scale(scale, scale), None);
            if let Ok(png) = small.encode_png() {
                content.push(json!({
                    "type": "image",
                    "data": base64::engine::general_purpose::STANDARD.encode(png),
                    "mimeType": "image/png",
                }));
            }
        }
    }
    content.extend(text(json!({ "path": path.display().to_string() })));
    Ok(content)
}

fn record(data_dir: &Path, args: &Args) -> Result<Vec<Value>> {
    let seconds = args.opt_f32("seconds")?.ok_or("seconds is required")?.clamp(1., 60.);
    let device = device(args)?;
    let path = recordings(data_dir)?.join(format!("recording-{}.mov", stamp()));
    let mut child = Command::new("xcrun")
        .args(["simctl", "io", &device, "recordVideo", "--codec", "h264", "--force"])
        .arg(&path)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| format!("Couldn't start recording ({error}). Is Xcode installed?"))?;
    std::thread::sleep(Duration::from_secs_f32(seconds));
    // simctl finishes the file on an interrupt, like Ctrl-C.
    let _ = Command::new("kill").args(["-INT", &child.id().to_string()]).status();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(100)),
            _ => {
                let _ = child.kill();
                break;
            }
        }
    }
    if !path.is_file() {
        let mut error = String::new();
        if let Some(mut stderr) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut stderr, &mut error);
        }
        return Err(format!("The recording didn't save. {}", error.trim()));
    }
    #[cfg(target_os = "macos")]
    let length = appdoctor_video::clip_duration(&path);
    #[cfg(not(target_os = "macos"))]
    let length: Option<f32> = None;
    Ok(text(json!({
        "path": path.display().to_string(),
        "seconds": length,
        "next": "Use the path as a device layer's screenshot (update_layer, or screen in create_video).",
    })))
}

fn open_url(args: &Args) -> Result<Vec<Value>> {
    let url = args.opt_str("url").ok_or("url is required")?;
    let device = device(args)?;
    simctl(&["openurl", &device, url])?;
    Ok(text(json!({ "opened": url })))
}
