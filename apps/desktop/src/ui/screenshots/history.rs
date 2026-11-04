//! Version history for the open design. Versions save themselves as you
//! work, when edits pause and before something replaces the design (an
//! agent's edits, a restore); you can name one to keep it. Picking one shows
//! it on the canvas to look at; nothing changes until you restore it, and
//! restoring saves what was there first, so it can be undone too.
//!
//! Versions keep only the design: the images it uses stay in its folder.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::sync::Arc;
use std::time::{Duration, Instant};

use appdoctor_screenshots::{Design, Slide, Video};
use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::view::RIGHT_PANEL;
use super::{COALESCE, ScreenshotsView, render_thumb};
use crate::rank_check::now;
use crate::ui::versions;

/// An autosave waits for edits to pause this long...
const SETTLE: Duration = Duration::from_secs(30);
/// ...and comes no sooner than this after the last version...
const MIN_GAP: Duration = Duration::from_secs(120);
/// ...unless edits have gone on for this long without a pause.
const MAX_GAP: Duration = Duration::from_secs(600);
/// Slides shown for each version.
const STRIP: usize = 5;

struct Version {
    id: i64,
    created_at: i64,
    label: String,
    named: bool,
    slides: Vec<Slide>,
    video: Option<Video>,
    /// Keys into [`History::thumbs`] for the first few slides.
    strip: Vec<u64>,
}

/// A version on the canvas, and what it's standing in for.
pub(super) struct Viewing {
    id: i64,
    slides: Vec<Slide>,
    video: Option<Video>,
    slide: usize,
}

pub(super) struct History {
    pub(super) open: bool,
    versions: Vec<Version>,
    pub(super) viewing: Option<Viewing>,
    /// Versions share most slides, so thumbnails are kept per slide.
    thumbs: HashMap<u64, Arc<Image>>,
    name: Entity<InputState>,
    /// Edited since the last version.
    dirty: bool,
    /// The last edit was an agent's.
    by_agent: bool,
    changed_at: Instant,
    saved_at: Instant,
    _load: Option<Task<()>>,
    _thumbs: Option<Task<()>>,
}

impl History {
    pub(super) fn new(window: &mut Window, cx: &mut Context<ScreenshotsView>) -> (Self, Subscription) {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name this version"));
        let subscription = cx.subscribe_in(&name, window, |this, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.name_version(cx);
            }
        });
        let history = Self {
            open: false,
            versions: Vec::new(),
            viewing: None,
            thumbs: HashMap::new(),
            name,
            dirty: false,
            by_agent: false,
            changed_at: Instant::now(),
            saved_at: Instant::now(),
            _load: None,
            _thumbs: None,
        };
        (history, subscription)
    }

    pub(super) fn note_edit(&mut self, by_agent: bool) {
        self.dirty = true;
        self.by_agent = by_agent;
        self.changed_at = Instant::now();
    }
}

/// Identifies a slide's thumbnail: its own layers and the neighbours' that
/// can span onto it.
fn thumb_key(slides: &[Slide], index: usize) -> u64 {
    let start = index.saturating_sub(1);
    let end = (index + 2).min(slides.len());
    let mut hasher = DefaultHasher::new();
    (index - start).hash(&mut hasher);
    serde_json::to_vec(&slides[start..end])
        .unwrap_or_default()
        .hash(&mut hasher);
    hasher.finish()
}

impl ScreenshotsView {
    fn version_key(&self) -> String {
        self.target.version_key(self.locale.as_deref())
    }

    /// Saves what's there now as a version, unless the newest one is the same.
    pub(super) fn store_version(&mut self, label: &str, named: bool, cx: &mut Context<Self>) {
        let Some(app_id) = self.app_id else {
            return;
        };
        if self.design.slides.is_empty() {
            return;
        }
        let services = self.state.read(cx).services.clone();
        let saved =
            services
                .store
                .save_design_version(app_id, &self.version_key(), &self.design.to_json(), label, named, now());
        if let Err(error) = saved {
            tracing::warn!(%error, "couldn't save a version of the design");
        }
        self.history.dirty = false;
        self.history.by_agent = false;
        self.history.saved_at = Instant::now();
        if self.history.open {
            self.reload_versions(cx);
        }
    }

