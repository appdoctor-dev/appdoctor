//! AI command-line tools run headless, each with AppDoctor's MCP server
//! attached for the run and allowed only AppDoctor's tools (plus reading
//! the project, when there is one).

use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use crate::{Event, Provider, Task, shorten};

/// The MCP server, as the command-line tools' settings describe one.
fn server(task: &Task) -> Value {
    let mut server = json!({ "command": task.launch.command, "args": task.launch.args });
    if !task.launch.env.is_empty() {
        server["env"] = Value::Object(
            task.launch
                .env
                .iter()
                .map(|(key, value)| (key.clone(), json!(value)))
                .collect(),
        );
    }
    server
}

/// A private folder for this run's settings files.
fn scratch() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "appdoctor-ai-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis())
    ));
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

/// A TOML string for `codex -c`.
fn toml(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(crate) fn run(task: &Task, on_event: &mut dyn FnMut(Event), cancel: &Arc<AtomicBool>) -> Result<(), String> {
    let scratch = scratch()?;
    let mut command = Command::new(&task.path);
    let full_prompt = format!("{}\n\n{}", task.context, task.prompt);
    match task.provider {
        Provider::ClaudeCode => {
            let config = scratch.join("mcp.json");
            std::fs::write(&config, json!({ "mcpServers": { "appdoctor": server(task) } }).to_string())
                .map_err(|error| error.to_string())?;
            let allowed = if task.project.is_some() {
                "mcp__appdoctor,Read,Glob,Grep"
            } else {
                "mcp__appdoctor"
            };
            command
                .arg("-p")
                .args(["--output-format", "stream-json", "--verbose"])
                .arg("--mcp-config")
                .arg(&config)
                .arg("--strict-mcp-config")
                .args(["--allowedTools", allowed])
                .args(["--append-system-prompt", &task.context]);
            if let Some(model) = &task.model {
                command.args(["--model", model]);
            }
            command.arg(&task.prompt);
        }
        Provider::Codex => {
            command.args(["exec", "--json", "--skip-git-repo-check"]);
            command.args(["-c", &format!("mcp_servers.appdoctor.command={}", toml(&task.launch.command))]);
            let args: Vec<String> = task.launch.args.iter().map(|arg| toml(arg)).collect();
            command.args(["-c", &format!("mcp_servers.appdoctor.args=[{}]", args.join(","))]);
            for (key, value) in &task.launch.env {
                command.args(["-c", &format!("mcp_servers.appdoctor.env.{key}={}", toml(value))]);
            }
            if let Some(model) = &task.model {
                command.args(["-m", model]);
            }
            if let Some(project) = &task.project {
                command.arg("-C").arg(project);
            }
            command.arg(&full_prompt);
        }
        Provider::Gemini => {
            // Gemini reads MCP servers from the working folder's settings;
            // trusting AppDoctor's lets its tools run without asking, while
            // anything else still needs approval (so is refused headless).
            let settings = scratch.join(".gemini");
            std::fs::create_dir_all(&settings).map_err(|error| error.to_string())?;
            let mut entry = server(task);
            entry["trust"] = json!(true);
            std::fs::write(
                settings.join("settings.json"),
                json!({ "mcpServers": { "appdoctor": entry } }).to_string(),
            )
            .map_err(|error| error.to_string())?;
            command
                .args(["-o", "stream-json", "--allowed-mcp-server-names", "appdoctor"])
                .args(["-p", &full_prompt]);
            if let Some(model) = &task.model {
                command.args(["-m", model]);
            }
            if let Some(project) = &task.project {
                command.arg("--include-directories").arg(project);
            }
        }
        Provider::Ollama => unreachable!("Ollama runs over HTTP"),
    }
    let cwd = match (task.provider, &task.project) {
        (Provider::Gemini, _) => scratch.clone(),
        (_, Some(project)) => project.clone(),
        (_, None) => scratch.clone(),
    };
    let mut child = command
        .current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Couldn't start {}: {error}", task.provider.label()))?;

    // Stopping kills the process; reading its output then ends.
    let pid = child.id();
    let finished = Arc::new(AtomicBool::new(false));
    let watcher = {
        let (cancel, finished) = (cancel.clone(), finished.clone());
        std::thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    let _ = Command::new("kill").arg(pid.to_string()).status();
                    return;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        })
    };
    let mut stderr = child.stderr.take();
    let mut done = false;
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            for event in parse(task.provider, &line) {
                done |= matches!(event, Event::Done(_));
                on_event(event);
            }
        }
    }
    let status = child.wait().map_err(|error| error.to_string())?;
    finished.store(true, Ordering::Relaxed);
    let _ = watcher.join();
    let _ = std::fs::remove_dir_all(&scratch);
    if cancel.load(Ordering::Relaxed) {
        on_event(Event::Failed("Stopped.".into()));
    } else if !status.success() && !done {
        let mut error = String::new();
        if let Some(stderr) = stderr.as_mut() {
            let _ = stderr.read_to_string(&mut error);
        }
        return Err(format!("{} stopped: {}", task.provider.label(), reason(&error)));
    } else if !done {
        on_event(Event::Done(String::new()));
    }
    Ok(())
}

