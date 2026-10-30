//! "Make with AI": a chat with the user's own AI (Claude Code, Codex,
//! Gemini CLI or a model in Ollama), which designs here with AppDoctor's
//! tools. Nothing new to sign up for: the command-line tools use their own
//! sign-in, and Ollama runs on this Mac. Each change lands in the design
//! file, which the editor watches, so the work appears as it happens; the
//! chat shows what it's doing in plain words and its replies as they're
//! written.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use appdoctor_ai::{Event, Found, Model, Provider, Task as AiTask, effort_label};
use appdoctor_screenshots::Target;
use futures::StreamExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use serde_json::Value;

use super::ScreenshotsView;
use super::view::AI_PANEL;
use crate::state::AppState;

const PROVIDER: &str = "ai_provider";
/// Project folders used lately, newest first, one per line.
const RECENT_PROJECTS: &str = "ai_recent_projects";
/// Before models could be chosen for every provider, Ollama's was here.
const OLD_OLLAMA_MODEL: &str = "ai_model";
/// Steps shown before older ones fold away.
const STEPS_SHOWN: usize = 4;

/// The providers' marks (see assets/logos/SOURCES.md), drawn in colour.
struct Logos {
    claude: Arc<Image>,
    gemini: Arc<Image>,
    openai: [Arc<Image>; 2],
    ollama: [Arc<Image>; 2],
}

static LOGOS: LazyLock<Logos> = LazyLock::new(|| {
    let svg = |bytes: &[u8]| Arc::new(Image::from_bytes(ImageFormat::Svg, bytes.to_vec()));
    Logos {
        claude: svg(include_bytes!("../../../assets/logos/claude.svg")),
        gemini: svg(include_bytes!("../../../assets/logos/gemini.svg")),
        openai: [
            svg(include_bytes!("../../../assets/logos/openai-blossom-black.svg")),
            svg(include_bytes!("../../../assets/logos/openai-blossom-white.svg")),
        ],
        ollama: [
            svg(include_bytes!("../../../assets/logos/ollama-black.svg")),
            svg(include_bytes!("../../../assets/logos/ollama-white.svg")),
        ],
    }
});

/// A provider's mark at `size`, for a light or dark background.
fn logo(provider: Provider, dark: bool, size: f32) -> Div {
    let shade = usize::from(dark);
    let (image, scale) = match provider {
        Provider::ClaudeCode => (LOGOS.claude.clone(), 1.),
        Provider::Gemini => (LOGOS.gemini.clone(), 1.),
        // OpenAI's file keeps its clear space around the mark.
        Provider::Codex => (LOGOS.openai[shade].clone(), 1.6),
        Provider::Ollama => (LOGOS.ollama[shade].clone(), 1.),
    };
    div()
        .size(px(size))
        .flex()
        .items_center()
        .justify_center()
        .child(img(image).size(px(size * scale)).flex_shrink_0())
}

/// One entry in the chat.
enum Item {
    /// What the user asked.
    Asked(String),
    /// The AI's words. `shown` bytes have appeared so far; an `open`
    /// message is still being written.
    Said { text: String, shown: usize, open: bool },
    /// Something it did or thought about.
    Step(Step),
    /// How a request ended, when it didn't simply finish.
    Ended { text: String, failed: bool },
}

struct Step {
    glyph: Glyph,
    doing: String,
    done: String,
    state: StepState,
}

#[derive(PartialEq)]
enum StepState {
    Working,
    Done,
    Failed(String),
    /// Refused: a tool AppDoctor doesn't let it use.
    Skipped,
}

#[derive(Clone, Copy)]
enum Glyph {
    Read,
    Search,
    Command,
    Apps,
    Design,
    Look,
    Video,
    Draw,
    Sound,
    Simulator,
    Export,
    Think,
}

impl Glyph {
    fn icon(self) -> Icon {
        use gpui_kit::assets::IconName as Extra;
        match self {
            Glyph::Read => Icon::new(IconName::FileText),
            Glyph::Search => Icon::new(IconName::Search),
            Glyph::Command => Icon::new(IconName::SquareTerminal),
            Glyph::Apps => Icon::new(IconName::LayoutDashboard),
            Glyph::Design => Icon::new(IconName::Palette),
            Glyph::Look => Icon::new(IconName::Eye),
            Glyph::Video => Icon::new(Extra::Clapperboard),
            Glyph::Draw => Icon::new(Extra::WandSparkles),
            Glyph::Sound => Icon::new(Extra::Volume2),
            Glyph::Simulator => Icon::new(Extra::Smartphone),
            Glyph::Export => Icon::new(IconName::ArrowDown),
            Glyph::Think => Icon::new(Extra::Lightbulb),
        }
    }
}

