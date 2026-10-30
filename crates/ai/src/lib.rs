//! AppDoctor's in-app AI: the user's own AI does the work with AppDoctor's
//! tools, so there's no second subscription and nothing new to sign up for.
//!
//! - AI command-line tools the user already has signed in (Claude Code,
//!   Codex, Gemini CLI) run headless with AppDoctor's MCP server attached
//!   for that run only, allowed to use AppDoctor's tools and, when the user
//!   picks their app's project folder, to read its code.
//! - Ollama runs a local model; AppDoctor drives the tool loop itself over
//!   Ollama's HTTP API, with the tools served in-process.
//!
//! Progress comes back as [`Event`]s while the editor shows each change as
//! it lands in the design file.

mod cli;
mod ollama;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub use appdoctor_mcp::clients::Launch;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    ClaudeCode,
    Codex,
    Gemini,
    Ollama,
}

impl Provider {
    pub const ALL: [Provider; 4] = [Provider::ClaudeCode, Provider::Codex, Provider::Gemini, Provider::Ollama];

    pub fn label(self) -> &'static str {
        match self {
            Provider::ClaudeCode => "Claude Code",
            Provider::Codex => "Codex",
            Provider::Gemini => "Gemini CLI",
            Provider::Ollama => "Ollama",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Provider::ClaudeCode => "claude",
            Provider::Codex => "codex",
            Provider::Gemini => "gemini",
            Provider::Ollama => "ollama",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Provider> {
        Provider::ALL.into_iter().find(|p| p.slug() == slug)
    }

    fn command(self) -> &'static str {
        match self {
            Provider::ClaudeCode => "claude",
            Provider::Codex => "codex",
            Provider::Gemini => "gemini",
            Provider::Ollama => "ollama",
        }
    }
}

/// A provider found on this computer.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub provider: Provider,
    pub path: PathBuf,
    /// The models to choose from: Ollama's downloaded ones, the others'
    /// own list. Leaving it unchosen uses the provider's default (Ollama
    /// has none, so needs one).
    pub models: Vec<Model>,
    /// What the provider's default is, when it says ("GPT-5.6-Sol").
    pub default_model: Option<Model>,
    /// What stands in the way, if anything ("Ollama isn't running").
    pub problem: Option<String>,
}

/// A model a provider offers.
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    /// What the provider's `--model` takes.
    pub id: String,
    pub label: String,
    /// How hard it can think (`low` … `max`), weakest first; empty when
    /// there's no choice.
    pub efforts: Vec<String>,
}

impl Model {
    fn new(id: &str, label: &str, efforts: &[&str]) -> Model {
        Model {
            id: id.to_owned(),
            label: label.to_owned(),
            efforts: efforts.iter().map(|e| (*e).to_owned()).collect(),
        }
    }
}

/// An effort level in words.
pub fn effort_label(effort: &str) -> String {
    match effort {
        "xhigh" => "Extra high".to_owned(),
        other => {
            let mut chars = other.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    }
}

const CLAUDE_EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// Claude Code's aliases for the latest of each model.
fn claude_models() -> Vec<Model> {
    vec![
        Model::new("fable", "Fable", CLAUDE_EFFORTS),
        Model::new("opus", "Opus", CLAUDE_EFFORTS),
        Model::new("sonnet", "Sonnet", CLAUDE_EFFORTS),
        Model::new("haiku", "Haiku", &[]),
    ]
}

/// Gemini CLI's aliases.
fn gemini_models() -> Vec<Model> {
    vec![
        Model::new("pro", "Pro", &[]),
        Model::new("flash", "Flash", &[]),
        Model::new("flash-lite", "Flash-Lite", &[]),
    ]
}

/// The models Codex lists, from the list it keeps (`models_cache.json`),
/// and the one its settings choose.
fn codex_models() -> (Vec<Model>, Option<Model>) {
    let home = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")));
    let Some(home) = home else {
        return (Vec::new(), None);
    };
    let cache: Value = std::fs::read_to_string(home.join("models_cache.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null);
    let all: Vec<(Model, bool)> = cache["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model["slug"].as_str()?;
            let efforts = model["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|level| level["effort"].as_str())
                .collect::<Vec<_>>();
            let label = model["display_name"].as_str().unwrap_or(id);
            Some((Model::new(id, label, &efforts), model["visibility"] == "list"))
        })
        .collect();
    let chosen = std::fs::read_to_string(home.join("config.toml")).ok().and_then(|config| {
        config.lines().find_map(|line| {
            let value = line.trim().strip_prefix("model")?.trim_start().strip_prefix('=')?;
            Some(value.trim().trim_matches('"').to_owned())
        })
    });
    let default = chosen.and_then(|id| all.iter().find(|(model, _)| model.id == id).map(|(model, _)| model.clone()));
    let listed = all.into_iter().filter(|(_, listed)| *listed).map(|(model, _)| model).collect();
    (listed, default)
}

/// Where command-line tools live. Apps opened from the Dock don't get the
/// shell's PATH, so the usual install places are searched too.
fn search_path() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for dir in [".local/bin", ".claude/local", ".npm-global/bin", ".bun/bin", ".cargo/bin", ".volta/bin"] {
            dirs.push(home.join(dir));
        }
    }
    for dir in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] {
        dirs.push(PathBuf::from(dir));
    }
    dirs
}

