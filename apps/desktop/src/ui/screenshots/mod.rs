//! The screenshot designer: App Store screenshots for the selected app,
//! made of layers you drag, resize and edit on a live canvas.
//!
//! The canvas is drawn natively by GPUI from the design (no image round
//! trip), so dragging and typing stay at frame rate. Export renders the
//! same layers through `appdoctor_screenshots` with the same geometry.
//! Designs live in the data folder: `screenshots/<app id>/design.json` for
//! iPhone and `design-ipad.json` for iPad, beside the images they use.
//!
//! An AI agent can edit the same files through `appdoctor mcp`; the view
//! watches the file and takes in changes it didn't write, as an undoable
//! step.

mod agent;
mod clips;
mod inspector;
mod view;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use appdoctor_screenshots::edit::{Corner, Guide, resize, snap_move};
use appdoctor_screenshots::{
    Align, Background, CanvasSize, DEFAULT_FONT, Design, DeviceModel, Drift, Effect, Frame, Layer, LayerKind, Rect,
    SUGGESTED_FONTS, Slide, Target, Transition, Video, font_families, import_image, import_media, motion,
    render_png_with, write_atomic,
};
use gpui_kit::component::input::{InputEvent, InputState, TextareaState};
use gpui_kit::component::select::{SearchableVec, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::slider::{SliderEvent, SliderState};
use gpui_kit::*;

use crate::state::AppState;
use clips::ClipFrames;

/// Undo steps kept.
const HISTORY: usize = 100;
/// Edits closer together than this undo as one step (typing, sliders).
const COALESCE: Duration = Duration::from_millis(700);
/// Snapping distance in screen pixels.
const SNAP_PX: f32 = 7.;
const THUMB_SCALE: f32 = 0.08;
/// How often the design file is checked for an agent's edits.
const WATCH: Duration = Duration::from_millis(700);

/// A font family in the picker, drawn in its own typeface.
#[derive(Clone)]
struct FontItem {
    name: SharedString,
}

impl SelectItem for FontItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.name.clone()
    }

    fn display_title(&self) -> Option<AnyElement> {
        Some(div().font_family(self.name.clone()).child(self.name.clone()).into_any_element())
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().font_family(self.name.clone()).child(self.name.clone())
    }

    fn value(&self) -> &SharedString {
        &self.name
    }
}

type FontSelect = SelectState<SearchableVec<FontItem>>;

/// A device model in the picker; its value is the model's slug.
#[derive(Clone)]
struct ModelItem {
    slug: SharedString,
    label: SharedString,
}

impl SelectItem for ModelItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.label.clone()
    }

    fn value(&self) -> &SharedString {
        &self.slug
    }
}

type ModelSelect = SelectState<SearchableVec<ModelItem>>;

/// The installed fonts, suggested ones first.
fn font_items() -> Vec<FontItem> {
    let all = font_families();
    let mut names: Vec<String> = SUGGESTED_FONTS
        .iter()
        .filter(|name| all.iter().any(|family| family == *name))
        .map(|name| (*name).to_owned())
        .collect();
    names.extend(all.into_iter().filter(|family| !SUGGESTED_FONTS.contains(&family.as_str())));
    names.into_iter().map(|name| FontItem { name: name.into() }).collect()
}

#[derive(Clone, Copy)]
enum DragMode {
    Move,
    Resize(Corner),
}

#[derive(Clone, Copy)]
struct Drag {
    layer: u64,
    mode: DragMode,
    origin: Point<Pixels>,
    start: Rect,
    scale: f32,
    moved: bool,
}

struct Inputs {
    text: Entity<TextareaState>,
    color: Entity<InputState>,
    from: Entity<InputState>,
    to: Entity<InputState>,
    /// X, Y, W, H of the selected layer.
    geometry: [Entity<InputState>; 4],
}

struct Sliders {
    size: Entity<SliderState>,
    opacity: Entity<SliderState>,
    radius: Entity<SliderState>,
    /// The scene's length, and the selected layer's start and leave times.
    scene: Entity<SliderState>,
    start: Entity<SliderState>,
    leave: Entity<SliderState>,
    /// The preview's playhead.
    time: Entity<SliderState>,
}

pub struct ScreenshotsView {
    state: Entity<AppState>,
    focus: FocusHandle,
    app_id: Option<u64>,
    /// iPhone or iPad: which of the app's designs is open.
    target: Target,
    design: Design,
    /// The design file's bytes as last read or written here; anything else
    /// on disk was written by someone else (an agent).
    last_disk: Vec<u8>,
    next_id: u64,
    slide: usize,
    selected: Option<u64>,
    hovered: Option<u64>,
    drag: Option<Drag>,
    guides: Vec<Guide>,
    /// User zoom on top of fit-to-window.
    zoom: f32,
    left_open: bool,
    right_open: bool,
    /// Bumped on each panel toggle so its slide animation replays.
    panel_epoch: usize,
    /// The floating tools fade out while a layer is being dragged.
    chrome_hidden: bool,
    chrome_epoch: usize,
    /// The canvas scale last drawn at (screen px per canvas px).
    scale: f32,
    stage_scroll: ScrollHandle,
    undo: Vec<Vec<Slide>>,
    redo: Vec<Vec<Slide>>,
    last_edit: Instant,
    thumbs: Vec<Option<Arc<Image>>>,
    inputs: Inputs,
    sliders: Sliders,
    fonts: Entity<FontSelect>,
    models: Entity<ModelSelect>,
    /// While controls are filled from the selection, their events aren't edits.
    syncing: bool,
    status: Option<Result<String, String>>,
    exporting: bool,
    /// Showing the video as it plays, rather than the slides to edit.
    preview: bool,
    playing: bool,
    /// The playhead, in seconds.
    time: f32,
    clips: ClipFrames,
    _ticker: Option<Task<()>>,
    _clips: Task<()>,
    _save: Option<Task<()>>,
    _thumbs: Option<Task<()>>,
    _watch: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl ScreenshotsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_owned();
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let inputs = Inputs {
            text: cx.new(|cx| TextareaState::new(window, cx).auto_grow(2, 6)),
            color: input("#ffffff", window, cx),
            from: input("#4f46e5", window, cx),
            to: input("#9333ea", window, cx),
            geometry: [(); 4].map(|_| input("0", window, cx)),
        };
        let slider = |min: f32, max: f32, cx: &mut Context<Self>| {
            cx.new(|_| SliderState::new().min(min).max(max).step(1.).default_value(min))
        };
        let seconds = |max: f32, step: f32, cx: &mut Context<Self>| {
            cx.new(|_| SliderState::new().min(0.).max(max).step(step).default_value(0.))
        };
        let sliders = Sliders {
            size: slider(16., 240., cx),
            opacity: slider(0., 100., cx),
            radius: slider(0., 300., cx),
            scene: cx.new(|_| SliderState::new().min(1.).max(12.).step(0.5).default_value(3.)),
            // Shares of the scene's length, which differs per scene.
            start: seconds(1., 0.01, cx),
            leave: seconds(1., 0.01, cx),
            // A share of the video's length, which changes as scenes do.
            time: seconds(1., 0.001, cx),
        };
        let (clip_frames, clips_task) = ClipFrames::new(cx);