/// A tool call in plain words: what it's doing, and what it did.
fn describe(name: &str, input: &Value) -> (Glyph, String, String) {
    let text = |key: &str| input[key].as_str().unwrap_or_default().trim().to_owned();
    let file = |key: &str| {
        Path::new(&text(key))
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let slide = || input["slide"].as_u64().map(|n| format!(" {}", n + 1)).unwrap_or_default();
    let quoted = |words: String| {
        let words = words.split_whitespace().collect::<Vec<_>>().join(" ").replace('*', "");
        if words.chars().count() > 32 {
            format!("“{}…”", words.chars().take(30).collect::<String>())
        } else {
            format!("“{words}”")
        }
    };
    let pair = |glyph, doing: &str, done: &str, object: String| {
        let object = if object.is_empty() || doing.is_empty() { object } else { format!(" {object}") };
        (glyph, format!("{doing}{object}"), format!("{done}{object}"))
    };
    match name {
        "Read" | "read_file" | "read_many_files" | "view" => {
            let name = [file("file_path"), file("absolute_path"), file("path")]
                .into_iter()
                .find(|name| !name.is_empty())
                .unwrap_or_else(|| "a file".into());
            pair(Glyph::Read, "Reading", "Read", name)
        }
        "Glob" | "Grep" | "LS" | "glob" | "grep" | "list_directory" | "search_file_content" => {
            pair(Glyph::Search, "Looking through", "Looked through", "the code".into())
        }
        "shell" | "Bash" | "run_shell_command" => pair(Glyph::Command, "Running", "Ran", "a command".into()),
        "list_apps" => pair(Glyph::Apps, "Checking", "Checked", "your apps".into()),
        "get_design" => pair(Glyph::Design, "Reading", "Read", "the design".into()),
        "design_options" | "list_fonts" | "list_themes" => {
            pair(Glyph::Design, "Looking at", "Looked at", "the options".into())
        }
        "add_slide" => pair(Glyph::Design, "Adding", "Added", "a slide".into()),
        "delete_slide" => pair(Glyph::Design, "Removing", "Removed", format!("slide{}", slide())),
        "move_slide" => pair(Glyph::Design, "Moving", "Moved", format!("slide{}", slide())),
        "set_background" => pair(Glyph::Design, "Colouring", "Coloured", format!("slide{}", slide())),
        "add_layer" => {
            let what = match input["kind"].as_str() {
                Some("text") if !text("text").is_empty() => quoted(text("text")),
                Some("device") => "a device".into(),
                Some("image") => "an image".into(),
                Some("shape") => "a shape".into(),
                _ => "a layer".into(),
            };
            pair(Glyph::Design, "Adding", "Added", what)
        }
        "update_layer" if !text("text").is_empty() => pair(Glyph::Design, "Writing", "Wrote", quoted(text("text"))),
        "update_layer" => pair(Glyph::Design, "Adjusting", "Adjusted", "a layer".into()),
        "delete_layer" => pair(Glyph::Design, "Removing", "Removed", "a layer".into()),
        "arrange_layer" => pair(Glyph::Design, "Rearranging", "Rearranged", "the layers".into()),
        "import_svg" => pair(Glyph::Draw, "Drawing", "Drew", "a picture".into()),
        "apply_theme" if !text("theme").is_empty() => {
            pair(Glyph::Design, "Applying", "Applied", format!("the {} theme", text("theme")))
        }
        "apply_theme" => pair(Glyph::Design, "Applying", "Applied", "a theme".into()),
        "render_slide" => pair(Glyph::Look, "Looking at", "Looked at", format!("slide{}", slide())),
        "render_frame" => pair(Glyph::Look, "Looking at", "Looked at", "a frame".into()),
        "render_storyboard" => pair(Glyph::Look, "Looking at", "Looked at", "the storyboard".into()),
        "review_video" => pair(Glyph::Look, "Checking", "Checked", "the video".into()),
        "create_video" => pair(Glyph::Video, "Making", "Made", "the video".into()),
        "set_video" if input["enabled"] == false => pair(Glyph::Video, "Turning", "Turned", "the video off".into()),
        "set_video" => pair(Glyph::Video, "Setting up", "Set up", "the video".into()),
        "update_slide" => pair(Glyph::Video, "Timing", "Timed", format!("scene{}", slide())),
        "set_camera" => pair(Glyph::Video, "Moving", "Moved", "the camera".into()),
        "add_callout" => pair(Glyph::Video, "Adding", "Added", "a callout".into()),
        "add_sound" if !text("effect").is_empty() => {
            pair(Glyph::Sound, "Adding", "Added", format!("a {} sound", text("effect")))
        }
        "add_sound" => pair(Glyph::Sound, "Adding", "Added", "a sound".into()),
        "remove_sound" => pair(Glyph::Sound, "Removing", "Removed", "a sound".into()),
        "export_video" => pair(Glyph::Export, "Exporting", "Exported", "the video".into()),
        "export_screenshots" => pair(Glyph::Export, "Exporting", "Exported", "the screenshots".into()),
        "simulator_devices" => pair(Glyph::Simulator, "Checking", "Checked", "the simulators".into()),
        "simulator_screenshot" => pair(Glyph::Simulator, "Capturing", "Captured", "the simulator's screen".into()),
        "record_simulator" => pair(Glyph::Simulator, "Recording", "Recorded", "the simulator".into()),
        "simulator_open_url" => pair(Glyph::Simulator, "Opening", "Opened", "a screen in the simulator".into()),
        other => {
            let words = other.replace(['_', '-'], " ");
            let mut chars = words.chars();
            let words: String = chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
            pair(Glyph::Design, "", "", words)
        }
    }
}

/// The headline of a thought: its first bold phrase or line, briefly.
fn headline(thought: &str) -> String {
    let line = thought
        .split("**")
        .nth(1)
        .filter(|bold| !bold.trim().is_empty())
        .unwrap_or_else(|| thought.lines().find(|line| !line.trim().is_empty()).unwrap_or_default());
    let line = line.trim().trim_end_matches(['.', ':']).replace('*', "");
    if line.chars().count() > 60 {
        format!("{}…", line.chars().take(58).collect::<String>())
    } else {
        line
    }
}

/// A failed tool's answer, briefly.
fn reason(text: &str) -> String {
    let first = text
        .split(['\n', '.'])
        .map(str::trim)
        .find(|part| !part.is_empty())
        .unwrap_or("It didn't work");
    if first.chars().count() > 90 {
        format!("{}…", first.chars().take(88).collect::<String>())
    } else {
        first.to_owned()
    }
}

/// A line of the AI's words with **bold** and `code` marks drawn, not
/// shown. A mark still open (the words are still coming) runs to the end.
fn styled_line(line: &str, code: Hsla) -> StyledText {
    let bold = HighlightStyle {
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    let mono = HighlightStyle {
        color: Some(code),
        ..Default::default()
    };
    let mut clean = String::new();
    let mut highlights = Vec::new();
    let (mut bold_from, mut mono_from) = (None::<usize>, None::<usize>);
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix("**") {
            match bold_from.take() {
                Some(start) => highlights.push((start..clean.len(), bold)),
                None => bold_from = Some(clean.len()),
            }
            rest = after;
        } else if let Some(after) = rest.strip_prefix('`') {
            match mono_from.take() {
                Some(start) => highlights.push((start..clean.len(), mono)),
                None => mono_from = Some(clean.len()),
            }
            rest = after;
        } else {
            clean.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    if let Some(start) = bold_from {
        highlights.push((start..clean.len(), bold));
    }
    if let Some(start) = mono_from {
        highlights.push((start..clean.len(), mono));
    }
    highlights.sort_by_key(|(range, _)| range.start);
    // Highlights can't overlap; keep the first of any pair.
    let mut end = 0;
    highlights.retain(|(range, _)| {
        let keep = range.start >= end && range.start < range.end;
        if keep {
            end = range.end;
        }
        keep
    });
    StyledText::new(clean).with_highlights(highlights)
}

/// The AI's words as paragraphs, bullets and headings.
fn said_view(text: &str, foreground: Hsla, muted: Hsla, code: Hsla) -> Div {
    let mut column = v_flex()
        .gap(px(6.))
        .text_size(px(13.))
        .line_height(rems(1.4))
        .text_color(foreground);
    for line in text.lines().map(str::trim_end) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let bullet = ["- ", "* ", "• "].iter().find_map(|mark| trimmed.strip_prefix(mark));
        let heading = trimmed.strip_prefix('#').map(|rest| rest.trim_start_matches('#').trim());
        column = column.child(match (bullet, heading) {
            (Some(item), _) => h_flex()
                .items_start()
                .gap(px(7.))
                .pl(px(2.))
                .child(div().mt(px(8.)).size(px(4.)).rounded_full().bg(muted).flex_shrink_0())
                .child(div().flex_1().min_w_0().child(styled_line(item, code))),
            (None, Some(heading)) => div().font_weight(FontWeight::SEMIBOLD).child(styled_line(heading, code)),
            _ => div().child(styled_line(line, code)),
        });
    }
    column
}

/// Looks for the app's code in the usual places: a folder named after the
/// app (or the last part of its bundle ID) that holds a project.
fn find_project(name: &str, bundle_id: Option<&str>) -> Option<PathBuf> {
    let squash = |text: &str| {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase()
    };
    let mut wanted: Vec<String> = Vec::new();
    if let Some(last) = bundle_id.and_then(|id| id.rsplit('.').next()) {
        wanted.push(squash(last));
    }
    // "Meditation & Sleep: Strobia" names the app after the colon.
    if let Some((_, brand)) = name.rsplit_once([':', '–', '—', '|']) {
        wanted.push(squash(brand));
    }
    wanted.push(squash(name));
    let mut words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric())
        .map(squash)
        .filter(|w| w.len() >= 5)
        .collect();
    words.sort_by_key(|w| std::cmp::Reverse(w.len()));
    wanted.extend(words);
    wanted.retain(|w| w.len() >= 3);
    wanted.dedup();

    let home = PathBuf::from(std::env::var_os("HOME")?);
    let roots = [
        "Apps", "Developer", "Projects", "Code", "code", "dev", "src", "Sites", "repos", "workspace", "GitHub",
        "Documents/GitHub", "Documents/Projects", "Documents/Developer", "Desktop", "Documents",
    ];
    let folders: Vec<PathBuf> = roots
        .iter()
        .filter_map(|root| std::fs::read_dir(home.join(root)).ok())
        .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    let is_project = |path: &Path| {
        ["README.md", ".git", "Package.swift", "package.json", "Podfile"]
            .iter()
            .any(|f| path.join(f).exists())
            || std::fs::read_dir(path).is_ok_and(|entries| {
                entries
                    .flatten()
                    .any(|e| e.path().extension().is_some_and(|x| x == "xcodeproj" || x == "xcworkspace"))
            })
    };
    wanted.iter().find_map(|word| {
        folders
            .iter()
            .find(|folder| {
                folder.file_name().is_some_and(|name| squash(&name.to_string_lossy()) == *word) && is_project(folder)
            })
            .cloned()
    })
}

/// `~/…` for a path in the home folder.
fn tilde(path: &Path) -> String {
    match std::env::var_os("HOME").and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

pub(super) struct AiPanel {
    state: Entity<AppState>,
    view: WeakEntity<ScreenshotsView>,
    found: Vec<Found>,
    detecting: bool,
    provider: Option<Provider>,
    /// The chosen model's id and effort; unset uses the provider's default.
    model: Option<String>,
    effort: Option<String>,
    prompt: Entity<TextareaState>,
    running: bool,
    cancel: Arc<AtomicBool>,
    chat: Vec<Item>,
    /// The app the chat is about; another app starts a new one.
    chat_app: Option<u64>,
    /// The provider's own conversation, to carry on from.
    session: Option<(Provider, String)>,
    /// Step groups (by first item) opened to show every step.
    unfolded: HashSet<usize>,
    scroll: ScrollHandle,
    /// Keeps the newest words in view, until the user scrolls up.
    following: bool,
    /// The last frame, for revealing words at a steady pace.
    tick: Instant,
    /// A folder that looks like the app's code, for an app without one.
    suggestion: Option<(u64, PathBuf)>,
    looked_for: Option<u64>,
    _run: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl AiPanel {
    pub(super) fn new(
        state: Entity<AppState>,
        view: WeakEntity<ScreenshotsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AiPanel {
        let prompt = cx.new(|cx| {
            let mut prompt = TextareaState::new(window, cx)
                .auto_grow(1, 8)
                .placeholder("Ask for screenshots, a video, a drawing…");
            prompt.set_submit_on_enter(true, cx);
            prompt
        });
        let subscriptions = vec![cx.subscribe_in(&prompt, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = event {
                this.send(window, cx);
            }
        })];
        let mut panel = AiPanel {
            state,
            view,
            found: Vec::new(),
            detecting: false,
            provider: None,
            model: None,
            effort: None,
            prompt,
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            chat: Vec::new(),
            chat_app: None,
            session: None,
            unfolded: HashSet::new(),
            scroll: ScrollHandle::new(),
            following: true,
            tick: Instant::now(),
            suggestion: None,
            looked_for: None,
            _run: None,
            _subscriptions: subscriptions,
        };
        panel.detect(cx);
        panel
    }

    fn setting(&self, key: &str, cx: &App) -> Option<String> {
        self.state.read(cx).services.store.setting(key).ok().flatten()
    }

    fn save(&self, key: &str, value: &str, cx: &App) {
        let _ = self.state.read(cx).services.store.set_setting(key, value);
    }

    /// Looks for the providers on this Mac (and Ollama's models).
    fn detect(&mut self, cx: &mut Context<Self>) {
        self.detecting = true;
        cx.spawn(async move |this, cx| {
            let found = cx.background_spawn(async { appdoctor_ai::detect() }).await;
            this.update(cx, |this, cx| {
                this.detecting = false;
                let saved = this.setting(PROVIDER, cx).and_then(|slug| Provider::from_slug(&slug));
                this.provider = saved
                    .filter(|p| found.iter().any(|f| f.provider == *p))
                    .or_else(|| found.first().map(|f| f.provider));
                this.found = found;
                this.load_choices(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn found(&self) -> Option<&Found> {
        self.provider.and_then(|p| self.found.iter().find(|f| f.provider == p))
    }

    /// The chosen model, if it's still offered.
    fn chosen_model(&self) -> Option<&Model> {
        let id = self.model.as_deref()?;
        self.found()?.models.iter().find(|model| model.id == id)
    }

    /// The efforts the chosen model (or the default one) can take.
    fn efforts(&self) -> Vec<String> {
        let Some(found) = self.found() else {
            return Vec::new();
        };
        self.chosen_model()
            .or(found.default_model.as_ref())
            .or(found.models.first().filter(|_| found.provider == Provider::ClaudeCode))
            .map(|model| model.efforts.clone())
            .unwrap_or_default()
    }

    /// The provider's saved model and effort, where still on offer.
    fn load_choices(&mut self, cx: &App) {
        let Some(found) = self.found().cloned() else {
            return;
        };
        let slug = found.provider.slug();
        let saved = self
            .setting(&format!("ai_model:{slug}"), cx)
            .or_else(|| (found.provider == Provider::Ollama).then(|| self.setting(OLD_OLLAMA_MODEL, cx)).flatten());
        self.model = saved.filter(|id| found.models.iter().any(|model| model.id == *id));
        // Ollama has no default of its own.
        if found.provider == Provider::Ollama && self.model.is_none() {
            self.model = found.models.first().map(|model| model.id.clone());
        }
        self.effort = None;
        self.effort = self
            .setting(&format!("ai_effort:{slug}"), cx)
            .filter(|effort| self.efforts().contains(effort));
    }

    fn choose_model(&mut self, model: Option<String>, cx: &mut Context<Self>) {
        let Some(provider) = self.provider else {
            return;
        };
        self.save(&format!("ai_model:{}", provider.slug()), model.as_deref().unwrap_or_default(), cx);
        self.model = model;
        // An effort the new model can't take goes back to its default.
        if self.effort.as_ref().is_some_and(|effort| !self.efforts().contains(effort)) {
            self.choose_effort(None, cx);
        }
        cx.notify();
    }

    fn choose_effort(&mut self, effort: Option<String>, cx: &mut Context<Self>) {
        let Some(provider) = self.provider else {
            return;
        };
        self.save(&format!("ai_effort:{}", provider.slug()), effort.as_deref().unwrap_or_default(), cx);
        self.effort = effort;
        cx.notify();
    }

    fn app(&self, cx: &App) -> Option<(u64, String)> {
        self.state.read(cx).selected().map(|app| (app.id, app.name.clone()))
    }

    fn project_key(&self, cx: &App) -> Option<String> {
        self.app(cx).map(|(id, _)| format!("ai_project:{id}"))
    }

    /// The app's source folder, which the AI may read.
    fn project(&self, cx: &App) -> Option<PathBuf> {
        self.project_key(cx)
            .and_then(|key| self.setting(&key, cx))
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
    }

    fn set_project(&mut self, folder: Option<&Path>, cx: &mut Context<Self>) {
        if let Some(key) = self.project_key(cx) {
            self.save(&key, &folder.map(|f| f.display().to_string()).unwrap_or_default(), cx);
        }
        if let Some(folder) = folder {
            let mut recent = vec![folder.to_path_buf()];
            recent.extend(self.recent_projects(cx).into_iter().filter(|path| path != folder).take(5));
            let lines: Vec<String> = recent.iter().map(|path| path.display().to_string()).collect();
            self.save(RECENT_PROJECTS, &lines.join("\n"), cx);
        }
        // A conversation that started without the code can't see it.
        self.session = None;
        cx.notify();
    }

    /// Project folders used lately that still exist.
    fn recent_projects(&self, cx: &App) -> Vec<PathBuf> {
        self.setting(RECENT_PROJECTS, cx)
            .unwrap_or_default()
            .lines()
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .collect()
    }

    /// A folder that looks like this app's code, when it has none yet.
    fn suggested_project(&self, cx: &App) -> Option<PathBuf> {
        let app = self.app(cx).map(|(id, _)| id);
        self.suggestion
            .as_ref()
            .filter(|(id, _)| Some(*id) == app)
            .map(|(_, folder)| folder.clone())
    }

    fn choose_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Use This Folder".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.set_project(Some(&folder), cx)).ok();
        })
        .detach();
    }

    /// For an app without a folder, looks once for one that seems to hold
    /// its code.
    fn look_for_project(&mut self, cx: &mut Context<Self>) {
        let Some(app) = self.state.read(cx).selected() else {
            return;
        };
        if self.looked_for == Some(app.id) {
            return;
        }
        self.looked_for = Some(app.id);
        let (id, name, bundle_id) = (app.id, app.name.clone(), app.bundle_id.clone());
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move { find_project(&name, bundle_id.as_deref()) })
                .await;
            this.update(cx, |this, cx| {
                this.suggestion = found.map(|folder| (id, folder));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Earlier requests and the last thing said after each, for providers
    /// that don't keep the conversation themselves.
    fn history(&self) -> Vec<(String, String)> {
        let mut history: Vec<(String, String)> = Vec::new();
        for item in &self.chat {
            match item {
                Item::Asked(text) => history.push((text.clone(), String::new())),
                Item::Said { text, .. } => {
                    if let Some(last) = history.last_mut() {
                        last.1 = text.clone();
                    }
                }
                _ => {}
            }
        }
        history.retain(|(_, answered)| !answered.is_empty());
        history
    }

    fn can_send(&self) -> bool {
        !self.running
            && self.provider.is_some_and(|provider| {
                self.found
                    .iter()
                    .any(|f| f.provider == provider && (f.problem.is_none() || provider != Provider::Ollama))
            })
    }

    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = self.prompt.read(cx).value().trim().to_owned();
        if prompt.is_empty() || !self.can_send() {
            return;
        }
        self.prompt.update(cx, |state, cx| state.set_value("", window, cx));
        self.ask(prompt, cx);
    }

    fn ask(&mut self, prompt: String, cx: &mut Context<Self>) {
        let (Some(provider), Some((app_id, app_name))) = (self.provider, self.app(cx)) else {
            return;
        };
        let Some(found) = self.found.iter().find(|f| f.provider == provider).cloned() else {
            return;
        };
        let state = self.state.read(cx);
        let data_dir = state.services.data_dir.clone();
        let target = state.screenshot_target;
        let folder = data_dir.join("screenshots").join(app_id.to_string());
        let project = self.project(cx);
        let resume = self
            .session
            .as_ref()
            .filter(|(p, _)| *p == provider && matches!(provider, Provider::ClaudeCode | Provider::Codex))
            .map(|(_, id)| id.clone());
        let task = AiTask {
            provider,
            path: found.path,
            model: self.model.clone(),
            effort: self.effort.clone(),
            prompt: prompt.clone(),
            context: appdoctor_ai::context(&app_name, app_id, target.slug(), &folder, project.as_deref()),
            launch: super::agent::launch(),
            data_dir,
            project,
            history: if resume.is_some() { Vec::new() } else { self.history() },
            resume,
        };
        self.chat_app = Some(app_id);
        self.chat.push(Item::Asked(prompt));
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.running = true;
        self.following = true;
        let (events, mut received) = futures::channel::mpsc::unbounded::<Event>();
        std::thread::Builder::new()
            .name("ai".into())
            .spawn(move || appdoctor_ai::run(&task, &mut |event| drop(events.unbounded_send(event)), cancel))
            .ok();
        self._run = Some(cx.spawn(async move |this, cx| {
            while let Some(event) = received.next().await {
                let alive = this.update(cx, |this, cx| {
                    this.take(provider, event);
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                this.running = false;
                this.settle(StepState::Done);
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Ends whatever's still in progress: steps as `state` (done or
    /// skipped), and any message being written.
    fn settle(&mut self, state: StepState) {
        let skipped = state == StepState::Skipped;
        for item in &mut self.chat {
            match item {
                Item::Step(step) if step.state == StepState::Working => {
                    step.state = if skipped { StepState::Skipped } else { StepState::Done };
                }
                Item::Said { open, .. } => *open = false,
                _ => {}
            }
        }
    }

    /// Adds what the AI did to the chat.
    fn take(&mut self, provider: Provider, event: Event) {
        match event {
            Event::Session(id) => self.session = Some((provider, id)),
            Event::Delta(piece) => match self.chat.last_mut() {
                Some(Item::Said { text, open: true, .. }) => text.push_str(&piece),
                _ => self.chat.push(Item::Said {
                    text: piece,
                    shown: 0,
                    open: true,
                }),
            },
            Event::Text(whole) => match self.chat.last_mut() {
                // The whole of the message that was streaming in.
                Some(Item::Said { text, open, shown }) if *open => {
                    *text = whole;
                    *open = false;
                    *shown = (*shown).min(text.len());
                    while !text.is_char_boundary(*shown) {
                        *shown -= 1;
                    }
                }
                _ if whole.trim().is_empty() => {}
                _ => self.chat.push(Item::Said {
                    text: whole,
                    shown: 0,
                    open: false,
                }),
            },
            Event::Thinking(thought) => {
                let line = headline(&thought);
                if !line.is_empty() {
                    self.settle(StepState::Done);
                    self.chat.push(Item::Step(Step {
                        glyph: Glyph::Think,
                        doing: line.clone(),
                        done: line,
                        state: StepState::Done,
                    }));
                }
            }
            Event::Tool { name, input } => {
                self.settle(StepState::Done);
                let (glyph, doing, done) = describe(&name, &input);
                self.chat.push(Item::Step(Step {
                    glyph,
                    doing,
                    done,
                    state: StepState::Working,
                }));
            }
            Event::Result { text, failed } => {
                let step = self.chat.iter_mut().rev().find_map(|item| match item {
                    Item::Step(step) if step.state == StepState::Working => Some(step),
                    _ => None,
                });
                if let Some(step) = step {
                    let lower = text.to_lowercase();
                    let refused = ["approval", "permission", "not allowed", "denied"]
                        .iter()
                        .any(|word| lower.contains(word));
                    step.state = match (failed, refused) {
                        (false, _) => StepState::Done,
                        (true, true) => StepState::Skipped,
                        (true, false) => StepState::Failed(reason(&text)),
                    };
                }
            }
            Event::Done(summary) => {
                self.settle(StepState::Done);
                // Claude Code ends by repeating its last message.
                let said_already = self
                    .chat
                    .iter()
                    .rev()
                    .take_while(|item| !matches!(item, Item::Asked(_)))
                    .any(|item| matches!(item, Item::Said { text, .. } if text.trim() == summary.trim()));
                if !summary.trim().is_empty() && !said_already {
                    self.chat.push(Item::Said {
                        text: summary,
                        shown: 0,
                        open: false,
                    });
                }
            }
            Event::Failed(text) => {
                self.settle(StepState::Skipped);
                let stopped = text == "Stopped.";
                self.chat.push(Item::Ended { text, failed: !stopped });
            }
        }
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        cx.notify();
    }

    fn new_chat(&mut self, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        self.chat.clear();
        self.session = None;
        self.unfolded.clear();
        cx.notify();
    }

    fn suggestions(&self, cx: &App) -> &'static [&'static str] {
        match self.state.read(cx).screenshot_target {
            Target::Iphone | Target::Ipad => &[
                "Make a 20-second app preview of the app's best features",
                "Design 5 screenshots with short benefit headlines",
                "Draw an illustration of what the app does on slide 1",
            ],
            Target::Header | Target::Search => &[
                "Make a bold looping header video that shows what the app does",
                "Design a calm, premium header image",
                "Draw an illustration of the app's idea, as layers",
            ],
        }
    }

    /// Reveals the AI's words at a steady pace and glides to the newest;
    /// true while there's more to animate.
    fn animate(&mut self) -> bool {
        let now = Instant::now();
        let dt = now.duration_since(self.tick).as_secs_f32().min(0.1);
        self.tick = now;
        let mut moving = false;
        for item in &mut self.chat {
            if let Item::Said { text, shown, .. } = item
                && *shown < text.len()
            {
                // Faster with more to show, so it never falls far behind.
                let backlog = (text.len() - *shown) as f32;
                let step = (dt * (70. + backlog * 6.)).ceil().max(1.) as usize;
                let mut next = (*shown + step).min(text.len());
                while !text.is_char_boundary(next) {
                    next += 1;
                }
                *shown = next;
                moving = true;
            }
        }
        if self.following {
            let bottom = f32::from(self.scroll.max_offset().y);
            let at = -f32::from(self.scroll.offset().y);
            let gap = bottom - at;
            if gap > 0.5 {
                let next = at + (gap * 0.22).max(1.).min(gap);
                self.scroll.set_offset(point(px(0.), px(-next)));
                moving = true;
            }
        }
        moving
    }

    fn header(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let running = self.running;
        let problem = self.found().and_then(|f| f.problem.clone());
        v_flex()
            .px(px(12.))
            .pt(px(12.))
            .pb(px(10.))
            .gap(px(9.))
            .border_b_1()
            .border_color(theme.border)
            .child(
                h_flex()
                    .justify_between()
                    .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("Make with AI"))
                    .child(
                        h_flex()
                            .gap(px(2.))
                            .when(!self.chat.is_empty() && !running, |this| {
                                this.child(
                                    Button::new("ai-new-chat")
                                        .ghost()
                                        .xsmall()
                                        .icon(gpui_kit::assets::IconName::MessageSquarePlus)
                                        .tooltip("New chat")
                                        .on_click(cx.listener(|this, _, _, cx| this.new_chat(cx))),
                                )
                            })
                            .child(
                                Button::new("ai-detect")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::RefreshCw)
                                    .loading(self.detecting)
                                    .tooltip("Look for AI again")
                                    .on_click(cx.listener(|this, _, _, cx| this.detect(cx))),
                            ),
                    ),
            )
            .when(self.found.is_empty() && !self.detecting, |this| {
                this.child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(
                    "No AI found on this Mac. Install Claude Code, Codex (it comes with the ChatGPT app), Gemini CLI \
                     or Ollama, then look again.",
                ))
            })
            .when_some(problem, |this, problem| {
                this.child(div().text_size(px(11.5)).text_color(theme.warning).child(problem))
            })
    }

    /// Switches to `provider` (with its saved effort) and `model`.
    fn choose(&mut self, provider: Provider, model: Option<String>, cx: &mut Context<Self>) {
        if self.running {
            return;
        }
        if self.provider != Some(provider) {
            self.provider = Some(provider);
            self.save(PROVIDER, provider.slug(), cx);
            self.load_choices(cx);
        }
        self.choose_model(model, cx);
    }

    /// Which AI and model: a pop-up menu, by provider, with the current one
    /// ticked.
    fn model_menu(&self, dark: bool, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let panel = cx.entity().downgrade();
        let found = self.found.clone();
        let current = (self.provider, self.model.clone());
        let title = match (self.chosen_model(), self.found()) {
            (Some(model), _) => model.label.clone(),
            (None, Some(found)) => found
                .default_model
                .as_ref()
                .map_or_else(|| found.provider.label().to_owned(), |model| model.label.clone()),
            (None, None) => "Choose an AI".to_owned(),
        };
        Button::new("ai-model-menu")
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .disabled(self.running || self.found.is_empty())
            .tooltip("Which AI does the work")
            .child(
                h_flex()
                    .gap(px(5.))
                    .items_center()
                    .when_some(self.provider, |this, provider| this.child(logo(provider, dark, 13.)))
                    .child(div().max_w(px(100.)).truncate().text_color(theme.foreground).child(title)),
            )
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu.min_w(px(220.)).max_h(px(440.)).scrollable(true);
                for (ix, found) in found.iter().enumerate() {
                    let provider = found.provider;
                    if ix > 0 {
                        menu = menu.separator();
                    }
                    menu = menu.label(provider.label());
                    if let Some(problem) = &found.problem {
                        menu = menu.item(PopupMenuItem::new(problem.clone()).disabled(true));
                        continue;
                    }
                    let mut choices: Vec<(Option<String>, String)> = Vec::new();
                    if provider != Provider::Ollama {
                        let default = found
                            .default_model
                            .as_ref()
                            .map_or_else(|| "Default".to_owned(), |model| format!("Default ({})", model.label));
                        choices.push((None, default));
                    }
                    choices.extend(found.models.iter().map(|model| (Some(model.id.clone()), model.label.clone())));
                    for (model, label) in choices {
                        let checked = current.0 == Some(provider) && current.1 == model;
                        let panel = panel.clone();
                        menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(move |_, _, cx| {
                            panel.update(cx, |this, cx| this.choose(provider, model.clone(), cx)).ok();
                        }));
                    }
                }
                menu
            })
    }

    /// How hard it thinks: a pop-up menu, for models that take a choice.
    fn effort_menu(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let efforts = self.efforts();
        if efforts.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let panel = cx.entity().downgrade();
        let current = self.effort.clone();
        let title = self.effort.as_deref().map_or_else(|| "Effort".to_owned(), effort_label);
        Some(
            Button::new("ai-effort-menu")
                .ghost()
                .xsmall()
                .dropdown_caret(true)
                .disabled(self.running)
                .tooltip("Effort: how hard it thinks. More is slower but more careful.")
                .child(
                    h_flex()
                        .gap(px(4.))
                        .items_center()
                        .text_color(theme.muted_foreground)
                        .child(Icon::new(gpui_kit::assets::IconName::Gauge).size(px(13.)))
                        .child(title),
                )
                .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                    menu = menu.min_w(px(170.)).label("Effort");
                    let choices = std::iter::once(None).chain(efforts.iter().cloned().map(Some));
                    for effort in choices {
                        let label = effort.as_deref().map_or_else(|| "Default".to_owned(), effort_label);
                        let checked = current == effort;
                        let panel = panel.clone();
                        menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(move |_, _, cx| {
                            panel.update(cx, |this, cx| this.choose_effort(effort.clone(), cx)).ok();
                        }));
                    }
                    menu
                }),
        )
    }

    /// The app's code folder: a pop-up menu of recent ones, the one that
    /// looks like the app's, or another.
    fn project_menu(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let theme = cx.theme().clone();
        let panel = cx.entity().downgrade();
        let project = self.project(cx);
        let mut folders: Vec<PathBuf> = project.iter().cloned().collect();
        for folder in self.suggested_project(cx).into_iter().chain(self.recent_projects(cx)) {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        let name = project
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned());
        let shared = name.is_some();
        Button::new("ai-project-menu")
            .ghost()
            .xsmall()
            .dropdown_caret(true)
            .disabled(self.running)
            .tooltip(match &project {
                Some(path) => format!("The AI reads your app's code in {}", tilde(path)),
                None => "Choose your app's project folder, so the AI knows what your app does".to_owned(),
            })
            .child(
                h_flex()
                    .gap(px(5.))
                    .items_center()
                    .child(
                        Icon::new(if shared { IconName::FolderOpen } else { IconName::Folder })
                            .size(px(13.))
                            .text_color(if shared { theme.primary } else { theme.muted_foreground }),
                    )
                    .child(
                        div()
                            .max_w(px(90.))
                            .truncate()
                            .text_color(if shared { theme.foreground } else { theme.muted_foreground })
                            .child(name.unwrap_or_else(|| "Project".to_owned())),
                    ),
            )
            .dropdown_menu_with_anchor(Anchor::BottomLeft, move |mut menu, _, _| {
                menu = menu.min_w(px(240.)).label("Project folder");
                for folder in &folders {
                    let checked = project.as_ref() == Some(folder);
                    let (panel, folder) = (panel.clone(), folder.clone());
                    menu = menu.item(PopupMenuItem::new(tilde(&folder)).checked(checked).on_click(move |_, _, cx| {
                        panel.update(cx, |this, cx| this.set_project(Some(&folder), cx)).ok();
                    }));
                }
                if !folders.is_empty() {
                    menu = menu.separator();
                }
                let chooser = panel.clone();
                menu = menu.item(PopupMenuItem::new("Choose folder…").on_click(move |_, window, cx| {
                    chooser.update(cx, |this, cx| this.choose_project(window, cx)).ok();
                }));
                let panel = panel.clone();
                menu.item(PopupMenuItem::new("Don't share code").checked(project.is_none()).on_click(move |_, _, cx| {
                    panel.update(cx, |this, cx| this.set_project(None, cx)).ok();
                }))
            })
    }

    fn step_row(&self, index: usize, step: &Step, cx: &App) -> Div {
        let theme = cx.theme();
        let (label, color) = match &step.state {
            StepState::Working => (step.doing.clone(), theme.foreground),
            StepState::Skipped => (format!("{} (not allowed here)", step.doing), theme.muted_foreground.opacity(0.7)),
            _ => (step.done.clone(), theme.muted_foreground),
        };
        let icon: AnyElement = match &step.state {
            StepState::Working => Icon::new(IconName::LoaderCircle)
                .size(px(13.))
                .text_color(theme.primary)
                .with_animation(
                    ("ai-spin", index),
                    Animation::new(Duration::from_millis(900)).repeat(),
                    |icon, delta| icon.transform(Transformation::rotate(percentage(delta))),
                )
                .into_any_element(),
            StepState::Failed(_) => Icon::new(IconName::CircleX)
                .size(px(13.))
                .text_color(theme.danger)
                .into_any_element(),
            _ => step
                .glyph
                .icon()
                .size(px(13.))
                .text_color(theme.muted_foreground)
                .into_any_element(),
        };
        let why = match &step.state {
            StepState::Failed(why) => Some(why.clone()),
            _ => None,
        };
        v_flex()
            .child(
                h_flex()
                    .gap(px(7.))
                    .items_center()
                    .text_size(px(12.))
                    .text_color(color)
                    .child(
                        div()
                            .size(px(14.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .child(icon),
                    )
                    .child(div().min_w_0().truncate().child(label)),
            )
            .when_some(why, |this, why| {
                this.child(div().pl(px(21.)).text_size(px(11.)).text_color(theme.danger).child(why))
            })
    }

    /// Steps in a row, as one quiet list; a long one folds its start away.
    fn steps(&self, first: usize, steps: &[(usize, &Step)], cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let folded = steps.len() > STEPS_SHOWN + 1 && !self.unfolded.contains(&first);
        let hidden = if folded { steps.len() - STEPS_SHOWN } else { 0 };
        v_flex()
            .gap(px(5.))
            .py(px(2.))
            .pl(px(2.))
            .when(folded, |this| {
                this.child(
                    h_flex()
                        .id(("ai-unfold", first))
                        .gap(px(7.))
                        .text_size(px(11.5))
                        .text_color(theme.muted_foreground)
                        .cursor_pointer()
                        .hover(|style| style.text_color(theme.foreground))
                        .child(Icon::new(IconName::ChevronRight).size(px(13.)))
                        .child(format!("{hidden} earlier steps"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.unfolded.insert(first);
                            cx.notify();
                        })),
                )
            })
            .children(steps.iter().skip(hidden).map(|(index, step)| self.step_row(*index, step, cx)))
    }

    fn conversation(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let mut column = v_flex().gap(px(12.));
        let mut index = 0;
        while index < self.chat.len() {
            match &self.chat[index] {
                Item::Step(_) => {
                    let first = index;
                    let mut steps = Vec::new();
                    while let Some(Item::Step(step)) = self.chat.get(index) {
                        steps.push((index, step));
                        index += 1;
                    }
                    column = column.child(self.steps(first, &steps, cx));
                    continue;
                }
                Item::Asked(text) => {
                    column = column.child(
                        h_flex().justify_end().pl(px(36.)).child(
                            div()
                                .px(px(11.))
                                .py(px(7.))
                                .rounded(px(14.))
                                .rounded_br(px(4.))
                                .bg(theme.primary)
                                .text_color(theme.primary_foreground)
                                .text_size(px(13.))
                                .child(text.clone()),
                        ),
                    );
                }
                Item::Said { text, shown, .. } => {
                    column = column.child(said_view(
                        &text[..*shown],
                        theme.foreground,
                        theme.muted_foreground,
                        theme.primary,
                    ));
                }
                Item::Ended { text, failed: true } => {
                    column = column.child(
                        h_flex()
                            .items_start()
                            .gap(px(7.))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(9.))
                            .bg(theme.danger.opacity(0.1))
                            .text_size(px(12.))
                            .text_color(theme.danger)
                            .child(Icon::new(IconName::TriangleAlert).size(px(13.)).mt(px(1.)))
                            .child(div().flex_1().min_w_0().child(text.clone())),
                    );
                }
                Item::Ended { text, failed: false } => {
                    column = column.child(
                        h_flex()
                            .justify_center()
                            .text_size(px(11.5))
                            .text_color(theme.muted_foreground)
                            .child(text.clone()),
                    );
                }
            }
            index += 1;
        }
        let busy = matches!(self.chat.last(), Some(Item::Said { open: true, .. }))
            || self
                .chat
                .iter()
                .any(|item| matches!(item, Item::Step(step) if step.state == StepState::Working));
        if self.running && !busy {
            column = column.child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child("Thinking…")
                    .with_animation(
                        "ai-thinking",
                        Animation::new(Duration::from_millis(1400))
                            .repeat()
                            .with_easing(pulsating_between(0.35, 1.)),
                        |label, delta| label.opacity(delta),
                    ),
            );
        }
        column
    }

    fn welcome(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let can_send = self.can_send();
        v_flex()
            .gap(px(8.))
            .pt(px(4.))
            .child(div().text_size(px(12.5)).text_color(theme.muted_foreground).pb(px(6.)).child(
                "Your own AI designs here with AppDoctor's tools. You'll see each change on the canvas as it's \
                 made, and ⌘Z undoes it.",
            ))
            .when_some(
                self.suggested_project(cx).filter(|_| self.project(cx).is_none()),
                |this, folder| {
                    this.child(
                        h_flex()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(8.))
                            .mb(px(6.))
                            .rounded(px(9.))
                            .bg(theme.primary.opacity(0.08))
                            .child(Icon::new(IconName::Folder).size(px(14.)).text_color(theme.primary))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(px(12.))
                                    .child(format!("Your app's code looks to be in {}.", tilde(&folder))),
                            )
                            .child(
                                Button::new("ai-project-use")
                                    .primary()
                                    .xsmall()
                                    .label("Use it")
                                    .on_click(cx.listener(move |this, _, _, cx| this.set_project(Some(&folder), cx))),
                            ),
                    )
                },
            )
            .child(div().text_size(px(12.)).font_weight(FontWeight::MEDIUM).child("Try"))
            .children(self.suggestions(cx).iter().enumerate().map(|(ix, text)| {
                h_flex()
                    .id(("ai-suggestion", ix))
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(9.))
                    .text_size(px(12.))
                    .bg(theme.foreground.opacity(0.04))
                    .when(can_send, |this| {
                        this.cursor_pointer().hover(|style| style.bg(theme.foreground.opacity(0.08)))
                    })
                    .child(
                        Icon::new(gpui_kit::assets::IconName::Sparkles)
                            .size(px(12.))
                            .text_color(theme.primary),
                    )
                    .child(div().flex_1().min_w_0().child(*text))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.can_send() {
                            this.ask(text.to_string(), cx);
                        }
                    }))
            }))
    }

    fn composer(&self, dark: bool, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let action = if self.running {
            Button::new("ai-stop")
                .xsmall()
                .icon(IconName::Square)
                .tooltip("Stop")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("ai-send")
                .primary()
                .xsmall()
                .icon(IconName::ArrowUp)
                .tooltip("Send")
                .disabled(!self.can_send())
                .on_click(cx.listener(|this, _, window, cx| this.send(window, cx)))
        };
        v_flex()
            .px(px(12.))
            .pt(px(10.))
            .pb(px(12.))
            .gap(px(8.))
            .border_t_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .rounded(px(12.))
                    .bg(theme.foreground.opacity(0.045))
                    .border_1()
                    .border_color(theme.foreground.opacity(0.07))
                    .px(px(4.))
                    .pt(px(3.))
                    .pb(px(6.))
                    .child(div().text_size(px(13.)).child(Textarea::new(&self.prompt).appearance(false)))
                    .child(
                        h_flex()
                            .px(px(2.))
                            .gap(px(2.))
                            .items_center()
                            .child(self.project_menu(cx))
                            .child(self.model_menu(dark, cx))
                            .children(self.effort_menu(cx))
                            .child(div().flex_1())
                            .child(action),
                    ),
            )
            .child(
                div()
                    .id("ai-elsewhere")
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .cursor_pointer()
                    .hover(|style| style.text_color(theme.primary))
                    .child("Use AppDoctor from Claude Desktop, Cursor or another app…")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.view
                            .update(cx, |view, cx| view.open_agent_dialog(window, cx))
                            .ok();
                    })),
            )
    }
}

