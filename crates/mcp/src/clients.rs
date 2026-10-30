//! One-click setup: adds AppDoctor's server to the MCP settings of the AI
//! apps installed on this computer (Claude, Claude Code, Cursor, VS Code,
//! Windsurf, Codex, Gemini CLI, Antigravity, LM Studio).
//!
//! Each app keeps its servers in a file of its own. Only AppDoctor's entry
//! is added, replaced or removed; everything else in the file stays. The
//! first change to a file keeps a copy of it beside it
//! (`<name>.before-appdoctor`).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// The entry's name in every app's settings.
pub const SERVER_NAME: &str = "appdoctor";

/// How an app starts AppDoctor's server.
#[derive(Debug, Clone, PartialEq)]
pub struct Launch {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// `{ "<key>": { "appdoctor": { "command", "args", "env" } } }`, with
    /// `"type": "stdio"` too when `typed`.
    Json { key: &'static str, typed: bool },
    /// Codex's `config.toml`: a `[mcp_servers.appdoctor]` table.
    CodexToml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Client {
    pub name: &'static str,
    /// What to do after connecting, if anything.
    pub note: &'static str,
    pub config: PathBuf,
    /// Present when the app is installed.
    marker: PathBuf,
    format: Format,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    NotConnected,
    Connected,
    /// Connected to another copy of AppDoctor (a moved or older build).
    Outdated,
}

/// Every app AppDoctor knows how to connect, installed or not. `home` is
/// the user's home folder, `config` the OS settings folder
/// (`~/Library/Application Support` on macOS, `%APPDATA%` on Windows).
pub fn known_clients(home: &Path, config: &Path) -> Vec<Client> {
    let json = |key| Format::Json { key, typed: false };
    let client = |name, note, config: PathBuf, marker: PathBuf, format| Client {
        name,
        note,
        config,
        marker,
        format,
    };
    vec![
        client(
            "Claude",
            "Quit and reopen Claude to load it.",
            config.join("Claude/claude_desktop_config.json"),
            config.join("Claude"),
            json("mcpServers"),
        ),
        client(
            "Claude Code",
            "New sessions have it.",
            home.join(".claude.json"),
            home.join(".claude.json"),
            Format::Json {
                key: "mcpServers",
                typed: true,
            },
        ),
        client(
            "Cursor",
            "",
            home.join(".cursor/mcp.json"),
            home.join(".cursor"),
            json("mcpServers"),
        ),
        client(
            "VS Code",
            "For Copilot's agent mode.",
            config.join("Code/User/mcp.json"),
            config.join("Code/User"),
            Format::Json {
                key: "servers",
                typed: true,
            },
        ),
        client(
            "Windsurf",
            "",
            home.join(".codeium/windsurf/mcp_config.json"),
            home.join(".codeium/windsurf"),
            json("mcpServers"),
        ),
        client(
            "Codex",
            "New sessions have it.",
            home.join(".codex/config.toml"),
            home.join(".codex"),
            Format::CodexToml,
        ),
        client(
            "Gemini CLI",
            "",
            home.join(".gemini/settings.json"),
            home.join(".gemini/settings.json"),
            json("mcpServers"),
        ),
        client(
            "Antigravity",
            "",
            home.join(".gemini/antigravity/mcp_config.json"),
            home.join(".gemini/antigravity"),
            json("mcpServers"),
        ),
        client(
            "LM Studio",
            "",
            home.join(".lmstudio/mcp.json"),
            home.join(".lmstudio"),
            json("mcpServers"),
        ),
    ]
}

/// The apps that are installed here.
pub fn installed_clients(home: &Path, config: &Path) -> Vec<Client> {
    known_clients(home, config)
        .into_iter()
        .filter(Client::installed)
        .collect()
}

impl Client {
    pub fn installed(&self) -> bool {
        self.marker.exists()
    }

    pub fn status(&self, launch: &Launch) -> Result<Status, String> {
        let command = match self.format {
            Format::Json { key, .. } => {
                let root = read_json(&self.config)?;
                match root.get(key).and_then(|servers| servers.get(SERVER_NAME)) {
                    None => return Ok(Status::NotConnected),
                    Some(entry) => entry.get("command").and_then(Value::as_str).map(str::to_owned),
                }
            }
            Format::CodexToml => {
                let text = read_text(&self.config)?;
                let Some(section) = toml_section(&text) else {
                    return Ok(Status::NotConnected);
                };
                Some(text[section].to_owned())
                    .filter(|table| table.contains(&format!("command = {}", toml_string(&launch.command))))
                    .map(|_| launch.command.clone())
            }
        };
        Ok(if command.as_deref() == Some(launch.command.as_str()) {
            Status::Connected
        } else {
            Status::Outdated
        })
    }

    /// Adds AppDoctor's entry, or replaces one that points elsewhere.
    pub fn connect(&self, launch: &Launch) -> Result<(), String> {
        match self.format {
            Format::Json { key, typed } => {
                let mut root = read_json(&self.config)?;
                let object = root
                    .as_object_mut()
                    .ok_or_else(|| format!("{} isn't a JSON object.", self.config.display()))?;
                let servers = object.entry(key).or_insert_with(|| json!({}));
                let servers = servers
                    .as_object_mut()
                    .ok_or_else(|| format!("\"{key}\" in {} isn't a JSON object.", self.config.display()))?;
                servers.insert(SERVER_NAME.into(), json_entry(launch, typed));
                write_json(&self.config, &root)
            }
            Format::CodexToml => {
                let mut text = read_text(&self.config)?;
                if let Some(section) = toml_section(&text) {
                    text.replace_range(section, "");
                }
                let mut text = text.trim_end().to_owned();
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text.push_str(&toml_entry(launch));
                write_text(&self.config, &text)
            }
        }
    }

    /// Removes AppDoctor's entry, leaving the rest of the file.
    pub fn disconnect(&self) -> Result<(), String> {
        match self.format {
            Format::Json { key, .. } => {
                let mut root = read_json(&self.config)?;
                let removed = root
                    .get_mut(key)
                    .and_then(Value::as_object_mut)
                    .and_then(|servers| servers.remove(SERVER_NAME));
                match removed {
                    Some(_) => write_json(&self.config, &root),
                    None => Ok(()),
                }
            }
            Format::CodexToml => {
                let mut text = read_text(&self.config)?;
                match toml_section(&text) {
                    Some(section) => {
                        text.replace_range(section, "");
                        write_text(&self.config, &format!("{}\n", text.trim_end()))
                    }
                    None => Ok(()),
                }
            }
        }
    }
}

fn json_entry(launch: &Launch, typed: bool) -> Value {
    let mut entry = Map::new();
    if typed {
        entry.insert("type".into(), json!("stdio"));
    }
    entry.insert("command".into(), json!(launch.command));
    entry.insert("args".into(), json!(launch.args));
    if !launch.env.is_empty() {
        let env: Map<String, Value> = launch.env.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        entry.insert("env".into(), Value::Object(env));
    }
    Value::Object(entry)
}

fn toml_entry(launch: &Launch) -> String {
    let args: Vec<String> = launch.args.iter().map(|arg| toml_string(arg)).collect();
    let mut entry = format!(
        "[mcp_servers.{SERVER_NAME}]\ncommand = {}\nargs = [{}]\n",
        toml_string(&launch.command),
        args.join(", ")
    );
    if !launch.env.is_empty() {
        let env: Vec<String> = launch
            .env
            .iter()
            .map(|(key, value)| format!("{} = {}", toml_string(key), toml_string(value)))
            .collect();
        entry.push_str(&format!("env = {{ {} }}\n", env.join(", ")));
    }
    entry
}

/// A TOML basic string.
fn toml_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            c if c.is_control() => quoted.push_str(&format!("\\u{:04X}", c as u32)),
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

/// The byte range of AppDoctor's table (and any sub-tables) in a Codex
/// config, from its header to the next unrelated table.
fn toml_section(text: &str) -> Option<std::ops::Range<usize>> {
    let header = format!("[mcp_servers.{SERVER_NAME}]");
    let sub = format!("[mcp_servers.{SERVER_NAME}.");
    let mut start = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        match start {
            None if trimmed == header => start = Some(offset),
            Some(begin) if trimmed.starts_with('[') && !trimmed.starts_with(&sub) => return Some(begin..offset),
            _ => {}
        }
        offset += line.len();
    }
    start.map(|begin| begin..text.len())
}

fn read_text(path: &Path) -> Result<String, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(format!("Couldn't read {}: {error}", path.display())),
    }
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = read_text(path)?;
    if text.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&text).map_err(|error| {
        format!(
            "{} isn't plain JSON ({error}), so AppDoctor won't edit it. Add the server by hand.",
            path.display()
        )
    })
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    let mut text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    text.push('\n');
    write_text(path, &text)
}