/// The line of a tool's error output that says what went wrong: the first
/// one naming an error (warnings come first in some tools), else the last.
fn reason(stderr: &str) -> String {
    let lines: Vec<&str> = stderr.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    lines
        .iter()
        .find(|line| line.contains("Error") || line.contains("error:"))
        .or(lines.last())
        .map_or_else(|| "no details".to_owned(), |line| shorten(line, 300))
}

/// AppDoctor's tools appear as `mcp__appdoctor__name` and the like.
fn tool_name(name: &str) -> String {
    name.rsplit("__").next().unwrap_or(name).trim_start_matches("appdoctor.").to_owned()
}

/// Events in one line of a provider's JSON output.
pub(crate) fn parse(provider: Provider, line: &str) -> Vec<Event> {
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        let line = line.trim();
        return if line.is_empty() { Vec::new() } else { vec![Event::Text(line.to_owned())] };
    };
    match provider {
        Provider::ClaudeCode => claude(&message),
        _ => generic(&message),
    }
}

/// Claude Code's stream-json: assistant turns with text and tool_use
/// blocks, user turns with tool_result blocks, and a final result.
fn claude(message: &Value) -> Vec<Event> {
    let blocks = || message["message"]["content"].as_array().cloned().unwrap_or_default();
    match message["type"].as_str() {
        Some("assistant") => blocks()
            .iter()
            .filter_map(|block| match block["type"].as_str() {
                Some("text") => block["text"].as_str().map(|text| Event::Text(text.to_owned())),
                // Claude Code's own tool loader isn't the user's business.
                Some("tool_use") if block["name"] == "ToolSearch" => None,
                Some("tool_use") => Some(Event::Tool {
                    name: tool_name(block["name"].as_str().unwrap_or("tool")),
                    input: shorten(&block["input"].to_string(), 160),
                }),
                _ => None,
            })
            .collect(),
        Some("user") => blocks()
            .iter()
            .filter(|block| block["type"] == "tool_result")
            // The loader's answers carry no text.
            .filter(|block| block["content"] != json!("") && block["content"] != json!([]))
            .map(|block| {
                let text = match &block["content"] {
                    Value::String(text) => text.clone(),
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(|part| part["text"].as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => other.to_string(),
                };
                Event::Result {
                    text: shorten(&text, 200),
                    failed: block["is_error"].as_bool().unwrap_or(false),
                }
            })
            .collect(),
        Some("result") => {
            let text = message["result"].as_str().unwrap_or_default().to_owned();
            if message["is_error"].as_bool().unwrap_or(false) {
                vec![Event::Failed(if text.is_empty() { "Claude Code stopped with an error.".into() } else { text })]
            } else {
                vec![Event::Done(text)]
            }
        }
        _ => Vec::new(),
    }
}

/// Codex and Gemini: their JSON events differ by version, so read what
/// they share: a tool's name, a message's text, the end of a turn.
fn generic(message: &Value) -> Vec<Event> {
    let item = if message["item"].is_object() { &message["item"] } else { message };
    let kind = item["type"].as_str().or(message["type"].as_str()).unwrap_or_default();
    let tool = item["tool"]
        .as_str()
        .or(item["tool_name"].as_str())
        .or(item["name"].as_str().filter(|_| kind.contains("tool")));
    if kind.contains("tool")
        && !kind.contains("result")
        && message["type"] != "item.completed"
        && let Some(name) = tool
    {
        let input = item["arguments"]
            .as_object()
            .map(|_| item["arguments"].to_string())
            .or_else(|| item["parameters"].as_object().map(|_| item["parameters"].to_string()))
            .unwrap_or_default();
        return vec![Event::Tool {
            name: tool_name(name),
            input: shorten(&input, 160),
        }];
    }
    if kind.contains("result") || (kind.contains("tool") && message["type"] == "item.completed") {
        // Codex wraps MCP answers: {"content": [{"type": "text", "text": ...}]}.
        let wrapped = item["result"]["content"].as_array().map(|parts| {
            parts
                .iter()
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join(" ")
        });
        let text = item["output"]
            .as_str()
            .or(item["result"].as_str())
            .map(str::to_owned)
            .or(wrapped)
            .unwrap_or_else(|| item["result"].to_string());
        let failed = item["status"].as_str().is_some_and(|s| s.contains("error") || s.contains("fail"));
        if kind == "result" {
            return vec![Event::Done(message["response"].as_str().or(item["result"].as_str()).unwrap_or_default().to_owned())];
        }
        return vec![Event::Result {
            text: shorten(&text, 200),
            failed,
        }];
    }
    if kind.contains("turn.completed") {
        return vec![Event::Done(String::new())];
    }
    let text = item["text"].as_str().or(item["content"].as_str()).or(message["delta"].as_str());
    match text {
        Some(text) if item["role"] != "user" && !text.trim().is_empty() => vec![Event::Text(text.to_owned())],
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_claude_codes_stream() {
        let tool = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Making it."},{"type":"tool_use","name":"mcp__appdoctor__create_video","input":{"app_id":1}}]}}"#;
        assert_eq!(
            parse(Provider::ClaudeCode, tool),
            vec![
                Event::Text("Making it.".into()),
                Event::Tool {
                    name: "create_video".into(),
                    input: r#"{"app_id":1}"#.into()
                }
            ]
        );
        let result = r#"{"type":"user","message":{"content":[{"type":"tool_result","content":[{"type":"text","text":"{\"scenes\": 5}"}]}]}}"#;
        assert!(matches!(&parse(Provider::ClaudeCode, result)[0], Event::Result { text, failed: false } if text.contains("scenes")));
        let done = r#"{"type":"result","subtype":"success","is_error":false,"result":"Five scenes, sunset theme."}"#;
        assert_eq!(parse(Provider::ClaudeCode, done), vec![Event::Done("Five scenes, sunset theme.".into())]);
        assert!(parse(Provider::ClaudeCode, r#"{"type":"system","subtype":"init"}"#).is_empty());
    }

    #[test]
    fn finds_the_error_among_warnings() {
        let stderr = "Skill conflict detected: x\nSkill conflict detected: y\nError authenticating: IneligibleTierError: move to Antigravity\n    at foo";
        assert!(reason(stderr).starts_with("Error authenticating"));
    }

    #[test]
    fn reads_codex_and_gemini_events() {
        let call = r#"{"type":"item.started","item":{"type":"mcp_tool_call","server":"appdoctor","tool":"render_storyboard","arguments":{"app_id":1}}}"#;
        assert!(matches!(&parse(Provider::Codex, call)[0], Event::Tool { name, .. } if name == "render_storyboard"));
        let said = r#"{"type":"item.completed","item":{"type":"agent_message","text":"Done."}}"#;
        assert_eq!(parse(Provider::Codex, said), vec![Event::Text("Done.".into())]);
        let gemini = r#"{"type":"tool_use","tool_name":"create_video","parameters":{"app_id":1}}"#;
        assert!(matches!(&parse(Provider::Gemini, gemini)[0], Event::Tool { name, .. } if name == "create_video"));
        assert_eq!(parse(Provider::Gemini, "plain words"), vec![Event::Text("plain words".into())]);
    }
}
