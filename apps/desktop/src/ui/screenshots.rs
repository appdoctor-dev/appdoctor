//! The screenshot designer: App Store screenshots for the selected app,
//! designed in AppDoctor and exported at Apple's sizes.
//!
//! The preview is rendered by the same code as the export
//! (`appdoctor_screenshots::render_png`), just smaller, so it can't drift
//! from what gets uploaded. Designs live in the data folder, one per app:
//! `screenshots/<app id>/design.json` beside the imported images.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use appdoctor_screenshots::{Background, CanvasSize, Design, Frame, Slide, render_png};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;
use crate::theme::meter;
use crate::ui::widgets::{self, card, status_dot};

/// Preview rendering scale: the preview shows about 0.24× the canvas, and
/// twice that keeps it sharp on Retina displays.
const PREVIEW_SCALE: f32 = 0.48;
const THUMB_SCALE: f32 = 0.1;

/// One-click backgrounds and the text colours that read on them.
const PRESETS: &[(&str, &str, &str, &str)] = &[
    ("Indigo", "#4f46e5", "#9333ea", "#ffffff"),
    ("Sunset", "#f97316", "#db2777", "#ffffff"),
    ("Ocean", "#06b6d4", "#2563eb", "#ffffff"),
    ("Mint", "#34d399", "#059669", "#ffffff"),
    ("Midnight", "#0f172a", "#334155", "#ffffff"),
    ("Paper", "#f8fafc", "#e2e8f0", "#0f172a"),
];

struct Inputs {
    headline: Entity<InputState>,
    subheadline: Entity<InputState>,
    headline_color: Entity<InputState>,
    subheadline_color: Entity<InputState>,
    from: Entity<InputState>,
    to: Entity<InputState>,
}