fn which(command: &str) -> Option<PathBuf> {
    search_path()
        .into_iter()
        .map(|dir| dir.join(command))
        .find(|path| path.is_file())
        .or_else(|| bundled(command))
}

/// Copies of a command that apps carry inside them: Codex ships in the
/// ChatGPT app and in its editor extension.
fn bundled(command: &str) -> Option<PathBuf> {
    if command != "codex" {
        return None;
    }
    let app = PathBuf::from("/Applications/ChatGPT.app/Contents/Resources/codex-cli/bin/codex");
    if app.is_file() {
        return Some(app);
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    // The newest extension, in Cursor or VS Code.
    let mut found: Vec<PathBuf> = [".cursor/extensions", ".vscode/extensions"]
        .iter()
        .filter_map(|dir| std::fs::read_dir(home.join(dir)).ok())
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("openai.chatgpt-")))
        .flat_map(|extension| {
            ["macos-aarch64", "macos-x86_64"]
                .into_iter()
                .map(move |arch| extension.join("bin").join(arch).join("codex"))
        })
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found.pop()
}

/// The providers installed here, with Ollama's models when it's running.
pub fn detect() -> Vec<Found> {
    Provider::ALL
        .into_iter()
        .filter_map(|provider| {
            let path = which(provider.command())?;
            let (mut models, mut default_model, mut problem) = (Vec::new(), None, None);
            match provider {
                Provider::ClaudeCode => models = claude_models(),
                Provider::Codex => (models, default_model) = codex_models(),
                Provider::Gemini => models = gemini_models(),
                Provider::Ollama => match ollama::models() {
                    Ok(names) if names.is_empty() => problem = Some("No models yet: `ollama pull qwen3`".into()),
                    Ok(names) => models = names.iter().map(|name| Model::new(name, name, &[])).collect(),
                    Err(_) => problem = Some("Ollama isn't running: open the Ollama app".into()),
                },
            }
            Some(Found {
                provider,
                path,
                models,
                default_model,
                problem,
            })
        })
        .collect()
}

/// What happened, as the AI works.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The conversation's id, to carry on from it next time.
    Session(String),
    /// More of the words it's writing, as they come.
    Delta(String),
    /// A whole message of its own words (the end of any deltas before it).
    Text(String),
    /// What it's thinking about, for providers that say.
    Thinking(String),
    /// It called a tool (AppDoctor's tools without their prefix): `shell`
    /// for a command, the provider's own names for its others.
    Tool { name: String, input: Value },
    /// A tool's answer, shortened.
    Result { text: String, failed: bool },
    /// It finished; the summary is its last words, if any.
    Done(String),
    Failed(String),
}

/// A job for the AI.
pub struct Task {
    pub provider: Provider,
    /// The provider's executable, from [`detect`].
    pub path: PathBuf,
    /// A model's id: Ollama's must be set; the others use their default
    /// when it isn't.
    pub model: Option<String>,
    /// How hard it thinks (`low` … `max`), for the models that take one.
    pub effort: Option<String>,
    pub prompt: String,
    /// What it's working on: the app, the design, how to work.
    pub context: String,
    /// How to start `appdoctor mcp` (for the command-line tools).
    pub launch: Launch,
    /// AppDoctor's data folder (Ollama's tools run in-process on it).
    pub data_dir: PathBuf,
    /// The app's source code, which the AI may read.
    pub project: Option<PathBuf>,
    /// The conversation to carry on (from [`Event::Session`]), for the
    /// providers that keep their own.
    pub resume: Option<String>,
    /// Earlier requests and answers in this chat, for the others.
    pub history: Vec<(String, String)>,
}