impl Render for AiPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Another app starts a new chat.
        let app = self.app(cx).map(|(id, _)| id);
        if !self.running && self.chat_app.is_some() && self.chat_app != app {
            self.chat.clear();
            self.session = None;
            self.chat_app = None;
        }
        if self.project(cx).is_none() {
            self.look_for_project(cx);
        }
        if self.animate() {
            window.request_animation_frame();
        }

        let theme = cx.theme().clone();
        let dark = theme.mode.is_dark();
        let body = if self.chat.is_empty() { self.welcome(cx) } else { self.conversation(cx) };

        v_flex()
            .w(px(AI_PANEL))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_l_1()
            .border_color(theme.border)
            .child(self.header(cx))
            .child(
                div()
                    .id("ai-chat")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_scroll(cx.listener(|this, event: &ScrollEvent, window, _| {
                        let delta = event.delta.pixel_delta(window.line_height());
                        if f32::from(delta.y) > 0. {
                            // Reading back: stop gliding to the newest.
                            this.following = false;
                        } else {
                            let gap = f32::from(this.scroll.max_offset().y + this.scroll.offset().y);
                            this.following = gap < 40.;
                        }
                    }))
                    .child(div().px(px(12.)).pt(px(12.)).pb(px(16.)).child(body)),
            )
            .child(self.composer(dark, cx))
    }
}
