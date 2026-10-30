//! AppDoctor's local MCP server, so the user's own AI agent (Claude, Cursor,
//! anything that speaks the Model Context Protocol) can drive AppDoctor.
//!
//! It runs as `appdoctor mcp` and speaks JSON-RPC over stdin and stdout, one
//! message per line. It opens no port and talks to nothing but the files in
//! AppDoctor's data folder. Today it designs App Store screenshots, in the
//! same design files the editor shows; the editor watches them, so each
//! change appears there as it lands and can be undone.

pub mod clients;
mod design;

use std::io::{BufRead, Write};
use std::path::PathBuf;

use serde_json::{Value, json};

/// Newest first; an older client gets its own version back.
const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "AppDoctor designs App Store art on the user's computer. \
Each app has four designs: iPhone screenshots (canvas 1320 x 2868), iPad screenshots (2064 x 2752), the \
product page header (21:9, 3840 x 1646; keep words inside the middle, about 18% in from the sides and 15% \
from top and bottom) and the search result art (3:2, 3840 x 2560). A design is a row of slides, each a \
background plus layers (text, device, image, shape) positioned in canvas pixels from the top-left. Device \
layers can be any iPhone or iPad from iPhone X to iPhone 18 Pro Max and the iPhone Duo. Screenshot slides \
sit edge to edge, so a layer whose x runs past the canvas width continues onto the next slide, which makes \
panoramas; header and search slides are alternative versions. Start with list_apps and get_design, use design_options and list_fonts for \
allowed values, and look at your work with render_slide before you say it's done. The user sees every \
change live in AppDoctor and can undo it.";

pub struct Server {
    data_dir: PathBuf,
}

impl Server {
    /// A server over AppDoctor's data folder (the one the app uses).
    pub fn new(data_dir: PathBuf) -> Server {
        Server { data_dir }
    }

    /// Answers requests from `input` on `output` until `input` closes.
    pub fn serve(&self, input: impl BufRead, mut output: impl Write) -> std::io::Result<()> {
        for line in input.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(reply) = self.handle(&line) {
                serde_json::to_writer(&mut output, &reply)?;
                output.write_all(b"\n")?;
                output.flush()?;
            }
        }
        Ok(())
    }

    /// The reply to one message, or `None` for notifications.
    pub fn handle(&self, line: &str) -> Option<Value> {
        let message: Value = match serde_json::from_str(line) {
            Ok(message) => message,
            Err(error) => return Some(failure(Value::Null, -32700, format!("not JSON: {error}"))),
        };
        // Notifications (and responses to us) carry no id and get no reply.
        let id = message.get("id").cloned()?;
        let method = message.get("method").and_then(Value::as_str)?;
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        Some(match method {
            "initialize" => success(id, self.initialize(&params)),
            "ping" => success(id, json!({})),
            "tools/list" => success(id, json!({ "tools": design::tools() })),
            "tools/call" => success(id, self.call(&params)),
            _ => failure(id, -32601, format!("unknown method {method}")),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = requested
            .filter(|version| PROTOCOL_VERSIONS.contains(version))
            .unwrap_or(PROTOCOL_VERSIONS[0]);
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": {
                "name": "appdoctor",
                "title": "AppDoctor",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "instructions": INSTRUCTIONS,
        })
    }

    /// Tool failures are results with `isError`, so the agent reads why.
    fn call(&self, params: &Value) -> Value {
        let name = params.get("name").and_then(Value::as_str).unwrap_or_default();
        let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
        match design::call(&self.data_dir, name, &arguments) {
            Ok(content) => json!({ "content": content, "isError": false }),
            Err(message) => json!({
                "content": [{ "type": "text", "text": message }],
                "isError": true,
            }),
        }
    }
}

fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn failure(id: Value, code: i64, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

#[cfg(test)]
mod tests;
