//! "AI agent": connects the user's own AI apps (Claude, Cursor, Codex,
//! anything that speaks MCP) to the designer through `appdoctor mcp`, a
//! local stdio server that edits the same design files this view watches.
//! One click adds AppDoctor to each installed app's MCP settings.

use appdoctor_mcp::clients::{Client, Launch, Status, installed_clients};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ScreenshotsView;
use crate::theme::meter;
use crate::ui::controls::{group, heading, separator};

/// How an AI app starts AppDoctor's server: this executable with `mcp`,
/// plus the data folder when it isn't the default.
pub(super) fn launch() -> Launch {
    Launch {
        command: std::env::current_exe()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|_| "appdoctor".into()),
        args: vec!["mcp".into()],
        env: std::env::var("APPDOCTOR_DATA_DIR")
            .map(|dir| vec![("APPDOCTOR_DATA_DIR".into(), dir)])
            .unwrap_or_default(),
    }
}

/// The settings to paste into an app AppDoctor can't set up itself.
fn settings_json(launch: &Launch) -> String {
    let mut server = serde_json::json!({ "command": launch.command, "args": launch.args });
    if !launch.env.is_empty() {
        let env: serde_json::Map<_, _> = launch
            .env
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::json!(value)))
            .collect();
        server["env"] = serde_json::Value::Object(env);
    }
    serde_json::to_string_pretty(&serde_json::json!({ "mcpServers": { "appdoctor": server } })).unwrap_or_default()
}

/// The installed AI apps and whether each is connected.
struct Connections {
    launch: Launch,
    clients: Vec<(Client, Result<Status, String>)>,
    message: Option<Result<String, String>>,
}

impl Connections {
    fn new() -> Connections {
        let mut this = Connections {
            launch: launch(),
            clients: Vec::new(),
            message: None,
        };
        this.refresh();
        this
    }

    fn refresh(&mut self) {
        let (Some(home), Some(config)) = (dirs::home_dir(), dirs::config_dir()) else {
            return;
        };
        self.clients = installed_clients(&home, &config)
            .into_iter()
            .map(|client| {
                let status = client.status(&self.launch);
                (client, status)
            })
            .collect();
    }

    fn connect(&mut self, which: Vec<usize>, cx: &mut Context<Self>) {
        let (mut connected, mut notes, mut errors) = (Vec::new(), Vec::new(), Vec::new());
        for ix in which {
            let Some((client, _)) = self.clients.get(ix) else {
                continue;
            };
            match client.connect(&self.launch) {
                Ok(()) => {
                    connected.push(client.name);
                    if !client.note.is_empty() && !notes.contains(&client.note) {
                        notes.push(client.note);
                    }
                }
                Err(error) => errors.push(format!("{}: {error}", client.name)),
            }
        }
        self.refresh();
        self.message = Some(if errors.is_empty() {
            Ok(format!("Connected {}. {}", list(&connected), notes.join(" ")))
        } else {
            Err(errors.join(" "))
        });
        cx.notify();
    }

    fn disconnect(&mut self, ix: usize, cx: &mut Context<Self>) {
        if let Some((client, _)) = self.clients.get(ix) {
            let name = client.name;
            self.message = Some(client.disconnect().map(|()| format!("Removed AppDoctor from {name}.")));
            self.refresh();
            cx.notify();
        }
    }

    fn row(&self, ix: usize, client: &Client, status: &Result<Status, String>, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let control = match status {
            Ok(Status::Connected) => h_flex()
                .gap_2()
                .items_center()
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .text_size(px(12.))
                        .text_color(meter::up())
                        .child(Icon::new(IconName::Check).size(px(13.)))
                        .child("Connected"),
                )
                .child(
                    Button::new(("disconnect", ix))
                        .ghost()
                        .xsmall()
                        .label("Remove")
                        .on_click(cx.listener(move |this, _, _, cx| this.disconnect(ix, cx))),
                )
                .into_any_element(),
            Ok(status) => Button::new(("connect", ix))
                .small()
                .when(*status == Status::Outdated, |button| button.primary())
                .label(if *status == Status::Outdated { "Update" } else { "Connect" })
                .tooltip(if *status == Status::Outdated {
                    "Connected to another copy of AppDoctor; point it at this one"
                } else {
                    "Add AppDoctor to its MCP servers"
                })
                .on_click(cx.listener(move |this, _, _, cx| this.connect(vec![ix], cx)))
                .into_any_element(),
            Err(error) => {
                let error = SharedString::from(error.clone());
                div()
                    .id(("client-error", ix))
                    .text_size(px(12.))
                    .text_color(theme.warning)
                    .child("Set up by hand")
                    .tooltip(move |window, cx| Tooltip::new(error.clone()).build(window, cx))
                    .into_any_element()
            }
        };
        h_flex()
            .min_h(px(44.))
            .px(px(10.))
            .gap_2()
            .items_center()
            .justify_between()
            .child(
                v_flex()
                    .min_w_0()
                    .child(div().text_size(px(13.)).child(client.name))
                    .when(!client.note.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(client.note),
                        )
                    }),
            )
            .child(control)
    }
}