    /// Run often: saves a version once edits have settled.
    pub(super) fn autosave_version(&mut self, cx: &mut Context<Self>) {
        let history = &self.history;
        if !history.dirty || history.viewing.is_some() || self.drag.is_some() {
            return;
        }
        let since_saved = history.saved_at.elapsed();
        let settled = history.changed_at.elapsed() >= SETTLE && since_saved >= MIN_GAP;
        if settled || since_saved >= MAX_GAP {
            let label = if history.by_agent { "Your AI's edits" } else { "" };
            self.store_version(label, false, cx);
        }
    }

    /// Called as a design opens: changes made while it wasn't open (an agent,
    /// the app quitting before an autosave) become a version.
    pub(super) fn version_opened(&mut self, cx: &mut Context<Self>) {
        self.store_version("", false, cx);
        self.history.versions.clear();
        if self.history.open {
            self.reload_versions(cx);
        }
    }

    /// Called before another design replaces this one on screen.
    pub(super) fn version_leaving(&mut self, cx: &mut Context<Self>) {
        if self.history.dirty {
            self.store_version("", false, cx);
        }
    }

    /// Puts back the design a viewed version was standing in for.
    pub(super) fn stop_viewing(&mut self) -> bool {
        let Some(viewing) = self.history.viewing.take() else {
            return false;
        };
        self.design.slides = viewing.slides;
        self.design.video = viewing.video;
        self.slide = viewing.slide.min(self.design.slides.len().saturating_sub(1));
        true
    }