pub struct ScreenshotsView {
    state: Entity<AppState>,
    app_id: Option<u64>,
    design: Design,
    selected: usize,
    preview: Option<Arc<Image>>,
    thumbs: Vec<Option<Arc<Image>>>,
    inputs: Inputs,
    status: Option<Result<String, String>>,
    exporting: bool,
    /// While the inputs are being filled from a slide, their change events
    /// aren't edits.
    syncing: bool,
    _render: Option<Task<()>>,
    _thumbs: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl ScreenshotsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_owned();
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let inputs = Inputs {
            headline: input("Headline", window, cx),
            subheadline: input("Subheadline", window, cx),
            headline_color: input("#ffffff", window, cx),
            subheadline_color: input("#e0e7ff", window, cx),
            from: input("#4f46e5", window, cx),
            to: input("#9333ea", window, cx),
        };
        let mut subscriptions = vec![cx.observe_in(&state, window, |this, _, window, cx| this.follow_app(window, cx))];
        for field in [
            &inputs.headline,
            &inputs.subheadline,
            &inputs.headline_color,
            &inputs.subheadline_color,
            &inputs.from,
            &inputs.to,
        ] {
            subscriptions.push(cx.subscribe(field, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && !this.syncing {
                    this.apply_inputs(cx);
                }
            }));
        }
        let mut this = Self {
            state,
            app_id: None,
            design: Design::default(),
            selected: 0,
            preview: None,
            thumbs: Vec::new(),
            inputs,
            status: None,
            exporting: false,
            syncing: false,
            _render: None,
            _thumbs: None,
            _subscriptions: subscriptions,
        };
        this.follow_app(window, cx);
        this
    }

    fn folder(&self, cx: &App) -> Option<PathBuf> {
        let app = self.app_id?;
        Some(
            self.state
                .read(cx)
                .services
                .data_dir
                .join("screenshots")
                .join(app.to_string()),
        )
    }

    /// Loads the selected app's design when the selection changes.
    fn follow_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let selected = state.selected_app;
        if selected == self.app_id {
            return;
        }
        let app_name = state.selected().map(|app| app.name.clone());
        self.app_id = selected;
        self.selected = 0;
        self.status = None;
        self.design = match self.folder(cx) {
            Some(folder) => Design::load(&folder.join("design.json")),
            None => Design::default(),
        };
        if self.design.slides.is_empty() && selected.is_some() {
            let mut first = Slide::default();
            if let Some(name) = app_name {
                first.headline.text = name;
            }
            self.design.slides.push(first);
        }
        self.sync_inputs(window, cx);
        self.render_all(cx);
    }

    fn slide(&self) -> Option<&Slide> {
        self.design.slides.get(self.selected)
    }

    fn slide_mut(&mut self) -> Option<&mut Slide> {
        self.design.slides.get_mut(self.selected)
    }

    /// Puts the selected slide's values in the inputs.
    fn sync_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(slide) = self.slide().cloned() else {
            return;
        };
        let (from, to) = match &slide.background {
            Background::Solid { color } => (color.clone(), color.clone()),
            Background::Gradient { from, to, .. } => (from.clone(), to.clone()),
        };
        self.syncing = true;
        for (input, value) in [
            (&self.inputs.headline, slide.headline.text),
            (&self.inputs.subheadline, slide.subheadline.text),
            (&self.inputs.headline_color, slide.headline.color),
            (&self.inputs.subheadline_color, slide.subheadline.color),
            (&self.inputs.from, from),
            (&self.inputs.to, to),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.syncing = false;
    }

    /// Takes edits from the inputs into the selected slide.
    fn apply_inputs(&mut self, cx: &mut Context<Self>) {
        let read = |input: &Entity<InputState>, cx: &App| input.read(cx).value().to_string();
        let headline = read(&self.inputs.headline, cx);
        let subheadline = read(&self.inputs.subheadline, cx);
        let headline_color = read(&self.inputs.headline_color, cx);
        let subheadline_color = read(&self.inputs.subheadline_color, cx);
        let from = read(&self.inputs.from, cx);
        let to = read(&self.inputs.to, cx);
        let Some(slide) = self.slide_mut() else {
            return;
        };
        slide.headline.text = headline;
        slide.subheadline.text = subheadline;
        slide.headline.color = headline_color;
        slide.subheadline.color = subheadline_color;
        slide.background = match &slide.background {
            Background::Solid { .. } => Background::Solid { color: from },
            Background::Gradient { angle, .. } => Background::Gradient {
                from,
                to,
                angle: *angle,
            },
        };
        self.changed(cx);
    }

    /// An edit happened: save and re-render (debounced).
    fn changed(&mut self, cx: &mut Context<Self>) {
        self.status = None;
        if let Some(folder) = self.folder(cx)
            && let Err(error) = self.design.save(&folder.join("design.json"))
        {
            self.status = Some(Err(format!("Couldn't save the design: {error}")));
        }
        self.render_selected(true, cx);
        cx.notify();
    }

    fn render_selected(&mut self, debounce: bool, cx: &mut Context<Self>) {
        let (Some(slide), Some(folder)) = (self.slide().cloned(), self.folder(cx)) else {
            self.preview = None;
            return;
        };
        let index = self.selected;
        self._render = Some(cx.spawn(async move |this, cx| {
            if debounce {
                cx.background_executor().timer(Duration::from_millis(120)).await;
            }
            let (preview, thumb) = cx
                .background_spawn(async move {
                    (
                        render_image(&slide, &folder, PREVIEW_SCALE),
                        render_image(&slide, &folder, THUMB_SCALE),
                    )
                })
                .await;
            this.update(cx, |this, cx| {
                this.preview = preview;
                if let Some(slot) = this.thumbs.get_mut(index) {
                    *slot = thumb;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn render_all(&mut self, cx: &mut Context<Self>) {
        self.thumbs = vec![None; self.design.slides.len()];
        self.render_selected(false, cx);
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let slides = self.design.slides.clone();
        self._thumbs = Some(cx.spawn(async move |this, cx| {
            let thumbs = cx
                .background_spawn(async move {
                    slides
                        .iter()
                        .map(|slide| render_image(slide, &folder, THUMB_SCALE))
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                if this.thumbs.len() == thumbs.len() {
                    this.thumbs = thumbs;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn select(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.design.slides.len() {
            self.selected = index;
            self.sync_inputs(window, cx);
            self.render_selected(false, cx);
            cx.notify();
        }
    }

    fn add_slide(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let mut slide = self.slide().cloned().unwrap_or_default();
        if !duplicate {
            slide.headline.text = "Another great reason".into();
            slide.subheadline.text = "Say what this screen shows".into();
            slide.device.screenshot = None;
        }
        let at = (self.selected + 1).min(self.design.slides.len());
        self.design.slides.insert(at, slide);
        self.thumbs.insert(at, None);
        self.selected = at;
        self.sync_inputs(window, cx);
        self.changed(cx);
    }

    fn delete_slide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.design.slides.len() <= 1 {
            return;
        }
        self.design.slides.remove(self.selected);
        self.thumbs.remove(self.selected);
        self.selected = self.selected.min(self.design.slides.len() - 1);
        self.sync_inputs(window, cx);
        self.changed(cx);
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Slide)) {
        if let Some(slide) = self.slide_mut() {
            f(slide);
        }
        self.sync_inputs(window, cx);
        self.changed(cx);
    }

    fn apply_to_all(&mut self, cx: &mut Context<Self>) {
        let Some(style) = self.slide().cloned() else {
            return;
        };
        for slide in &mut self.design.slides {
            slide.background = style.background.clone();
            slide.headline.color = style.headline.color.clone();
            slide.headline.size = style.headline.size;
            slide.subheadline.color = style.subheadline.color.clone();
            slide.subheadline.size = style.subheadline.size;
            slide.device.frame = style.device.frame;
            slide.device.scale = style.device.scale;
            slide.device.offset = style.device.offset;
        }
        self.changed(cx);
        self.render_all(cx);
    }

    fn choose_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use Screenshot".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            let copied = import_image(&source, &folder);
            this.update_in(cx, |this, window, cx| match copied {
                Ok(name) => this.edit(window, cx, |slide| slide.device.screenshot = Some(name)),
                Err(error) => {
                    this.status = Some(Err(format!("Couldn't use that image: {error}")));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Export Here".into()),
        });
        let slides = self.design.slides.clone();
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(target) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.exporting = true;
                cx.notify();
            })
            .ok();
            let destination = target.clone();
            let result = cx
                .background_spawn(async move {
                    let mut written = 0;
                    for size in CanvasSize::ALL {
                        for (index, slide) in slides.iter().enumerate() {
                            let png = render_png(slide, size, &folder, 1.0).map_err(|e| e.to_string())?;
                            let name = format!("{:02}-{}.png", index + 1, size.slug());
                            std::fs::write(destination.join(name), png).map_err(|e| e.to_string())?;
                            written += 1;
                        }
                    }
                    Ok::<_, String>(written)
                })
                .await;
            this.update(cx, |this, cx| {
                this.exporting = false;
                this.status = Some(result.map(|count| format!("Exported {count} PNGs to {}", target.display())));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn choice<T: PartialEq + Copy + 'static>(
        &self,
        id: &'static str,
        options: &[(&'static str, T)],
        current: T,
        cx: &mut Context<Self>,
        apply: fn(&mut Slide, T),
    ) -> impl IntoElement {
        h_flex()
            .gap_1()
            .children(options.iter().enumerate().map(|(ix, (label, value))| {
                let value = *value;
                Button::new((id, ix))
                    .xsmall()
                    .label(*label)
                    .when(current == value, |button| button.primary())
                    .when(current != value, |button| button.outline())
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.edit(window, cx, |slide| apply(slide, value))),
                    )
            }))
    }

    fn inspector(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(slide) = self.slide().cloned() else {
            return div().into_any_element();
        };
        let theme = cx.theme().clone();
        let label = |text: &str| {
            div()
                .text_xs()
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.muted_foreground)
                .child(text.to_owned())
        };
        let gradient = matches!(slide.background, Background::Gradient { .. });
        let angle = match slide.background {
            Background::Gradient { angle, .. } => angle as i32,
            Background::Solid { .. } => 180,
        };
        let swatches = PRESETS.iter().enumerate().map(|(ix, (name, from, to, text))| {
            let (from, to, text) = (from.to_string(), to.to_string(), text.to_string());
            let swatch = gradient_fill(&from, &to);
            div()
                .id(("preset", ix))
                .size(px(26.))
                .rounded_full()
                .cursor_pointer()
                .border_1()
                .border_color(theme.border)
                .bg(swatch)
                .tooltip(move |window, cx| gpui_kit::component::tooltip::Tooltip::new(*name).build(window, cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let (from, to, text) = (from.clone(), to.clone(), text.clone());
                    this.edit(window, cx, move |slide| {
                        let angle = match slide.background {
                            Background::Gradient { angle, .. } => angle,
                            Background::Solid { .. } => 160.,
                        };
                        slide.background = Background::Gradient { from, to, angle };
                        slide.headline.color = text.clone();
                        slide.subheadline.color = text;
                    })
                }))
        });

        v_flex()
            .id("screenshot-inspector")
            .w(px(300.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .border_l_1()
            .border_color(theme.border)
            .p_4()
            .gap_3()
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Text"))
            .child(label("Headline"))
            .child(Input::new(&self.inputs.headline).small())
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.inputs.headline_color).small()))
                    .child(
                        Button::new("headline-smaller")
                            .xsmall()
                            .outline()
                            .label("A−")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |s| s.headline.size = (s.headline.size - 8.).max(48.))
                            })),
                    )
                    .child(
                        Button::new("headline-larger")
                            .xsmall()
                            .outline()
                            .label("A+")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.edit(window, cx, |s| s.headline.size = (s.headline.size + 8.).min(180.))
                            })),
                    ),
            )
            .child(label("Subheadline"))
            .child(Input::new(&self.inputs.subheadline).small())
            .child(Input::new(&self.inputs.subheadline_color).small())
            .child(
                div()
                    .pt_2()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Background"),
            )
            .child(h_flex().gap_2().children(swatches))
            .child(self.choice(
                "bg-kind",
                &[("Gradient", true), ("Solid", false)],
                gradient,
                cx,
                |slide, gradient| {
                    slide.background = match (&slide.background, gradient) {
                        (Background::Solid { color }, true) => Background::Gradient {
                            from: color.clone(),
                            to: color.clone(),
                            angle: 160.,
                        },
                        (Background::Gradient { from, .. }, false) => Background::Solid { color: from.clone() },
                        (background, _) => background.clone(),
                    };
                },
            ))
            .child(
                h_flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&self.inputs.from).small()))
                    .when(gradient, |this| {
                        this.child(div().flex_1().child(Input::new(&self.inputs.to).small()))
                    }),
            )
            .when(gradient, |this| {
                this.child(self.choice(
                    "angle",
                    &[("↓", 180), ("↘", 135), ("→", 90), ("↗", 45)],
                    angle,
                    cx,
                    |slide, angle| {
                        if let Background::Gradient { angle: current, .. } = &mut slide.background {
                            *current = angle as f32;
                        }
                    },
                ))
            })
            .child(div().pt_2().text_sm().font_weight(FontWeight::SEMIBOLD).child("Device"))
            .child(
                Button::new("choose-screenshot")
                    .small()
                    .outline()
                    .icon(IconName::FolderOpen)
                    .label(if slide.device.screenshot.is_some() {
                        "Replace screenshot…"
                    } else {
                        "Choose screenshot…"
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.choose_screenshot(window, cx))),
            )
            .child(label("Frame"))
            .child(self.choice(
                "frame",
                &[
                    ("Black", Frame::Black),
                    ("Silver", Frame::Silver),
                    ("None", Frame::None),
                ],
                slide.device.frame,
                cx,
                |slide, frame| slide.device.frame = frame,
            ))
            .child(label("Size"))
            .child(self.choice(
                "scale",
                &[("S", 80), ("M", 90), ("L", 100)],
                (slide.device.scale * 100.).round() as i32,
                cx,
                |slide, scale| slide.device.scale = scale as f32 / 100.,
            ))
            .child(label("Position"))
            .child(self.choice(
                "offset",
                &[("Higher", -4), ("Default", 0), ("Lower", 6)],
                (slide.device.offset * 100.).round() as i32,
                cx,
                |slide, offset| slide.device.offset = offset as f32 / 100.,
            ))
            .child(
                Button::new("apply-all")
                    .small()
                    .ghost()
                    .label("Use this style on every slide")
                    .on_click(cx.listener(|this, _, _, cx| this.apply_to_all(cx))),
            )
            .into_any_element()
    }
}