/// "A", "A and B", "A, B and C".
fn list(names: &[&str]) -> String {
    match names {
        [] => "nothing".into(),
        [one] => (*one).into(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

impl Render for Connections {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pending: Vec<usize> = self
            .clients
            .iter()
            .enumerate()
            .filter(|(_, (_, status))| matches!(status, Ok(Status::NotConnected | Status::Outdated)))
            .map(|(ix, _)| ix)
            .collect();
        let rows: Vec<Div> = self
            .clients
            .iter()
            .enumerate()
            .map(|(ix, (client, status))| self.row(ix, client, status, cx))
            .collect();
        let count = rows.len();

        v_flex()
            .gap(px(8.))
            .child(
                h_flex()
                    .items_end()
                    .justify_between()
                    .child(heading("Your AI apps", cx).pb_0())
                    .when(count > 0, |this| {
                        this.child(
                            Button::new("connect-all")
                                .small()
                                .primary()
                                .icon(gpui_kit::assets::IconName::Sparkles)
                                .label(match pending.len() {
                                    0 => "All connected",
                                    n if n == count => "Connect all",
                                    _ => "Connect the rest",
                                })
                                .disabled(pending.is_empty())
                                .on_click(cx.listener(move |this, _, _, cx| this.connect(pending.clone(), cx))),
                        )
                    }),
            )
            .child(if count == 0 {
                group(cx)
                    .p(px(12.))
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child(
                        "No AI apps found here. AppDoctor sets up Claude, Claude Code, Cursor, VS Code, Windsurf, \
                         Codex, Gemini CLI, Antigravity and LM Studio; for anything else, paste the settings below.",
                    )
            } else {
                rows.into_iter().enumerate().fold(group(cx), |group, (ix, row)| {
                    if ix == 0 {
                        group.child(row)
                    } else {
                        group.child(separator(cx)).child(row)
                    }
                })
            })
            .when_some(self.message.clone(), |this, message| {
                let (color, text) = match message {
                    Ok(text) => (meter::up(), text),
                    Err(text) => (theme.danger, text),
                };
                this.child(div().px(px(4.)).text_size(px(12.)).text_color(color).child(text))
            })
            .child(
                div()
                    .px(px(4.))
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child("Only AppDoctor's entry in each app's settings changes, and the first change keeps a copy of the file."),
            )
    }
}

fn snippet(id: &'static str, text: String, cx: &App) -> Div {
    let theme = cx.theme();
    h_flex()
        .gap_2()
        .items_start()
        .p(px(10.))
        .rounded(px(8.))
        .bg(theme.foreground.opacity(0.06))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_x_hidden()
                .text_size(px(11.5))
                .font_family(theme.mono_font_family.clone())
                .child(text.clone()),
        )
        .child(Clipboard::new(id).value(text).tooltip("Copy"))
}

impl ScreenshotsView {
    pub(super) fn open_agent_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let connections = cx.new(|_| Connections::new());
        let manual = settings_json(&connections.read(cx).launch);
        let app = self
            .state
            .read(cx)
            .selected()
            .map(|app| app.name.clone())
            .unwrap_or_else(|| "my app".into());
        window.open_dialog(cx, move |dialog, window, cx| {
            let theme = cx.theme();
            // Long paths make tall snippets: the body scrolls in small windows.
            let max_height = (f32::from(window.viewport_size().height) - 220.).max(240.);
            dialog
                .title("Design with your AI agent")
                .w(px(560.))
                .child(
                    v_flex()
                        .id("agent-dialog-body")
                        .max_h(px(max_height))
                        .overflow_y_scroll()
                        .pr(px(4.))
                        .gap(px(18.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .text_color(theme.muted_foreground)
                                .child(
                                    "Your AI agent can build these screenshots: add slides and layers, write captions, \
                                     pick devices, finishes and fonts, drop in your app's screens, look at what it made \
                                     and export. Its changes appear here as it works, and ⌘Z undoes them.",
                                ),
                        )
                        .child(connections.clone())
                        .child(
                            v_flex()
                                .gap(px(6.))
                                .child(heading("Then ask, for example", cx).pb_0())
                                .child(
                                    div()
                                        .px(px(4.))
                                        .text_size(px(13.))
                                        .italic()
                                        .child(format!(
                                            "“Design five App Store screenshots for {app}: short benefit headlines in \
                                             Avenir Next, the Sunset background, and an orange iPhone with my screens \
                                             from ~/Desktop/shots. Show me each one.”"
                                        )),
                                ),
                        )
                        .child(
                            v_flex()
                                .gap(px(6.))
                                .child(heading("Another app", cx).pb_0())
                                .child(
                                    div()
                                        .px(px(4.))
                                        .text_size(px(12.))
                                        .text_color(theme.muted_foreground)
                                        .child("Add this to its MCP servers."),
                                )
                                .child(snippet("copy-mcp-json", manual.clone(), cx)),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child(
                                    "The server runs on this Mac and only reads and writes your screenshot designs. \
                                     Nothing goes to AppDoctor; your agent's own provider sees what you ask it.",
                                ),
                        ),
                )
                .footer(
                    h_flex().w_full().justify_end().child(
                        Button::new("agent-done")
                            .label("Done")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    ),
                )
        });
    }
}
