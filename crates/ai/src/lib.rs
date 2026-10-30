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
    /// Ollama's downloaded models; empty for the others.
    pub models: Vec<String>,
    /// What stands in the way, if anything ("Ollama isn't running").
    pub problem: Option<String>,
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
            let (models, problem) = match provider {
                Provider::Ollama => match ollama::models() {
                    Ok(models) if models.is_empty() => (models, Some("No models yet: `ollama pull qwen3`".into())),
                    Ok(models) => (models, None),
                    Err(_) => (Vec::new(), Some("Ollama isn't running: open the Ollama app".into())),
                },
                _ => (Vec::new(), None),
            };
            Some(Found {
                provider,
                path,
                models,
                problem,
            })
        })
        .collect()
}

/// What happened, as the AI works.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The AI's own words.
    Text(String),
    /// It called a tool (AppDoctor's tools without their prefix).
    Tool { name: String, input: String },
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
    /// A model name: Ollama's must be set; the others use their default.
    pub model: Option<String>,
    pub prompt: String,
    /// What it's working on: the app, the design, how to work.
    pub context: String,
    /// How to start `appdoctor mcp` (for the command-line tools).
    pub launch: Launch,
    /// AppDoctor's data folder (Ollama's tools run in-process on it).
    pub data_dir: PathBuf,
    /// The app's source code, which the AI may read.
    pub project: Option<PathBuf>,
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
pub fn context(app: &str, app_id: u64, device: &str, design_folder: &Path) -> String {
    format!(
        "You are working inside AppDoctor, a Mac app for App Store marketing, on the app \"{app}\" (app_id {app_id}). \
         Work on its \"{device}\" design (pass app_id {app_id} and device \"{device}\" to the appdoctor tools). \
         The user watches the design change live in AppDoctor as you work. Make sensible choices instead of asking \
         questions. For videos, start with create_video, look at your work with render_storyboard, check it with \
         review_video and fix what it reports. Screens and recordings of the app may be in {}; the simulator tools \
         can record new ones. Finish with a one-paragraph summary of what you made.",
        design_folder.display()
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