    pub(super) fn toggle_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.open {
            self.close_history(window, cx);
            return;
        }
        self.history.open = true;
        self.ai_open = false;
        let left = self.left_open;
        self.set_panels(left, true, cx);
        self.reload_versions(cx);
        cx.notify();
    }

    pub(super) fn close_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.history.open = false;
        self.back_to_current(window, cx);
        cx.notify();
    }

    /// Reads the versions, then draws their thumbnails, newest first.
    fn reload_versions(&mut self, cx: &mut Context<Self>) {
        let Some(app_id) = self.app_id else {
            return;
        };
        let services = self.state.read(cx).services.clone();
        let key = self.version_key();
        self.history._load = Some(cx.spawn(async move |this, cx| {
            let versions = cx
                .background_spawn(async move {
                    services
                        .store
                        .design_versions(app_id, &key)
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|version| {
                            let design = Design::from_json(&version.design)?;
                            let shown = design.slides.len().min(STRIP);
                            let strip = (0..shown).map(|ix| thumb_key(&design.slides, ix)).collect();
                            Some(Version {
                                id: version.id,
                                created_at: version.created_at,
                                label: version.label,
                                named: version.named,
                                slides: design.slides,
                                video: design.video,
                                strip,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                this.history.versions = versions;
                this.render_version_thumbs(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    fn render_version_thumbs(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let size = self.target.sizes()[0];
        let mut known: HashSet<u64> = self.history.thumbs.keys().copied().collect();
        let versions: Vec<_> = self
            .history
            .versions
            .iter()
            .map(|version| (version.slides.clone(), version.strip.clone()))
            .collect();
        self.history._thumbs = Some(cx.spawn(async move |this, cx| {
            for (slides, strip) in versions {
                let missing: Vec<(usize, u64)> = strip
                    .into_iter()
                    .enumerate()
                    .filter(|(_, key)| known.insert(*key))
                    .collect();
                if missing.is_empty() {
                    continue;
                }
                let folder = folder.clone();
                let rendered = cx
                    .background_spawn(async move {
                        let clips = appdoctor_video::clip_source();
                        missing
                            .into_iter()
                            .filter_map(|(ix, key)| {
                                Some((key, render_thumb(&slides, ix, size, &folder, clips.as_ref())?))
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                let stored = this.update(cx, |this, cx| {
                    this.history.thumbs.extend(rendered);
                    cx.notify();
                });
                if stored.is_err() {
                    return;
                }
            }
        }));
    }

    /// Shows a version on the canvas. The design underneath stays as it was.
    fn view_version(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self.history.versions.iter().find(|v| v.id == id) else {
            return;
        };
        let (slides, video) = (version.slides.clone(), version.video.clone());
        match &mut self.history.viewing {
            Some(viewing) => viewing.id = id,
            None => {
                self.flush(cx);
                self.history.viewing = Some(Viewing {
                    id,
                    slides: std::mem::take(&mut self.design.slides),
                    video: self.design.video.take(),
                    slide: self.slide,
                });
            }
        }
        self.design.slides = slides;
        self.design.video = video;
        self.slide = 0;
        self.selected = None;
        self.set_preview(false, cx);
        self.sync_controls(window, cx);
        self.thumbs.clear();
        self.render_thumbs(cx);
        cx.notify();
    }

    pub(super) fn back_to_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.stop_viewing() {
            self.selected = None;
            self.set_preview(false, cx);
            self.sync_controls(window, cx);
            self.thumbs.clear();
            self.render_thumbs(cx);
            cx.notify();
        }
    }

    /// Makes a version the design. What was there stays in the history, and
    /// ⌘Z undoes it like any other edit.
    fn restore_version(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self.history.versions.iter().find(|v| v.id == id) else {
            return;
        };
        let (slides, video) = (version.slides.clone(), version.video.clone());
        let when = versions::when(version.created_at);
        let name = if version.named {
            format!("“{}”", version.label)
        } else {
            format!("the version from {when}")
        };
        let label = if version.named {
            format!("Restored “{}”", version.label)
        } else {
            format!("Restored from {when}")
        };
        self.back_to_current(window, cx);
        self.store_version("Before restoring", false, cx);
        self.apply(window, cx, |this| {
            this.design.slides = slides;
            this.design.video = video;
            this.next_id = this.next_id.max(this.design.max_id());
            this.slide = this.slide.min(this.design.slides.len().saturating_sub(1));
            this.selected = None;
        });
        self.last_edit = Instant::now() - COALESCE;
        self.thumbs.clear();
        self.render_thumbs(cx);
        self.store_version(&label, false, cx);
        self.status = Some(Ok(format!("Restored {name}. ⌘Z undoes it.")));
        cx.notify();
    }

    fn delete_version(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        if self.history.viewing.as_ref().is_some_and(|viewing| viewing.id == id) {
            self.back_to_current(window, cx);
        }
        let services = self.state.read(cx).services.clone();
        if let Err(error) = services.store.delete_design_version(id) {
            self.status = Some(Err(format!("Couldn't delete the version: {error}")));
        }
        self.reload_versions(cx);
    }

    /// Keeps the design as it is now under the name in the field.
    fn name_version(&mut self, cx: &mut Context<Self>) {
        let name = self.history.name.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.status = Some(Err("Type a name for the version first.".into()));
            cx.notify();
            return;
        }
        if self.history.viewing.is_some() {
            self.status = Some(Err("Go back to the current design to save it as a version.".into()));
            cx.notify();
            return;
        }
        self.store_version(&name, true, cx);
        self.status = Some(Ok(format!("Saved “{name}”.")));
        cx.notify();
    }

    fn strip(&self, keys: &[u64], cx: &App) -> Div {
        // A row of thumbnails the same height, or width for wide art.
        let (width, height) = self.target.canvas();
        let (thumb_w, thumb_h) = if height >= width {
            (58. * width / height, 58.)
        } else {
            (112., 112. * height / width)
        };
        let theme = cx.theme();
        h_flex().gap(px(3.)).overflow_hidden().children(keys.iter().map(|key| {
            div()
                .w(px(thumb_w))
                .h(px(thumb_h))
                .flex_shrink_0()
                .rounded(px(3.))
                .overflow_hidden()
                .bg(theme.muted)
                .when_some(self.history.thumbs.get(key).cloned(), |this, image| {
                    this.child(img(image).size_full())
                })
        }))
    }

    pub(super) fn history_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let noun = self.slide_noun().to_lowercase();
        let viewing = self.history.viewing.as_ref().map(|viewing| viewing.id);
        let top = {
            let close = cx.listener(|this, _, window, cx| this.close_history(window, cx));
            let save = cx.listener(|this, _, _, cx| this.name_version(cx));
            versions::panel_top(
                "Saved on this computer as you work",
                &self.history.name,
                close,
                save,
                cx,
            )
        };
        let current = versions::row("version-current", viewing.is_none(), cx)
            .child(versions::heading(
                "Current design".into(),
                false,
                "What you're editing".into(),
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| this.back_to_current(window, cx)));

        let rows: Vec<_> = self
            .history
            .versions
            .iter()
            .map(|version| {
                let id = version.id;
                let selected = viewing == Some(id);
                let count = version.slides.len();
                let title = versions::title(&version.label);
                let detail = format!(
                    "{} · {count} {noun}{}",
                    versions::when(version.created_at),
                    if count == 1 { "" } else { "s" }
                );
                versions::row(("version", id as u64), selected, cx)
                    .child(versions::heading(title.clone(), version.named, detail, cx))
                    .child(self.strip(&version.strip, cx))
                    .when(selected, |this| {
                        let restore = cx.listener(move |this, _, window, cx| this.restore_version(id, window, cx));
                        let view = cx.entity().downgrade();
                        let name = title.clone();
                        this.child(versions::actions(Some(restore), move |_, window, cx| {
                            let view = view.clone();
                            versions::confirm_delete(name.clone(), window, cx, move |window, cx| {
                                view.update(cx, |this, cx| this.delete_version(id, window, cx)).ok();
                            });
                        }))
                    })
                    .on_click(cx.listener(move |this, _, window, cx| this.view_version(id, window, cx)))
            })
            .collect();
        let empty = rows.is_empty();

        v_flex()
            .w(px(RIGHT_PANEL))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_l_1()
            .border_color(theme.border)
            .child(top)
            .child(
                v_flex()
                    .id("version-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .gap(px(2.))
                    .child(current)
                    .children(rows)
                    .when(empty, |this| {
                        this.child(
                            div()
                                .px(px(9.))
                                .py(px(10.))
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("Earlier versions appear here as you work."),
                        )
                    }),
            )
    }

    /// In place of the tools while a version is on the canvas.
    pub(super) fn viewing_pill(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let viewing = self.history.viewing.as_ref().map(|viewing| viewing.id);
        let version = self.history.versions.iter().find(|version| Some(version.id) == viewing);
        let label = version.map_or_else(String::new, |version| {
            format!(
                "{}, {}",
                versions::title(&version.label),
                versions::when(version.created_at)
            )
        });
        h_flex()
            .pl(px(14.))
            .pr(px(8.))
            .py(px(7.))
            .gap(px(10.))
            .items_center()
            .rounded(px(18.))
            .border_1()
            .border_color(theme.border.opacity(0.7))
            .bg(theme.popover.opacity(0.95))
            .child(
                v_flex()
                    .child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Viewing an earlier version"),
                    )
                    .child(div().text_size(px(11.)).text_color(theme.muted_foreground).child(label)),
            )
            .child(
                Button::new("back-to-current")
                    .ghost()
                    .small()
                    .label("Back to Current")
                    .on_click(cx.listener(|this, _, window, cx| this.back_to_current(window, cx))),
            )
            .when_some(viewing, |this, id| {
                this.child(
                    Button::new("restore-viewed")
                        .primary()
                        .small()
                        .icon(ExtraIcon::RotateCcw)
                        .label("Restore")
                        .on_click(cx.listener(move |this, _, window, cx| this.restore_version(id, window, cx))),
                )
            })
    }
}
