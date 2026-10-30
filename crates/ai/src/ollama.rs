//! Ollama: a model on this Mac. AppDoctor asks it over Ollama's HTTP API
//! (localhost only), runs the tools it calls in-process, and hands back the
//! answers, until it stops calling tools.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use appdoctor_mcp::Server;
use serde_json::{Value, json};

use crate::{Event, Task, shorten};

/// A focused set of tools: small local models choose better among fewer.
const TOOLS: &[&str] = &[
    "list_apps",
    "get_keywords",
    "get_listing",
    "suggest_listing",
    "aso_audit",
    "get_design",
    "list_themes",
    "create_video",
    "apply_theme",
    "set_video",
    "update_slide",
    "update_layer",
    "set_camera",
    "add_callout",
    "add_sound",
    "import_svg",
    "render_storyboard",
    "review_video",
    "simulator_screenshot",
    "record_simulator",
    "export_video",
];
const STEPS: usize = 30;

/// Where Ollama listens: `OLLAMA_HOST`, as Ollama itself reads it, else
/// its default.
fn host() -> String {
    std::env::var("OLLAMA_HOST")
        .ok()
        .map(|host| host.trim_start_matches("http://").trim_end_matches('/').to_owned())
        .filter(|host| !host.is_empty())
        .unwrap_or_else(|| "127.0.0.1:11434".into())
}

/// One HTTP/1.1 request with a JSON body and answer.
fn request(host: &str, method: &str, path: &str, body: Option<&Value>, timeout: Duration) -> Result<Value, String> {
    let address = host
        .to_socket_addrs()
        .map_err(|error| error.to_string())?
        .next()
        .ok_or("Ollama's address didn't resolve")?;
    let mut stream =
        TcpStream::connect_timeout(&address, Duration::from_secs(2)).map_err(|error| format!("Ollama isn't running ({error})"))?;
    stream.set_read_timeout(Some(timeout)).map_err(|error| error.to_string())?;
    let body = body.map(Value::to_string).unwrap_or_default();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|error| error.to_string())?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw).map_err(|error| format!("Ollama didn't answer: {error}"))?;
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or("Ollama's answer was cut off")?;
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let mut content = raw[split + 4..].to_vec();
    if head.contains("transfer-encoding: chunked") {
        content = dechunk(&content);
    }
    if !head.starts_with("http/1.1 2") && !head.starts_with("http/1.0 2") {
        let detail: Value = serde_json::from_slice(&content).unwrap_or(Value::Null);
        return Err(format!(
            "Ollama said: {}",
            detail["error"].as_str().unwrap_or(head.lines().next().unwrap_or_default())
        ));
    }
    serde_json::from_slice(&content).map_err(|error| format!("Ollama's answer wasn't JSON: {error}"))
}

