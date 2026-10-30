//! "Make with AI": the user's own AI (Claude Code, Codex, Gemini CLI or a
//! model in Ollama) designs here with AppDoctor's tools. Nothing new to
//! sign up for: the command-line tools use their own sign-in, and Ollama
//! runs on this Mac. Each change lands in the design file, which the editor
//! watches, so the work appears as it happens.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use appdoctor_ai::{Event, Found, Provider, Task as AiTask};
use appdoctor_screenshots::Target;
use futures::StreamExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ScreenshotsView;
use super::view::RIGHT_PANEL;
use crate::state::AppState;
use crate::ui::controls::{group, heading, separator};

const PROVIDER: &str = "ai_provider";
const MODEL: &str = "ai_model";

pub(super) struct AiPanel {
    state: Entity<AppState>,
    view: WeakEntity<ScreenshotsView>,
    found: Vec<Found>,
    detecting: bool,
    provider: Option<Provider>,
    model: Option<String>,
    prompt: Entity<TextareaState>,
    running: bool,
    cancel: Arc<AtomicBool>,
    log: Vec<Event>,
    scroll: ScrollHandle,
    _run: Option<Task<()>>,
}

impl AiPanel {
    pub(super) fn new(
        state: Entity<AppState>,
        view: WeakEntity<ScreenshotsView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AiPanel {
        let mut panel = AiPanel {
            state,
            view,
            found: Vec::new(),
            detecting: false,
            provider: None,
            model: None,
            prompt: cx.new(|cx| TextareaState::new(window, cx).auto_grow(4, 12)),
            running: false,
            cancel: Arc::new(AtomicBool::new(false)),
            log: Vec::new(),
            scroll: ScrollHandle::new(),
            _run: None,
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
                let models: Vec<String> = found
                    .iter()
                    .find(|f| f.provider == Provider::Ollama)
                    .map(|f| f.models.clone())
                    .unwrap_or_default();
                this.model = this
                    .setting(MODEL, cx)
                    .filter(|m| models.contains(m))
                    .or_else(|| models.first().cloned());
                this.found = found;
                cx.notify();
            })
            .ok();
        })
        .detach();
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
            this.update(cx, |this, cx| {
                if let Some(key) = this.project_key(cx) {
                    this.save(&key, &folder.display().to_string(), cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let prompt = self.prompt.read(cx).value().trim().to_owned();
        let (Some(provider), Some((app_id, app_name))) = (self.provider, self.app(cx)) else {
            return;
        };
        let Some(found) = self.found.iter().find(|f| f.provider == provider).cloned() else {
            return;
        };
        if prompt.is_empty() {
            return;
        }
        let state = self.state.read(cx);
        let data_dir = state.services.data_dir.clone();
        let target = state.screenshot_target;
        let folder = data_dir.join("screenshots").join(app_id.to_string());
        let task = AiTask {
            provider,
            path: found.path,
            model: if provider == Provider::Ollama { self.model.clone() } else { None },
            prompt,
            context: appdoctor_ai::context(&app_name, app_id, target.slug(), &folder),
            launch: super::agent::launch(),
            data_dir,
            project: self.project(cx),
        };
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.log.clear();
        self.running = true;
        let (events, mut received) = futures::channel::mpsc::unbounded::<Event>();
        std::thread::Builder::new()
            .name("ai".into())
            .spawn(move || appdoctor_ai::run(&task, &mut |event| drop(events.unbounded_send(event)), cancel))
            .ok();
        self._run = Some(cx.spawn(async move |this, cx| {
            while let Some(event) = received.next().await {
                let alive = this.update(cx, |this, cx| {
                    this.log.push(event);
                    this.scroll.scroll_to_bottom();
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            this.update(cx, |this, cx| {
                this.running = false;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        self.cancel.store(true, Ordering::Relaxed);
        cx.notify();
    }

    fn suggestions(&self, cx: &App) -> &'static [&'static str] {
        match self.state.read(cx).screenshot_target {
            Target::Iphone | Target::Ipad => &[
                "Make a 20-second app preview of the app's best features, in the aurora theme",
                "Design 5 App Store screenshots with short benefit headlines",
                "Review the video and fix what it finds",
            ],
            Target::Header | Target::Search => &[
                "Make a bold looping header video that shows what the app does",
                "Design a calm, premium header image",
                "Review it and fix what it finds",
            ],
        }
    }

    fn event_row(&self, event: &Event, cx: &App) -> Div {
        let theme = cx.theme();
        let small = |text: String, color: Hsla| div().text_size(px(11.5)).text_color(color).child(text);
        match event {
            Event::Text(text) => div().text_size(px(12.5)).child(text.clone()),
            Event::Tool { name, input } => v_flex()
                .gap(px(1.))
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.primary)
                        .child(Icon::new(gpui_kit::assets::IconName::Sparkles).size(px(11.)))
                        .child(name.replace('_', " ")),
                )
                .when(!input.is_empty() && input != "{}", |this| {
                    this.child(small(input.clone(), theme.muted_foreground))
                }),
            Event::Result { text, failed } => small(
                format!("{} {text}", if *failed { "✕" } else { "✓" }),
                if *failed { theme.danger } else { theme.muted_foreground },
            ),
            Event::Done(text) => v_flex()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(crate::theme::meter::up())
                        .child("Done"),
                )
                .when(!text.is_empty(), |this| this.child(div().text_size(px(12.5)).child(text.clone()))),
            Event::Failed(text) => div().text_size(px(12.5)).text_color(theme.danger).child(text.clone()),
        }
    }
}

impl Render for AiPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let _ = window;
        let selected = self.provider.and_then(|p| self.found.iter().find(|f| f.provider == p)).cloned();
        let project = self.project(cx);
        let can_run = !self.running && selected.as_ref().is_some_and(|f| f.problem.is_none() || f.provider != Provider::Ollama);

        let providers = h_flex().flex_wrap().gap(px(6.)).children(self.found.iter().map(|found| {
            let provider = found.provider;
            let chosen = self.provider == Some(provider);
            div()
                .id(SharedString::from(format!("ai-provider-{}", provider.slug())))
                .px(px(10.))
                .py(px(5.))
                .rounded(px(7.))
                .text_size(px(12.))
                .cursor_pointer()
                .map(|this| {
                    if chosen {
                        this.bg(theme.primary).text_color(theme.primary_foreground)
                    } else {
                        this.bg(theme.foreground.opacity(0.06))
                            .hover(|style| style.bg(theme.foreground.opacity(0.1)))
                    }
                })
                .child(provider.label())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.provider = Some(provider);
                    this.save(PROVIDER, provider.slug(), cx);
                    cx.notify();
                }))
        }));

        let models = selected
            .as_ref()
            .filter(|f| f.provider == Provider::Ollama)
            .map(|f| f.models.clone())
            .unwrap_or_default();

        let setup = group(cx)
            .child(
                v_flex()
                    .p(px(10.))
                    .gap(px(8.))
                    .child(
                        h_flex()
                            .justify_between()
                            .child(div().text_size(px(13.)).child("Your AI"))
                            .child(
                                Button::new("ai-detect")
                                    .ghost()
                                    .xsmall()
                                    .icon(IconName::RefreshCw)
                                    .loading(self.detecting)
                                    .tooltip("Look again")
                                    .on_click(cx.listener(|this, _, _, cx| this.detect(cx))),
                            ),
                    )
                    .when(self.found.is_empty() && !self.detecting, |this| {
                        this.child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("No AI found. Install Claude Code, Codex (in the ChatGPT app), Gemini CLI or Ollama."),
                        )
                    })
                    .child(providers)
                    .when_some(selected.as_ref().and_then(|f| f.problem.clone()), |this, problem| {
                        this.child(div().text_size(px(11.5)).text_color(theme.warning).child(problem))
                    })
                    .when(!models.is_empty(), |this| {
                        this.child(h_flex().flex_wrap().gap(px(5.)).children(models.into_iter().map(|model| {
                            let chosen = self.model.as_deref() == Some(model.as_str());
                            div()
                                .id(SharedString::from(format!("ai-model-{model}")))
                                .px(px(8.))
                                .py(px(3.))
                                .rounded(px(6.))
                                .text_size(px(11.5))
                                .cursor_pointer()
                                .border_1()
                                .border_color(if chosen { theme.primary } else { theme.foreground.opacity(0.1) })
                                .child(model.clone())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.save(MODEL, &model, cx);
                                    this.model = Some(model.clone());
                                    cx.notify();
                                }))
                        })))
                    }),
            )
            .child(separator(cx))
            .child(
                h_flex()
                    .min_h(px(40.))
                    .px(px(10.))
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(div().text_size(px(13.)).child("App's code"))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(project.as_ref().map_or_else(
                                        || "Not shared".to_owned(),
                                        |p| p.file_name().map_or_else(|| p.display().to_string(), |n| n.to_string_lossy().into_owned()),
                                    )),
                            ),
                    )
                    .child(
                        Button::new("ai-project")
                            .xsmall()
                            .label(if project.is_some() { "Change…" } else { "Choose…" })
                            .tooltip("Lets the AI read your app's source to find what's worth showing")
                            .on_click(cx.listener(|this, _, window, cx| this.choose_project(window, cx))),
                    ),
            );

        let suggestions = v_flex().gap(px(5.)).children(self.suggestions(cx).iter().enumerate().map(|(ix, text)| {
            div()
                .id(("ai-suggestion", ix))
                .px(px(9.))
                .py(px(5.))
                .rounded(px(7.))
                .text_size(px(11.5))
                .text_color(theme.muted_foreground)
                .bg(theme.foreground.opacity(0.04))
                .cursor_pointer()
                .hover(|style| style.bg(theme.foreground.opacity(0.08)).text_color(theme.foreground))
                .child(*text)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.prompt
                        .update(cx, |prompt, cx| prompt.set_value(text.to_string(), window, cx));
                }))
        }));

        let action = if self.running {
            Button::new("ai-stop")
                .danger()
                .w_full()
                .label("Stop")
                .on_click(cx.listener(|this, _, _, cx| this.stop(cx)))
        } else {
            Button::new("ai-run")
                .primary()
                .w_full()
                .icon(gpui_kit::assets::IconName::Sparkles)
                .label("Make it")
                .disabled(!can_run)
                .on_click(cx.listener(|this, _, _, cx| this.run(cx)))
        };

        v_flex()
            .w(px(RIGHT_PANEL))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_l_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .id("ai-panel")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .px(px(12.))
                    .pt(px(14.))
                    .pb(px(20.))
                    .gap(px(14.))
                    .child(
                        v_flex()
                            .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child("Make with AI"))
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(theme.muted_foreground)
                                    .child("Your own AI designs here with AppDoctor's tools. Changes appear as it works; ⌘Z undoes them."),
                            ),
                    )
                    .child(setup)
                    .child(
                        v_flex()
                            .gap(px(6.))
                            .child(heading("What should it make?", cx).pb_0())
                            .child(
                                div()
                                    .rounded(px(10.))
                                    .bg(theme.foreground.opacity(0.045))
                                    .border_1()
                                    .border_color(theme.foreground.opacity(0.05))
                                    .px(px(4.))
                                    .py(px(2.))
                                    .text_size(px(13.))
                                    .child(Textarea::new(&self.prompt).appearance(false)),
                            )
                            .child(suggestions),
                    )
                    .child(action)
                    .when(!self.log.is_empty(), |this| {
                        this.child(
                            v_flex()
                                .gap(px(8.))
                                .p(px(10.))
                                .rounded(px(10.))
                                .bg(theme.foreground.opacity(0.03))
                                .children(self.log.iter().map(|event| self.event_row(event, cx)))
                                .when(self.running, |this| {
                                    this.child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(theme.muted_foreground)
                                            .child("Working…"),
                                    )
                                }),
                        )
                    })
                    .child(
                        div()
                            .id("ai-elsewhere")
                            .text_size(px(11.5))
                            .text_color(theme.primary)
                            .cursor_pointer()
                            .child("Use AppDoctor from Claude Desktop, Cursor or another app…")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.view
                                    .update(cx, |view, cx| view.open_agent_dialog(window, cx))
                                    .ok();
                            })),
                    ),
            )
    }
}