/// Keeps the original once, then writes atomically.
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    let fail = |error: std::io::Error| format!("Couldn't write {}: {error}", path.display());
    if path.exists() {
        let mut backup = path.as_os_str().to_owned();
        backup.push(".before-appdoctor");
        let backup = PathBuf::from(backup);
        if !backup.exists() {
            std::fs::copy(path, &backup).map_err(fail)?;
        }
    }
    appdoctor_screenshots::write_atomic(path, text.as_bytes()).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch() -> Launch {
        Launch {
            command: "/Applications/AppDoctor.app/Contents/MacOS/appdoctor".into(),
            args: vec!["mcp".into()],
            env: Vec::new(),
        }
    }

    fn home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("appdoctor-clients-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn client<'a>(clients: &'a [Client], name: &str) -> &'a Client {
        clients.iter().find(|client| client.name == name).unwrap()
    }

    #[test]
    fn finds_only_installed_apps() {
        let home = home("installed");
        let config = home.join("Library/Application Support");
        std::fs::create_dir_all(home.join(".cursor")).unwrap();
        std::fs::create_dir_all(config.join("Claude")).unwrap();
        let names: Vec<_> = installed_clients(&home, &config).iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["Claude", "Cursor"]);
    }

    #[test]
    fn json_settings_keep_everything_else() {
        let home = home("json");
        let config = home.join("Library/Application Support");
        let clients = known_clients(&home, &config);
        let claude = client(&clients, "Claude");
        std::fs::create_dir_all(claude.config.parent().unwrap()).unwrap();
        std::fs::write(
            &claude.config,
            r#"{"zeta": 1, "mcpServers": {"other": {"command": "x"}}, "alpha": true}"#,
        )
        .unwrap();

        assert_eq!(claude.status(&launch()), Ok(Status::NotConnected));
        claude.connect(&launch()).unwrap();
        assert_eq!(claude.status(&launch()), Ok(Status::Connected));
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&claude.config).unwrap()).unwrap();
        assert_eq!(saved["mcpServers"]["other"]["command"], "x");
        assert_eq!(saved["zeta"], 1);
        assert_eq!(saved["mcpServers"]["appdoctor"]["args"], json!(["mcp"]));
        assert!(saved["mcpServers"]["appdoctor"].get("type").is_none());
        // The user's key order survives.
        let text = std::fs::read_to_string(&claude.config).unwrap();
        assert!(text.find("zeta").unwrap() < text.find("alpha").unwrap());
        // The original is kept beside it.
        let backup = std::fs::read_to_string(claude.config.with_extension("json.before-appdoctor")).unwrap();
        assert!(backup.contains("\"zeta\": 1"));

        let moved = Launch {
            command: "/elsewhere/appdoctor".into(),
            ..launch()
        };
        assert_eq!(claude.status(&moved), Ok(Status::Outdated));

        claude.disconnect().unwrap();
        assert_eq!(claude.status(&launch()), Ok(Status::NotConnected));
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&claude.config).unwrap()).unwrap();
        assert_eq!(saved["mcpServers"]["other"]["command"], "x");
    }

    #[test]
    fn creates_missing_files_and_types_entries_where_needed() {
        let home = home("create");
        let config = home.join("config");
        let clients = known_clients(&home, &config);
        let vscode = client(&clients, "VS Code");
        let with_env = Launch {
            env: vec![("APPDOCTOR_DATA_DIR".into(), "/data".into())],
            ..launch()
        };
        vscode.connect(&with_env).unwrap();
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&vscode.config).unwrap()).unwrap();
        assert_eq!(saved["servers"]["appdoctor"]["type"], "stdio");
        assert_eq!(saved["servers"]["appdoctor"]["env"]["APPDOCTOR_DATA_DIR"], "/data");
    }

    #[test]
    fn refuses_files_it_cannot_read_safely() {
        let home = home("jsonc");
        let clients = known_clients(&home, &home);
        let cursor = client(&clients, "Cursor");
        std::fs::create_dir_all(cursor.config.parent().unwrap()).unwrap();
        std::fs::write(&cursor.config, "{ // a comment\n }").unwrap();
        assert!(cursor.connect(&launch()).unwrap_err().contains("by hand"));
        assert_eq!(std::fs::read_to_string(&cursor.config).unwrap(), "{ // a comment\n }");
    }

    #[test]
    fn codex_tables_are_added_replaced_and_removed() {
        let home = home("codex");
        let clients = known_clients(&home, &home);
        let codex = client(&clients, "Codex");
        std::fs::create_dir_all(codex.config.parent().unwrap()).unwrap();
        let original = "model = \"o4\"\n\n[mcp_servers.other]\ncommand = \"x\"\n";
        std::fs::write(&codex.config, original).unwrap();

        let windows = Launch {
            command: r"C:\Program Files\AppDoctor\appdoctor.exe".into(),
            env: vec![("APPDOCTOR_DATA_DIR".into(), "D:\\data".into())],
            ..launch()
        };
        codex.connect(&windows).unwrap();
        assert_eq!(codex.status(&windows), Ok(Status::Connected));
        let text = std::fs::read_to_string(&codex.config).unwrap();
        assert!(text.starts_with(original));
        assert!(text.contains(r#"command = "C:\\Program Files\\AppDoctor\\appdoctor.exe""#));
        assert!(text.contains(r#"env = { "APPDOCTOR_DATA_DIR" = "D:\\data" }"#));

        // Connecting again replaces the table rather than adding another.
        codex.connect(&launch()).unwrap();
        let text = std::fs::read_to_string(&codex.config).unwrap();
        assert_eq!(text.matches("[mcp_servers.appdoctor]").count(), 1);
        assert_eq!(codex.status(&launch()), Ok(Status::Connected));

        codex.disconnect().unwrap();
        let text = std::fs::read_to_string(&codex.config).unwrap();
        assert_eq!(text.trim_end(), original.trim_end());
        assert_eq!(codex.status(&launch()), Ok(Status::NotConnected));
    }
}