fn render_image(slide: &Slide, folder: &Path, scale: f32) -> Option<Arc<Image>> {
    match render_png(slide, CanvasSize::Iphone69, folder, scale) {
        Ok(png) => Some(Arc::new(Image::from_bytes(ImageFormat::Png, png))),
        Err(error) => {
            tracing::warn!(%error, "screenshot render failed");
            None
        }
    }
}

/// Copies an image into the design folder under a fresh name and returns it.
fn import_image(source: &Path, folder: &Path) -> std::io::Result<String> {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp"))
        .ok_or_else(|| std::io::Error::other("choose a PNG, JPEG or WebP image"))?;
    std::fs::create_dir_all(folder)?;
    let name = format!("shot-{}.{extension}", uuid::Uuid::new_v4().simple());
    std::fs::copy(source, folder.join(&name))?;
    Ok(name)
}

fn parse_hex(color: &str) -> Option<Hsla> {
    let hex = color.trim().trim_start_matches('#');
    let value = u32::from_str_radix(hex, 16).ok()?;
    (hex.len() == 6).then(|| rgb(value).into())
}

fn gradient_fill(from: &str, to: &str) -> Background_ {
    let (a, b) = (parse_hex(from).unwrap_or(black()), parse_hex(to).unwrap_or(black()));
    linear_gradient(160., linear_color_stop(a, 0.), linear_color_stop(b, 1.))
}