        let fonts = cx.new(|cx| FontSelect::new(SearchableVec::new(Vec::new()), None, window, cx).searchable(true));
        let model_items: Vec<ModelItem> = DeviceModel::all()
            .map(|model| ModelItem {
                slug: model.slug().into(),
                label: model.label().into(),
            })
            .collect();
        let models = cx.new(|cx| ModelSelect::new(SearchableVec::new(model_items), None, window, cx).searchable(true));
        cx.spawn_in(window, async move |this, cx| {
            let items = cx.background_spawn(async { font_items() }).await;
            this.update_in(cx, |this, window, cx| {
                this.fonts
                    .update(cx, |select, cx| select.set_items(SearchableVec::new(items), window, cx));
                this.sync_font(window, cx);
            })
            .ok();
        })
        .detach();
        let watch = cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(WATCH).await;
                if this.update_in(cx, |this, window, cx| this.check_disk(window, cx)).is_err() {
                    break;
                }
            }
        });

        let mut subscriptions = vec![
            cx.observe_in(&state, window, |this, _, window, cx| this.follow_app(window, cx)),
            cx.subscribe_in(&fonts, window, |this, _, event: &SelectEvent<_>, window, cx| {
                if let SelectEvent::Confirm(Some(name)) = event
                    && !this.syncing
                {
                    this.set_font(name.to_string(), window, cx);
                }
            }),
            cx.subscribe_in(&models, window, |this, _, event: &SelectEvent<_>, window, cx| {
                if let SelectEvent::Confirm(Some(slug)) = event
                    && !this.syncing
                    && let Some(model) = DeviceModel::from_slug(slug)
                {
                    this.set_model(model, window, cx);
                }
            }),
        ];
        subscriptions.push(cx.subscribe(&inputs.text, |this, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && !this.syncing {
                let text = this.inputs.text.read(cx).value().to_string();
                this.edit_selected(cx, |layer| {
                    if let LayerKind::Text { text: current, .. } = &mut layer.kind {
                        *current = text.clone();
                    }
                    layer.name = text.lines().next().unwrap_or("Text").chars().take(24).collect();
                    layer.fit_text();
                });
            }
        }));
        subscriptions.push(cx.subscribe(&inputs.color, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) && !this.syncing {
                let color = input.read(cx).value().trim().to_owned();
                if appdoctor_screenshots::parse_color(&color).is_some() {
                    this.edit_selected(cx, |layer| match &mut layer.kind {
                        LayerKind::Text { color: c, .. } => *c = color.clone(),
                        LayerKind::Shape { fill, .. } => *fill = color.clone(),
                        _ => {}
                    });
                }
            }
        }));
        for (input, end) in [(&inputs.from, 0), (&inputs.to, 1)] {
            subscriptions.push(cx.subscribe(input, move |this, input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) && !this.syncing {
                    let color = input.read(cx).value().trim().to_owned();
                    if appdoctor_screenshots::parse_color(&color).is_some() {
                        this.edit_background(cx, |background| match background {
                            Background::Solid { color: c } => *c = color.clone(),
                            Background::Gradient { from, to, .. } => {
                                *(if end == 0 { from } else { to }) = color.clone();
                            }
                        });
                    }
                }
            }));
        }
        for (which, input) in inputs.geometry.iter().enumerate() {
            subscriptions.push(
                cx.subscribe_in(input, window, move |this, input, event: &InputEvent, window, cx| {
                    if !matches!(event, InputEvent::Change) || this.syncing {
                        return;
                    }
                    let Ok(value) = input.read(cx).value().trim().parse::<f32>() else {
                        return;
                    };
                    this.edit_selected(cx, |layer| {
                        let mut rect = layer.rect();
                        let aspect = rect.height / rect.width.max(1.);
                        match which {
                            0 => rect.x = value,
                            1 => rect.y = value,
                            2 => {
                                rect.width = value.max(1.);
                                if layer.keeps_aspect() {
                                    rect.height = rect.width * aspect;
                                }
                            }
                            _ => {
                                rect.height = value.max(1.);
                                if layer.keeps_aspect() {
                                    rect.width = rect.height / aspect;
                                }
                            }
                        }
                        layer.set_rect(rect);
                    });
                    // A phone's other side follows; text refits its height.
                    this.sync_geometry(window, cx);
                }),
            );
        }
        subscriptions.push(cx.subscribe(&sliders.scene, |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Change(value) = event
                && !this.syncing
            {
                let length = value.start();
                this.checkpoint_coalesced();
                if let Some(slide) = this.current_mut() {
                    slide.duration = length;
                }
                this.changed(cx);
            }
        }));
        for (slider, leaving) in [(&sliders.start, false), (&sliders.leave, true)] {
            subscriptions.push(cx.subscribe(slider, move |this, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                if this.syncing {
                    return;
                }
                let scene = this.current().map_or(motion::DEFAULT_SCENE, |s| s.duration);
                // To a tenth of a second, as the inspector shows it.
                let seconds = (value.start() * scene * 10.).round() / 10.;
                this.edit_selected(cx, |layer| {
                    if leaving {
                        // All the way along means it stays to the end.
                        layer.motion.leave = (seconds < scene - 0.05).then_some(seconds);
                    } else {
                        layer.motion.start = seconds;
                    }
                });
            }));
        }
        subscriptions.push(cx.subscribe(&sliders.time, |this, _, event: &SliderEvent, cx| {
            if let SliderEvent::Change(value) = event
                && !this.syncing
            {
                this.time = value.start() * this.duration();
                this.playing = false;
                this._ticker = None;
                cx.notify();
            }
        }));
        for (slider, which) in [(&sliders.size, 0), (&sliders.opacity, 1), (&sliders.radius, 2)] {
            subscriptions.push(cx.subscribe(slider, move |this, _, event: &SliderEvent, cx| {
                let SliderEvent::Change(value) = event else {
                    return;
                };
                if this.syncing {
                    return;
                }
                let value = value.start();
                this.edit_selected(cx, |layer| match which {
                    0 => {
                        if let LayerKind::Text { size, .. } = &mut layer.kind {
                            *size = value;
                        }
                        layer.fit_text();
                    }
                    1 => layer.opacity = value / 100.,
                    _ => match &mut layer.kind {
                        LayerKind::Image { radius, .. } | LayerKind::Shape { radius, .. } => *radius = value,
                        _ => {}
                    },
                });
            }));
        }

        let mut this = Self {
            state,
            focus: cx.focus_handle(),
            app_id: None,
            target: Target::Iphone,
            design: Design::default(),
            last_disk: Vec::new(),
            next_id: 0,
            slide: 0,
            selected: None,
            hovered: None,
            drag: None,
            guides: Vec::new(),
            zoom: 1.,
            left_open: true,
            right_open: true,
            panel_epoch: 0,
            chrome_hidden: false,
            chrome_epoch: 0,
            scale: 0.2,
            stage_scroll: ScrollHandle::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: Instant::now() - COALESCE,
            thumbs: Vec::new(),
            inputs,
            sliders,
            fonts,
            models,
            syncing: false,
            status: None,
            exporting: false,
            preview: false,
            playing: false,
            time: 0.,
            clips: clip_frames,
            _ticker: None,
            _clips: clips_task,
            _save: None,
            _thumbs: None,
            _watch: watch,
            _subscriptions: subscriptions,
        };
        this.follow_app(window, cx);
        this
    }

    // Loading and saving

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

    fn design_path(&self, cx: &App) -> Option<PathBuf> {
        Some(self.folder(cx)?.join(self.target.file_name()))
    }

    /// Opens the selected app's design for the target the sidebar picked.
    fn follow_app(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let (app, target) = (state.selected_app, state.screenshot_target);
        if (app, target) == (self.app_id, self.target) {
            return;
        }
        self.flush(cx);
        self.app_id = app;
        self.target = target;
        self.load(window, cx);
    }

    /// What one slide is called: screenshots form a set, while header and
    /// search art are alternatives of one asset.
    fn slide_noun(&self) -> &'static str {
        if self.design.video.is_some() {
            "Scene"
        } else if self.target.is_screenshot() {
            "Screenshot"
        } else {
            "Version"
        }
    }

    /// Opens the selected app's design for the current target, starting
    /// one if there's none yet.
    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app_name = self
            .state
            .read(cx)
            .selected()
            .map(|app| app.name.clone())
            .unwrap_or_default();
        let target = self.target;
        self.design = match self.folder(cx) {
            Some(folder) => Design::load_for(&folder, target),
            None => Design {
                target,
                ..Design::default()
            },
        };
        self.last_disk = self
            .design_path(cx)
            .and_then(|path| std::fs::read(path).ok())
            .unwrap_or_default();
        self.next_id = self.design.max_id();
        let started = self.design.slides.is_empty() && self.app_id.is_some();
        if started {
            let slide = Slide::starter(target, &app_name, &mut self.next_id);
            self.design.slides.push(slide);
        }
        self.slide = 0;
        self.selected = None;
        self.undo.clear();
        self.redo.clear();
        self.status = None;
        self.thumbs.clear();
        self.preview = false;
        self.playing = false;
        self._ticker = None;
        self.time = 0.;
        self.sync_controls(window, cx);
        self.render_thumbs(cx);
        // A new design is saved straight away, so an agent sees what you see.
        if started {
            self.changed(cx);
        }
        cx.notify();
    }

    /// Saves and refreshes the edited slide's thumbnail shortly after edits stop.
    fn changed(&mut self, cx: &mut Context<Self>) {
        if self.folder(cx).is_none() {
            return;
        }
        self._save = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(350)).await;
            let Ok(Some((path, bytes))) = this.update(cx, |this, cx| this.prepare_save(cx)) else {
                return;
            };
            let result = cx.background_spawn(async move { write_atomic(&path, &bytes) }).await;
            this.update(cx, |this, cx| {
                if let Err(error) = result {
                    this.status = Some(Err(format!("Couldn't save the design: {error}")));
                }
                this.render_thumbs(cx);
            })
            .ok();
        }));
        cx.notify();
    }

    /// The path and bytes to save, noted as this view's own so the watcher
    /// doesn't take them for an agent's. `None` when the file changed on
    /// disk since it was last read: the watcher takes that in first, so an
    /// agent's edit is never overwritten.
    fn prepare_save(&mut self, cx: &App) -> Option<(PathBuf, Vec<u8>)> {
        let path = self.design_path(cx)?;
        if !self.last_disk.is_empty()
            && let Ok(disk) = std::fs::read(&path)
            && disk != self.last_disk
        {
            return None;
        }
        let bytes = self.design.to_json();
        self.last_disk = bytes.clone();
        Some((path, bytes))
    }

    /// Writes a pending save now, before another design replaces this one.
    fn flush(&mut self, cx: &mut Context<Self>) {
        if self._save.take().is_some()
            && let Some((path, bytes)) = self.prepare_save(cx)
            && let Err(error) = write_atomic(&path, &bytes)
        {
            tracing::warn!(%error, "couldn't save the design");
        }
    }

    /// Takes in edits made to the design file elsewhere (by an agent through
    /// `appdoctor mcp`) as one undoable step.
    fn check_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            return;
        }
        let Some(path) = self.design_path(cx) else {
            return;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            return;
        };
        if bytes == self.last_disk {
            return;
        }
        let Some(disk) = Design::from_json(&bytes) else {
            return;
        };
        self.last_disk = bytes;
        if disk.slides == self.design.slides {
            return;
        }
        self._save = None;
        self.checkpoint();
        self.design.slides = disk.slides;
        self.next_id = self.next_id.max(self.design.max_id());
        self.slide = self.slide.min(self.design.slides.len().saturating_sub(1));
        if self.selected.is_some_and(|id| self.layer(id).is_none()) {
            self.selected = None;
        }
        self.last_edit = Instant::now() - COALESCE;
        self.status = Some(Ok("Updated by your AI agent. ⌘Z undoes it.".into()));
        self.sync_controls(window, cx);
        self.thumbs.clear();
        self.render_thumbs(cx);
        cx.notify();
    }

    /// Re-renders the thumbnails. All of them: a layer spanning two slides
    /// changes both.
    fn render_thumbs(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        self.thumbs.resize(self.design.slides.len(), None);
        let slides = self.design.slides.clone();
        let size = self.target.sizes()[0];
        self._thumbs = Some(cx.spawn(async move |this, cx| {
            let rendered = cx
                .background_spawn(async move {
                    let clip_source = appdoctor_video::clip_source();
                    (0..slides.len())
                        .map(|ix| render_thumb(&slides, ix, size, &folder, clip_source.as_ref()))
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                if this.thumbs.len() == rendered.len() {
                    this.thumbs = rendered;
                }
                cx.notify();
            })
            .ok();
        }));
    }

    // History

    fn checkpoint(&mut self) {
        let snapshot = self.design.slides.clone();
        if self.undo.last() != Some(&snapshot) {
            self.undo.push(snapshot);
            if self.undo.len() > HISTORY {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
    }

    /// A checkpoint unless the previous edit was just now (typing, sliding).
    fn checkpoint_coalesced(&mut self) {
        if self.last_edit.elapsed() > COALESCE {
            self.checkpoint();
        }
        self.last_edit = Instant::now();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.design.slides, previous));
            self.after_history(window, cx);
        }
    }

    fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.design.slides, next));
            self.after_history(window, cx);
        }
    }

    fn after_history(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.slide = self.slide.min(self.design.slides.len().saturating_sub(1));
        if self.selected.is_some_and(|id| self.layer(id).is_none()) {
            self.selected = None;
        }
        self.last_edit = Instant::now() - COALESCE;
        self.sync_controls(window, cx);
        self.thumbs.clear();
        self.render_thumbs(cx);
        self.changed(cx);
    }

    // Access

    fn current(&self) -> Option<&Slide> {
        self.design.slides.get(self.slide)
    }

    fn current_mut(&mut self) -> Option<&mut Slide> {
        self.design.slides.get_mut(self.slide)
    }

    fn layer(&self, id: u64) -> Option<&Layer> {
        self.current()?.layers.iter().find(|layer| layer.id == id)
    }

    fn layer_mut(&mut self, id: u64) -> Option<&mut Layer> {
        self.current_mut()?.layers.iter_mut().find(|layer| layer.id == id)
    }

    fn selected_layer(&self) -> Option<&Layer> {
        self.selected.and_then(|id| self.layer(id))
    }

    fn mint_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    // Editing

    fn edit_selected(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Layer)) {
        let Some(id) = self.selected else {
            return;
        };
        self.checkpoint_coalesced();
        if let Some(layer) = self.layer_mut(id) {
            f(layer);
        }
        self.changed(cx);
    }

    fn edit_background(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Background)) {
        self.checkpoint_coalesced();
        if let Some(slide) = self.current_mut() {
            f(&mut slide.background);
        }
        self.changed(cx);
    }

    /// A one-off edit (a button): its own undo step, and controls refreshed.
    fn apply(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Self)) {
        self.checkpoint();
        f(self);
        self.last_edit = Instant::now() - COALESCE;
        self.sync_controls(window, cx);
        self.changed(cx);
    }

    fn select(&mut self, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected != id {
            self.selected = id;
            self.last_edit = Instant::now() - COALESCE;
            self.sync_controls(window, cx);
        }
        cx.notify();
    }

    /// Fills the property controls from the selection (or the slide).
    fn sync_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        let set = |state: &Entity<InputState>, value: String, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |input, cx| input.set_value(value, window, cx));
        };
        let set_slider = |state: &Entity<SliderState>, value: f32, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |slider, cx| slider.set_value(value, window, cx));
        };
        self.sync_geometry(window, cx);
        self.syncing = true;
        if let Some(layer) = self.selected_layer().cloned() {
            set_slider(&self.sliders.opacity, layer.opacity * 100., window, cx);
            match &layer.kind {
                LayerKind::Text { text, size, color, .. } => {
                    let text = text.clone();
                    self.sync_font(window, cx);
                    self.inputs.text.update(cx, |area, cx| area.set_value(text, window, cx));
                    set(&self.inputs.color, color.clone(), window, cx);
                    set_slider(&self.sliders.size, *size, window, cx);
                }
                LayerKind::Shape { fill, radius } => {
                    set(&self.inputs.color, fill.clone(), window, cx);
                    set_slider(&self.sliders.radius, *radius, window, cx);
                }
                LayerKind::Image { radius, .. } => set_slider(&self.sliders.radius, *radius, window, cx),
                LayerKind::Device { model, .. } => {
                    let slug = SharedString::from(model.slug());
                    self.models
                        .update(cx, |select, cx| select.set_selected_value(&slug, window, cx));
                }
            }
        }
        if let Some(scene) = self.current().map(|slide| slide.duration) {
            set_slider(&self.sliders.scene, scene, window, cx);
            if let Some(layer) = self.selected_layer().cloned() {
                let share = |seconds: f32| (seconds / scene.max(0.1)).clamp(0., 1.);
                set_slider(&self.sliders.start, share(layer.motion.start), window, cx);
                set_slider(&self.sliders.leave, share(layer.motion.leave.unwrap_or(scene)), window, cx);
            }
        }
        if let Some(background) = self.current().map(|slide| slide.background.clone()) {
            let (from, to) = match background {
                Background::Solid { color } => (color.clone(), color),
                Background::Gradient { from, to, .. } => (from, to),
            };
            set(&self.inputs.from, from, window, cx);
            set(&self.inputs.to, to, window, cx);
        }
        self.syncing = false;
    }

    /// Shows the selected text layer's font in the picker.
    fn sync_font(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let font = match self.selected_layer().map(|layer| &layer.kind) {
            Some(LayerKind::Text { font, .. }) => font.clone().unwrap_or_else(|| DEFAULT_FONT.into()),
            _ => return,
        };
        let syncing = std::mem::replace(&mut self.syncing, true);
        self.fonts.update(cx, |select, cx| {
            select.set_selected_value(&SharedString::from(font), window, cx)
        });
        self.syncing = syncing;
    }

    /// Fills X/Y/W/H from the selection, leaving alone the one being typed in.
    fn sync_geometry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(layer) = self.selected_layer() else {
            return;
        };
        let values = [layer.x, layer.y, layer.width, layer.height];
        self.syncing = true;
        for (input, value) in self.inputs.geometry.clone().iter().zip(values) {
            if !input.read(cx).focus_handle(cx).is_focused(window) {
                input.update(cx, |input, cx| input.set_value(format!("{value:.0}"), window, cx));
            }
        }
        self.syncing = false;
    }

    // Layers

    fn add_layer(&mut self, layer: Layer, window: &mut Window, cx: &mut Context<Self>) {
        let id = layer.id;
        self.apply(window, cx, |this| {
            if let Some(slide) = this.current_mut() {
                slide.layers.push(layer);
            }
        });
        self.select(Some(id), window, cx);
    }

    fn add_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.mint_id();
        let (canvas_width, canvas_height) = self.target.canvas();
        let size = 88. * canvas_width / 1320.;
        let layer = Layer::text(self.target, id, "New text", canvas_height * 0.42, size, 700, "#ffffff");
        self.add_layer(layer, window, cx);
        self.inputs.text.update(cx, |area, cx| area.focus(window, cx));
    }

    fn add_device(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.mint_id();
        let mut layer = Layer::device(self.target, id, self.target.canvas().1 * 0.2);
        let count = self
            .current()
            .map_or(0, |s| s.layers.iter().filter(|l| l.label() == "Device").count());
        if count > 0 {
            // A second phone: smaller and offset, so both are visible.
            let rect = Rect {
                x: layer.x + count as f32 * 120.,
                y: layer.y + count as f32 * 60.,
                width: layer.width * 0.85,
                height: layer.height * 0.85,
            };
            layer.set_rect(rect);
        }
        self.add_layer(layer, window, cx);
    }

    fn add_shape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.mint_id();
        self.add_layer(Layer::shape(self.target, id), window, cx);
    }

    fn pick_image(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        use_it: fn(&mut Self, String, (f32, f32), &mut Window, &mut Context<Self>),
    ) {
        self.pick_file(false, window, cx, use_it);
    }

    /// Picks an image, or with `videos` a screen recording too, and copies
    /// it into the design's folder. Recordings report no size.
    fn pick_file(
        &mut self,
        videos: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
        use_it: fn(&mut Self, String, (f32, f32), &mut Window, &mut Context<Self>),
    ) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use Image".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(source) = paths.into_iter().next() else {
                return;
            };
            let name = source.to_string_lossy().into_owned();
            let imported = if videos && appdoctor_screenshots::is_video(&name) {
                import_media(&source, &folder).map(|name| (name, (0., 0.)))
            } else {
                import_image(&source, &folder)
            };
            this.update_in(cx, |this, window, cx| match imported {
                Ok((name, size)) => use_it(this, name, size, window, cx),
                Err(error) => {
                    this.status = Some(Err(format!("Couldn't use that image: {error}")));
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    fn add_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_image(window, cx, |this, name, (w, h), window, cx| {
            // Fit into 60% of the canvas width, keeping the image's shape.
            let width = (this.target.canvas().0 * 0.6).min(w.max(1.));
            let height = width * h / w.max(1.);
            let id = this.mint_id();
            this.add_layer(Layer::image(this.target, id, name, width, height), window, cx);
        });
    }

    /// Sets the selected device's (or the slide's first device's) screenshot.
    /// Sets the selected device's (or the slide's first device's) screen:
    /// a screenshot, or a screen recording that plays in videos.
    fn choose_screenshot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_file(true, window, cx, |this, name, _, window, cx| {
            let target = this
                .selected_layer()
                .filter(|l| matches!(l.kind, LayerKind::Device { .. }))
                .or_else(|| {
                    this.current()
                        .and_then(|s| s.layers.iter().find(|l| matches!(l.kind, LayerKind::Device { .. })))
                })
                .map(|l| l.id);
            let Some(target) = target else {
                this.status = Some(Err("Add a device first.".into()));
                cx.notify();
                return;
            };
            this.apply(window, cx, |this| {
                if let Some(layer) = this.layer_mut(target)
                    && let LayerKind::Device { screenshot, .. } = &mut layer.kind
                {
                    *screenshot = Some(name);
                }
            });
        });
    }

    fn replace_image(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.pick_image(window, cx, |this, name, (w, h), window, cx| {
            this.apply(window, cx, |this| {
                if let Some(id) = this.selected
                    && let Some(layer) = this.layer_mut(id)
                    && let LayerKind::Image { file, .. } = &mut layer.kind
                {
                    *file = Some(name);
                    layer.height = layer.width * h / w.max(1.);
                }
            });
        });
    }

    fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else {
            return;
        };
        self.apply(window, cx, |this| {
            if let Some(slide) = this.current_mut() {
                slide.layers.retain(|layer| layer.id != id);
            }
        });
        self.select(None, window, cx);
    }

    fn duplicate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut copy) = self.selected_layer().cloned() else {
            return;
        };
        let original = copy.id;
        copy.id = self.mint_id();
        copy.x += 40.;
        copy.y += 40.;
        copy.locked = false;
        let id = copy.id;
        self.apply(window, cx, |this| {
            if let Some(slide) = this.current_mut() {
                // Directly above the original.
                let at = slide
                    .layers
                    .iter()
                    .position(|l| l.id == original)
                    .map_or(slide.layers.len(), |ix| ix + 1);
                slide.layers.insert(at, copy);
            }
        });
        self.select(Some(id), window, cx);
    }

    /// Moves the selected layer `steps` places up (positive) or down the stack;
    /// `to_end` goes all the way.
    fn restack(&mut self, steps: isize, to_end: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else {
            return;
        };
        self.apply(window, cx, |this| {
            let Some(slide) = this.current_mut() else {
                return;
            };
            let Some(from) = slide.layers.iter().position(|l| l.id == id) else {
                return;
            };
            let last = slide.layers.len() - 1;
            let to = match (to_end, steps > 0) {
                (true, true) => last,
                (true, false) => 0,
                _ => (from as isize + steps).clamp(0, last as isize) as usize,
            };
            let layer = slide.layers.remove(from);
            slide.layers.insert(to, layer);
        });
    }

    fn toggle(&mut self, id: u64, hidden: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(window, cx, |this| {
            if let Some(layer) = this.layer_mut(id) {
                if hidden {
                    layer.hidden = !layer.hidden;
                } else {
                    layer.locked = !layer.locked;
                }
            }
        });
    }

    fn nudge(&mut self, dx: f32, dy: f32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected else {
            return;
        };
        if self.layer(id).is_some_and(|l| l.locked) {
            return;
        }
        self.checkpoint_coalesced();
        if let Some(layer) = self.layer_mut(id) {
            layer.x += dx;
            layer.y += dy;
        }
        self.sync_geometry(window, cx);
        self.changed(cx);
    }

    // Slides

    fn select_slide(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index < self.design.slides.len() && index != self.slide {
            self.slide = index;
            self.selected = None;
            // +1: the stage's leading spacer is its first child.
            self.stage_scroll.scroll_to_item(index + 1);
            self.sync_controls(window, cx);
            cx.notify();
        }
    }

    fn add_slide(&mut self, duplicate: bool, window: &mut Window, cx: &mut Context<Self>) {
        let slide = if duplicate {
            let mut copy = self
                .current()
                .cloned()
                .unwrap_or_else(|| Slide::starter(self.target, "", &mut self.next_id));
            for layer in &mut copy.layers {
                layer.id = {
                    self.next_id += 1;
                    self.next_id
                };
            }
            copy
        } else {
            let mut slide = Slide::starter(self.target, "Another great reason", &mut self.next_id);
            if let Some(background) = self.current().map(|s| s.background.clone()) {
                slide.background = background;
            }
            slide
        };
        let at = (self.slide + 1).min(self.design.slides.len());
        self.apply(window, cx, |this| this.design.slides.insert(at, slide));
        self.thumbs.insert(at.min(self.thumbs.len()), None);
        self.slide = at;
        self.selected = None;
        self.sync_controls(window, cx);
        self.render_thumbs(cx);
    }

    fn delete_slide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.design.slides.len() <= 1 {
            return;
        }
        let at = self.slide;
        self.apply(window, cx, |this| {
            this.design.slides.remove(at);
        });
        if at < self.thumbs.len() {
            self.thumbs.remove(at);
        }
        self.slide = at.min(self.design.slides.len() - 1);
        self.selected = None;
        self.sync_controls(window, cx);
    }

    // Canvas interaction

    fn begin_drag(
        &mut self,
        id: u64,
        mode: DragMode,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.select(Some(id), window, cx);
        let Some(layer) = self.layer(id) else {
            return;
        };
        if layer.locked {
            return;
        }
        self.drag = Some(Drag {
            layer: id,
            mode,
            origin,
            start: layer.rect(),
            scale: self.scale,
            moved: false,
        });
    }

    fn drag_to(&mut self, position: Point<Pixels>, modifiers: Modifiers, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut drag) = self.drag else {
            return;
        };
        let dx = f32::from(position.x - drag.origin.x) / drag.scale;
        let dy = f32::from(position.y - drag.origin.y) / drag.scale;
        if !drag.moved {
            if dx.abs() < 1. && dy.abs() < 1. {
                return;
            }
            // The first real movement is one undo step for the whole drag.
            self.checkpoint();
            drag.moved = true;
            self.drag = Some(drag);
            self.set_chrome_hidden(true);
        }
        let keeps_aspect = self.layer(drag.layer).is_some_and(Layer::keeps_aspect);
        let others: Vec<Rect> = self
            .current()
            .map(|slide| {
                slide
                    .layers
                    .iter()
                    .filter(|l| l.id != drag.layer && !l.hidden)
                    .map(Layer::rect)
                    .collect()
            })
            .unwrap_or_default();
        let (rect, guides) = match drag.mode {
            DragMode::Move if modifiers.alt => (
                Rect {
                    x: drag.start.x + dx,
                    y: drag.start.y + dy,
                    ..drag.start
                },
                Vec::new(),
            ),
            DragMode::Move => snap_move(
                drag.start,
                dx,
                dy,
                self.target.canvas(),
                &others,
                SNAP_PX / drag.scale,
            ),
            DragMode::Resize(corner) => (
                resize(drag.start, corner, dx, dy, keeps_aspect || modifiers.shift),
                Vec::new(),
            ),
        };
        self.guides = guides;
        if let Some(layer) = self.layer_mut(drag.layer) {
            layer.set_rect(rect);
        }
        self.sync_geometry(window, cx);
        cx.notify();
    }

    fn end_drag(&mut self, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.take() {
            self.guides.clear();
            self.set_chrome_hidden(false);
            if drag.moved {
                self.adopt_by_neighbour(drag.layer);
                self.last_edit = Instant::now() - COALESCE;
                self.changed(cx);
            }
            cx.notify();
        }
    }

    /// A layer dropped with its centre over another slide moves to that
    /// slide (it still spans back onto this one if it overlaps).
    fn adopt_by_neighbour(&mut self, id: u64) {
        let Some(layer) = self.layer(id).cloned() else {
            return;
        };
        if !self.target.is_screenshot() {
            return;
        }
        let width = self.target.canvas().0;
        let centre = layer.x + layer.width / 2.;
        let steps = (centre / width).floor() as isize;
        if steps == 0 {
            return;
        }
        let target = self.slide as isize + steps;
        if target < 0 || target as usize >= self.design.slides.len() {
            return;
        }
        let target = target as usize;
        if let Some(slide) = self.current_mut() {
            slide.layers.retain(|l| l.id != id);
        }
        let mut moved = layer;
        moved.x -= steps as f32 * width;
        self.design.slides[target].layers.push(moved);
        self.slide = target;
        self.stage_scroll.scroll_to_item(target + 1);
    }

    /// Clicking the part of a layer that spans onto another slide picks it
    /// up on the slide that owns it.
    fn grab_from_neighbour(
        &mut self,
        owner: usize,
        id: u64,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if owner != self.slide && owner < self.design.slides.len() {
            self.slide = owner;
            self.sync_controls(window, cx);
        }
        self.begin_drag(id, DragMode::Move, origin, window, cx);
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Keys belong to text fields while one is focused.
        if !self.focus.is_focused(window) {
            return;
        }
        let keystroke = &event.keystroke;
        let command = keystroke.modifiers.secondary(cx);
        let shift = keystroke.modifiers.shift;
        let step = if shift { 10. } else { 1. };
        match (keystroke.key.as_str(), command) {
            ("z", true) if shift => self.redo(window, cx),
            ("z", true) => self.undo(window, cx),
            ("d", true) => self.duplicate_selected(window, cx),
            ("]", true) => self.restack(1, shift, window, cx),
            ("[", true) => self.restack(-1, shift, window, cx),
            ("backspace" | "delete", _) => self.delete_selected(window, cx),
            ("escape", _) if self.preview => self.set_preview(false, cx),
            ("escape", _) => self.select(None, window, cx),
            ("space", false) if self.design.video.is_some() => self.toggle_play(window, cx),
            ("\\", true) => {
                let open = !(self.left_open || self.right_open);
                self.set_panels(open, open, cx);
            }
            ("left", false) => self.nudge(-step, 0., window, cx),
            ("right", false) => self.nudge(step, 0., window, cx),
            ("up", false) => self.nudge(0., -step, window, cx),
            ("down", false) => self.nudge(0., step, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    fn set_chrome_hidden(&mut self, hidden: bool) {
        if self.chrome_hidden != hidden {
            self.chrome_hidden = hidden;
            self.chrome_epoch += 1;
        }
    }

    fn set_panels(&mut self, left: bool, right: bool, cx: &mut Context<Self>) {
        if (left, right) != (self.left_open, self.right_open) {
            self.left_open = left;
            self.right_open = right;
            self.panel_epoch += 1;
            cx.notify();
        }
    }

    fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        self.zoom = zoom.clamp(0.5, 4.);
        cx.notify();
    }

    // Export

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
        let sizes = self.target.sizes();
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
                    // Recordings show their first frame in stills.
                    let clip_source = appdoctor_video::clip_source();
                    let mut written = 0;
                    for size in sizes {
                        for index in 0..slides.len() {
                            let png = render_png_with(&slides, index, *size, &folder, 1.0, clip_source.as_ref())
                                .map_err(|e| e.to_string())?;
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

    // Video

    /// Turns the design into a video, or back into stills.
    fn set_video(&mut self, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(window, cx, |this| {
            this.design.video = on.then(|| this.design.video.clone().unwrap_or_default());
        });
        if !on {
            self.set_preview(false, cx);
        }
    }

    fn set_transition(&mut self, transition: Transition, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(window, cx, |this| {
            this.design.video.get_or_insert_with(Video::default).transition = transition;
        });
    }

    fn set_effect(&mut self, effect: Effect, leaving: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if leaving {
                layer.motion.exit = effect;
            } else {
                layer.motion.enter = effect;
            }
        });
    }

    fn set_drift(&mut self, drift: Drift, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| layer.motion.drift = drift);
    }

    fn duration(&self) -> f32 {
        motion::duration(&self.design.slides)
    }

    /// Shows the video at the playhead instead of the slides, or goes back.
    fn set_preview(&mut self, on: bool, cx: &mut Context<Self>) {
        self.preview = on;
        if !on {
            self.playing = false;
            self._ticker = None;
        }
        cx.notify();
    }

    fn toggle_play(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.playing {
            self.playing = false;
            self._ticker = None;
            cx.notify();
            return;
        }
        self.preview = true;
        self.playing = true;
        if self.time >= self.duration() - 0.05 {
            self.time = 0.;
        }
        // Advance by the real time between frames, looping at the end like
        // the App Store's header does.
        self._ticker = Some(cx.spawn_in(window, async move |this, cx| {
            let mut last = Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(16)).await;
                let now = Instant::now();
                let step = (now - last).as_secs_f32();
                last = now;
                let alive = this.update_in(cx, |this, window, cx| {
                    let length = this.duration().max(0.1);
                    this.time = (this.time + step) % length;
                    this.syncing = true;
                    let share = this.time / length;
                    this.sliders
                        .time
                        .update(cx, |slider, cx| slider.set_value(share, window, cx));
                    this.syncing = false;
                    cx.notify();
                });
                if alive.is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }

    /// Renders the video with AVFoundation into a folder the user picks.
    fn export_video(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Export Here".into()),
        });
        let design = self.design.clone();
        let name = format!("{}.mp4", design.target.video_size().slug());
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(target) = paths.into_iter().next() else {
                return;
            };
            let out = target.join(&name);
            let (progress, mut updates) = futures::channel::mpsc::unbounded::<f32>();
            this.update(cx, |this, cx| {
                this.exporting = true;
                this.status = Some(Ok("Exporting video…".into()));
                cx.notify();
            })
            .ok();
            let job = cx.background_spawn({
                let out = out.clone();
                async move {
                    appdoctor_video::export(&design, &folder, &out, |done| {
                        let _ = progress.unbounded_send(done);
                        true
                    })
                    .map_err(|error| error.to_string())
                }
            });
            while let Some(done) = futures::StreamExt::next(&mut updates).await {
                this.update(cx, |this, cx| {
                    this.status = Some(Ok(format!("Exporting video… {:.0}%", done * 100.)));
                    cx.notify();
                })
                .ok();
            }
            let result = job.await;
            this.update(cx, |this, cx| {
                this.exporting = false;
                this.status = Some(result.map(|()| format!("Saved {} to {}", name, target.display())));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // Small property setters used by the inspector's buttons.

    fn set_weight(&mut self, weight: u16, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Text { weight: w, .. } = &mut layer.kind {
                *w = weight;
            }
            // Heavier type is wider, so the lines may change.
            layer.fit_text();
        });
    }

    fn set_align(&mut self, align: Align, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Text { align: a, .. } = &mut layer.kind {
                *a = align;
            }
        });
    }

    fn set_frame(&mut self, frame: Frame, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Device { frame: f, .. } = &mut layer.kind {
                *f = frame;
            }
        });
    }

    /// Changes a device's model. The width stays; the height follows the
    /// new shape.
    fn set_model(&mut self, model: DeviceModel, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Device { model: current, .. } = &mut layer.kind {
                if layer.name == current.target().label() {
                    layer.name = model.target().label().into();
                }
                *current = model;
                layer.height = layer.width * model.aspect();
            }
        });
    }

    fn set_font(&mut self, font: String, window: &mut Window, cx: &mut Context<Self>) {
        let font = (font != DEFAULT_FONT).then_some(font);
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Text { font: current, .. } = &mut layer.kind {
                *current = font;
            }
            layer.fit_text();
        });
    }

    fn toggle_italic(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| {
            if let LayerKind::Text { italic, .. } = &mut layer.kind {
                *italic = !*italic;
            }
            layer.fit_text();
        });
    }

    fn toggle_shadow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.apply_to_selected(window, cx, |layer| match &mut layer.kind {
            LayerKind::Device { shadow, .. } | LayerKind::Image { shadow, .. } => *shadow = !*shadow,
            _ => {}
        });
    }

    fn set_color_value(&mut self, color: &str, window: &mut Window, cx: &mut Context<Self>) {
        let color = color.to_owned();
        self.apply_to_selected(window, cx, |layer| match &mut layer.kind {
            LayerKind::Text { color: c, .. } => *c = color.clone(),
            LayerKind::Shape { fill, .. } => *fill = color.clone(),
            _ => {}
        });
    }

    fn apply_to_selected(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Layer)) {
        let Some(id) = self.selected else {
            return;
        };
        self.apply(window, cx, |this| {
            if let Some(layer) = this.layer_mut(id) {
                f(layer);
            }
        });
    }

    fn set_background(&mut self, background: Background, window: &mut Window, cx: &mut Context<Self>) {
        self.apply(window, cx, |this| {
            if let Some(slide) = this.current_mut() {
                slide.background = background;
            }
        });
    }
}

fn render_thumb(
    slides: &[Slide],
    index: usize,
    size: CanvasSize,
    folder: &Path,
    clips: &dyn appdoctor_screenshots::ClipSource,
) -> Option<Arc<Image>> {
    match render_png_with(slides, index, size, folder, THUMB_SCALE, clips) {
        Ok(png) => Some(Arc::new(Image::from_bytes(ImageFormat::Png, png))),
        Err(error) => {
            tracing::warn!(%error, "thumbnail render failed");
            None
        }
    }
}