impl Task {
    /// The request, after any earlier exchanges it follows on from.
    fn prompt_with_history(&self) -> String {
        if self.history.is_empty() {
            return self.prompt.clone();
        }
        let mut text = String::from("Earlier in this conversation:\n");
        for (asked, answered) in &self.history {
            text.push_str(&format!("\nThe user asked: {asked}\nYou answered: {answered}\n"));
        }
        text.push_str(&format!("\nNow the user asks: {}", self.prompt));
        text
    }
}

/// Runs a task to the end, reporting as it goes; blocks, so call it on a
/// thread of its own. Setting `cancel` stops it.
pub fn run(task: &Task, on_event: &mut dyn FnMut(Event), cancel: Arc<AtomicBool>) {
    let result = match task.provider {
        Provider::Ollama => ollama::run(task, on_event, &cancel),
        _ => cli::run(task, on_event, &cancel),
    };
    if let Err(error) = result {
        on_event(Event::Failed(error));
    }
}

/// The working rules every provider gets before the user's request.
pub fn context(app: &str, app_id: u64, device: &str, design_folder: &Path, project: Option<&Path>) -> String {
    let project = match project {
        Some(project) => format!(
            "The app's source code is in {} (your working folder): read its README and the screens that matter to \
             learn what the app does and how it looks, and use its real words, colours and features. You can read \
             files there but not run commands.",
            project.display()
        ),
        None => "You don't have the app's source code; get_design and the App Store listing are what you know.".into(),
    };
    format!(
        "You are working inside AppDoctor, a Mac app for App Store marketing, on the app \"{app}\" (app_id {app_id}). \
         Work on its \"{device}\" design (pass app_id {app_id} and device \"{device}\" to the appdoctor tools). \
         {project} The user watches the design change live in AppDoctor and chats with you in a small panel, so \
         keep your messages short and plain, without tables or code. Make sensible choices instead of asking \
         questions. For videos, start with create_video, look at your work with render_storyboard, check it with \
         review_video and fix what it reports. To draw a picture, icon or illustration, write SVG and add it with \
         import_svg, so each part becomes a layer the user can move. Screens and recordings of the app may be in \
         {}; the simulator tools can record new ones. Finish with two or three sentences on what you made.",
        design_folder.display()
    )
}

/// The working rules for improving an app's App Store listing.
pub fn listing_context(app: &str, app_id: u64, countries: &[String], project: Option<&Path>) -> String {
    let project = match project {
        Some(project) => format!(
            "The app's source code is in {} (your working folder): read it to learn what the app really does, \
             so the listing is true to it. You can read files there but not run commands.",
            project.display()
        ),
        None => String::new(),
    };
    let countries = if countries.is_empty() { "us".to_owned() } else { countries.join(", ") };
    format!(
        "You are working inside AppDoctor, a Mac app for App Store optimization, on the App Store listing of \
         \"{app}\" (app_id {app_id}), which the user tracks in these storefronts: {countries}. {project} aso_audit \
         scores the whole product page and lists what to fix first. Start with get_keywords (the tracked keywords with ranks, Apple popularity and difficulty per country) and get_listing \
         (the listing as pulled from App Store Connect, with which tracked keywords it covers and misses). Then \
         suggest changes with suggest_listing, one language at a time, with a short note on why. Suggestions don't \
         change the App Store: the user reviews them in AppDoctor and saves them with their own App Store Connect \
         key. Good listings put the most valuable keywords (popular, rankable) in the name and subtitle, fill the \
         100-character keyword field with other words separated by commas without spaces, never repeat a word \
         already in the name, subtitle or keyword field, stay within the limits and read naturally. Each storefront \
         uses the listing in its own language (English (U.K.) in the UK) when there is one, so a storefront without \
         one is an opportunity. The user chats with you in a small panel: keep messages short and plain, without \
         tables or code. Finish with two or three sentences on what you suggested and why."
    )
}

/// The first `limit` characters of a tool's answer, on one line.
fn shorten(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > limit {
        format!("{}…", flat.chars().take(limit).collect::<String>())
    } else {
        flat
    }
}