type Background_ = gpui_kit::Background;

impl Render for ScreenshotsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        let theme = cx.theme().clone();
        let canvas_h = px(620.);
        let canvas_w = canvas_h * (1320. / 2868.);

        let strip = v_flex()
            .id("slide-strip")
            .w(px(150.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.border)
            .p_3()
            .gap_3()
            .items_center()
            .children(self.design.slides.iter().enumerate().map(|(ix, _)| {
                let active = ix == self.selected;
                let thumb = self.thumbs.get(ix).cloned().flatten();
                v_flex()
                    .id(("slide-thumb", ix))
                    .gap_1()
                    .items_center()
                    .cursor_pointer()
                    .child(
                        div()
                            .w(px(104.))
                            .h(px(226.))
                            .rounded(px(8.))
                            .overflow_hidden()
                            .border_2()
                            .border_color(if active { theme.primary } else { theme.border })
                            .bg(theme.muted)
                            .when_some(thumb, |this, image| this.child(img(image).size_full())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{}", ix + 1)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.select(ix, window, cx)))
            }))
            .child(
                Button::new("add-slide")
                    .small()
                    .outline()
                    .icon(IconName::Plus)
                    .label("Slide")
                    .on_click(cx.listener(|this, _, window, cx| this.add_slide(false, window, cx))),
            );

        let toolbar = h_flex()
            .h(px(60.))
            .flex_shrink_0()
            .px_5()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Screenshots"))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
                        "{} · {} slide{} · exports {} and {}",
                        app.name,
                        self.design.slides.len(),
                        if self.design.slides.len() == 1 { "" } else { "s" },
                        CanvasSize::Iphone69.label(),
                        CanvasSize::Iphone65.label()
                    ))),
            )
            .child(div().flex_1())
            .when_some(self.status.clone(), |this, status| {
                let (color, text) = match status {
                    Ok(text) => (meter::up(), text),
                    Err(text) => (theme.danger, text),
                };
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .max_w(px(420.))
                        .text_xs()
                        .child(status_dot(color))
                        .child(div().truncate().text_color(color).child(text)),
                )
            })
            .child(
                Button::new("duplicate-slide")
                    .small()
                    .outline()
                    .label("Duplicate")
                    .on_click(cx.listener(|this, _, window, cx| this.add_slide(true, window, cx))),
            )
            .child(
                Button::new("delete-slide")
                    .small()
                    .ghost()
                    .label("Delete")
                    .disabled(self.design.slides.len() <= 1)
                    .on_click(cx.listener(|this, _, window, cx| this.delete_slide(window, cx))),
            )
            .child(
                Button::new("export-shots")
                    .small()
                    .primary()
                    .icon(IconName::ArrowDown)
                    .label("Export PNGs")
                    .loading(self.exporting)
                    .disabled(self.exporting || self.design.slides.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
            );

        let canvas = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.muted.opacity(0.35))
            .child(
                card(cx).p_2().child(
                    div()
                        .w(canvas_w)
                        .h(canvas_h)
                        .rounded(px(10.))
                        .overflow_hidden()
                        .bg(theme.muted)
                        .when_some(self.preview.clone(), |this, image| this.child(img(image).size_full())),
                ),
            );

        v_flex()
            .size_full()
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(strip)
                    .child(canvas)
                    .child(self.inspector(cx)),
            )
            .into_any_element()
    }
}