fn dechunk(mut body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    while let Some(line_end) = body.windows(2).position(|w| w == b"\r\n") {
        let size = usize::from_str_radix(String::from_utf8_lossy(&body[..line_end]).trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        let start = line_end + 2;
        out.extend_from_slice(&body[start..(start + size).min(body.len())]);
        body = &body[(start + size + 2).min(body.len())..];
    }
    out
}

/// The models downloaded into Ollama.
pub(crate) fn models() -> Result<Vec<String>, String> {
    let tags = request(&host(), "GET", "/api/tags", None, Duration::from_secs(3))?;
    Ok(tags["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| model["name"].as_str().map(str::to_owned))
        .collect())
}

pub(crate) fn run(task: &Task, on_event: &mut dyn FnMut(Event), cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let model = task.model.clone().ok_or("Pick an Ollama model first.")?;
    chat(&host(), &model, task, on_event, cancel)
}

fn chat(host: &str, model: &str, task: &Task, on_event: &mut dyn FnMut(Event), cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let server = Server::new(task.data_dir.clone());
    let listed = server
        .handle(&json!({ "jsonrpc": "2.0", "id": 0, "method": "tools/list" }).to_string())
        .unwrap_or(Value::Null);
    let tools: Vec<Value> = listed["result"]["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|tool| TOOLS.contains(&tool["name"].as_str().unwrap_or_default()))
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool["name"],
                    "description": tool["description"],
                    "parameters": tool["inputSchema"],
                }
            })
        })
        .collect();
    let mut messages = vec![json!({ "role": "system", "content": task.context })];
    for (asked, answered) in &task.history {
        messages.push(json!({ "role": "user", "content": asked }));
        messages.push(json!({ "role": "assistant", "content": answered }));
    }
    messages.push(json!({ "role": "user", "content": task.prompt }));
    for _ in 0..STEPS {
        if cancel.load(Ordering::Relaxed) {
            on_event(Event::Failed("Stopped.".into()));
            return Ok(());
        }
        let reply = request(
            host,
            "POST",
            "/api/chat",
            Some(&json!({ "model": model, "messages": messages, "tools": tools, "stream": false, "options": { "temperature": 0.4 } })),
            Duration::from_secs(900),
        )?;
        let message = reply["message"].clone();
        let words = message["content"].as_str().unwrap_or_default().trim().to_owned();
        let calls = message["tool_calls"].as_array().cloned().unwrap_or_default();
        messages.push(message);
        if calls.is_empty() {
            on_event(Event::Done(words));
            return Ok(());
        }
        if !words.is_empty() {
            on_event(Event::Text(words));
        }
        for call in calls {
            let name = call["function"]["name"].as_str().unwrap_or_default().to_owned();
            // Some models send the arguments as a JSON string.
            let arguments = match &call["function"]["arguments"] {
                Value::String(text) => serde_json::from_str(text).unwrap_or(json!({})),
                other => other.clone(),
            };
            on_event(Event::Tool {
                name: name.clone(),
                input: arguments.clone(),
            });
            let answer = server
                .handle(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": { "name": name, "arguments": arguments } }).to_string())
                .unwrap_or(Value::Null);
            let result = &answer["result"];
            let text = result["content"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|part| match part["type"].as_str() {
                    Some("text") => part["text"].as_str().unwrap_or_default().to_owned(),
                    Some("image") => "[an image of the result]".to_owned(),
                    _ => String::new(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            let failed = result["isError"].as_bool().unwrap_or(true);
            on_event(Event::Result {
                text: shorten(&text, 200),
                failed,
            });
            messages.push(json!({ "role": "tool", "tool_name": name, "content": text }));
        }
    }
    on_event(Event::Failed(format!("Stopped after {STEPS} steps.")));
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::path::PathBuf;

    use super::*;
    use crate::{Launch, Provider};

    /// A stand-in for Ollama that answers `replies` in turn.
    fn fake_ollama(replies: Vec<Value>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 65536];
                let mut request = Vec::new();
                // Read the headers and the announced body.
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    request.extend_from_slice(&buffer[..read]);
                    if let Some(split) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&request[..split]).to_ascii_lowercase();
                        let length: usize = head
                            .lines()
                            .find_map(|l| l.strip_prefix("content-length: "))
                            .and_then(|n| n.trim().parse().ok())
                            .unwrap_or(0);
                        if request.len() >= split + 4 + length {
                            break;
                        }
                    }
                    if read == 0 {
                        break;
                    }
                }
                let body = reply.to_string();
                // Chunked, as Ollama sends.
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                    body.len()
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });
        host
    }

    #[test]
    fn runs_tools_until_the_model_is_done() {
        let host = fake_ollama(vec![
            json!({ "message": { "role": "assistant", "content": "", "tool_calls": [
                { "function": { "name": "list_apps", "arguments": {} } }
            ] } }),
            json!({ "message": { "role": "assistant", "content": "There are no apps yet." } }),
        ]);
        let task = Task {
            provider: Provider::Ollama,
            path: PathBuf::from("ollama"),
            model: Some("qwen3".into()),
            effort: None,
            prompt: "What apps are there?".into(),
            context: "Test.".into(),
            launch: Launch {
                command: "appdoctor".into(),
                args: vec!["mcp".into()],
                env: Vec::new(),
            },
            data_dir: std::env::temp_dir().join(format!("appdoctor-ai-test-{}", std::process::id())),
            project: None,
            resume: None,
            history: Vec::new(),
        };
        let mut events = Vec::new();
        chat(&host, "qwen3", &task, &mut |event| events.push(event), &Arc::new(AtomicBool::new(false))).unwrap();
        assert!(matches!(&events[0], Event::Tool { name, .. } if name == "list_apps"));
        assert!(matches!(&events[1], Event::Result { failed: false, text } if text.contains("No apps")));
        assert_eq!(events.last(), Some(&Event::Done("There are no apps yet.".into())));
        let _ = std::fs::remove_dir_all(task.data_dir);
    }
}
