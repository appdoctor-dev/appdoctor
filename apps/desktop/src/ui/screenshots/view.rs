//! Drawing the designer: the slides row with the live canvas, the panels
//! and the floating tools.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use appdoctor_screenshots::edit::{Corner, Guide};
use appdoctor_screenshots::{
    Align, Background, DEFAULT_FONT, LINE_HEIGHT, Layer, LayerKind, Rect, Slide, Target, Transition, device_geometry,
    is_video, motion, parse_color, visible_layers,
};
use gpui_kit::component::slider::Slider;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{DragMode, ScreenshotsView};
use crate::theme::meter;
use crate::ui::widgets::{self, status_dot};

const LEFT_PANEL: f32 = 240.;
pub(super) const RIGHT_PANEL: f32 = 280.;
const HEADER: f32 = 56.;
const SIDEBAR: f32 = 236.;
const TITLE_BAR: f32 = 34.;
const STAGE_TOP: f32 = 48.;
const STAGE_BOTTOM: f32 = 92.;

/// Apple's systemBlue, as Keynote and Freeform outline selections.
fn selection_color() -> Hsla {
    rgb(0x0a84ff).into()
}

fn guide_color() -> Hsla {
    rgb(0xf43f5e).into()
}

pub(super) fn color(hex: &str) -> Hsla {
    let (r, g, b) = parse_color(hex).unwrap_or((0, 0, 0));
    rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32).into()
}

fn background_fill(background: &Background) -> gpui_kit::Background {
    match background {
        Background::Solid { color: c } => color(c).into(),
        Background::Gradient { from, to, angle } => linear_gradient(
            *angle,
            linear_color_stop(color(from), 0.),
            linear_color_stop(color(to), 1.),
        ),
    }
}

pub(super) fn layer_icon(layer: &Layer) -> Icon {
    match layer.kind {
        LayerKind::Text { .. } => Icon::new(gpui_kit::assets::IconName::Type),
        LayerKind::Device { model, .. } => Icon::new(match model.target() {
            Target::Ipad => gpui_kit::assets::IconName::Tablet,
            _ => gpui_kit::assets::IconName::Smartphone,
        }),
        LayerKind::Image { .. } => Icon::new(gpui_kit::assets::IconName::Image),
        LayerKind::Shape { .. } => Icon::new(IconName::Square),
    }
}

/// Slides form one strip: only its two ends are rounded.
fn strip_corners<E: Styled>(element: E, first: bool, last: bool) -> E {
    match (first, last) {
        (true, true) => element.rounded(px(8.)),
        (true, false) => element.rounded_l(px(8.)),
        (false, true) => element.rounded_r(px(8.)),
        (false, false) => element,
    }
}

fn drop_shadow(s: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0., 0., 0., 0.35),
        offset: point(px(0.), px(24. * s)),
        blur_radius: px(70. * s),
        spread_radius: px(0.),
        inset: false,
    }]
}

/// A layer's look at scale `s`, positioned inside its own box. `clip` is
/// the current frame of a device's screen recording.
fn layer_visual(layer: &Layer, s: f32, folder: &Path, clip: Option<Arc<Image>>) -> AnyElement {
    let (w, h) = (layer.width * s, layer.height * s);
    match &layer.kind {
        LayerKind::Text {
            size,
            weight,
            color: c,
            align,
            font,
            italic,
            ..
        } => {
            let family = SharedString::from(font.clone().unwrap_or_else(|| DEFAULT_FONT.to_owned()));
            v_flex()
                .w(px(w))
                .map(|this| match align {
                    Align::Left => this.items_start(),
                    Align::Center => this.items_center(),
                    Align::Right => this.items_end(),
                })
                .children(layer.lines().into_iter().map(|line| {
                    div()
                        .whitespace_nowrap()
                        .font_family(family.clone())
                        .when(*italic, |this| this.italic())
                        .text_size(px(size * s))
                        .line_height(px(size * s * LINE_HEIGHT))
                        .font_weight(FontWeight(*weight as f32))
                        .text_color(color(c))
                        .child(if line.is_empty() { " ".to_owned() } else { line })
                }))
                .into_any_element()
        }
        LayerKind::Device {
            model,
            frame,
            screenshot,
            shadow,
            ..
        } => {
            let g = device_geometry(layer, *model, *frame);
            let glass = color(frame.glass_color(*model));
            // A part of the device, placed inside the layer's box.
            let part = |r: Rect, radius: f32| {
                div()
                    .absolute()
                    .left(px((r.x - layer.x) * s))
                    .top(px((r.y - layer.y) * s))
                    .w(px(r.width * s))
                    .h(px(r.height * s))
                    .rounded(px(radius * s))
            };
            let screen = part(g.screen, g.screen_radius)
                .bg(rgb(0x1c1c1f))
                .when(*shadow && frame.edge_color().is_none(), |this| {
                    this.shadow(drop_shadow(s))
                })
                .when_some(screenshot.clone().filter(|file| !is_video(file)), |this, file| {
                    this.child(
                        img(folder.join(file))
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .rounded(px(g.screen_radius * s)),
                    )
                })
                .when_some(clip, |this, frame| {
                    this.child(
                        img(frame)
                            .size_full()
                            .object_fit(ObjectFit::Cover)
                            .rounded(px(g.screen_radius * s)),
                    )
                });
            div()
                .relative()
                .w(px(w))
                .h(px(h))
                .when_some(frame.button_color(), |this, button| {
                    this.children(
                        g.buttons
                            .iter()
                            .map(|b| part(*b, b.width.min(b.height) / 2.).bg(color(button))),
                    )
                })
                .when_some(frame.edge_color(), |this, edge| {
                    this.child(
                        part(g.body, g.body_radius)
                            .bg(color(edge))
                            .when(*shadow, |this| this.shadow(drop_shadow(s))),
                    )
                    .child(part(g.glass, g.glass_radius).bg(glass))
                })
                .child(screen)
                .when_some(g.crease, |this, crease| {
                    this.child(part(crease, 0.).bg(hsla(0., 0., 0., 0.22)))
                })
                .when_some(g.island, |this, island| {
                    this.child(part(island, island.height / 2.).bg(rgb(0x000000)))
                })
                .when_some(g.notch, |this, notch| {
                    this.child(part(notch, 0.).rounded_b(px(g.notch_radius * s)).bg(glass))
                })
                .when_some(g.speaker, |this, speaker| {
                    this.child(part(speaker, speaker.height / 2.).bg(rgb(0x2b2b2e)))
                })
                .when_some(g.camera, |this, camera| {
                    this.child(part(camera, camera.width / 2.).bg(rgb(0x16161a)))
                })
                .when_some(g.home_button.zip(frame.button_color()), |this, (button, ring)| {
                    this.child(
                        part(button, button.width / 2.)
                            .bg(glass)
                            .border(px((button.width * 0.05 * s).max(0.5)))
                            .border_color(color(ring)),
                    )
                })
                .into_any_element()
        }
        LayerKind::Image { file, radius, shadow } => div()
            .w(px(w))
            .h(px(h))
            .rounded(px(radius * s))
            .when(*shadow, |this| this.shadow(drop_shadow(s)))
            .when_some(file.clone(), |this, file| {
                this.child(
                    img(folder.join(file))
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .rounded(px(radius * s)),
                )
            })
            .into_any_element(),
        LayerKind::Shape { fill, radius } => div()
            .w(px(w))
            .h(px(h))
            .rounded(px(radius.min(layer.width.min(layer.height) / 2.) * s))
            .bg(color(fill))
            .into_any_element(),
    }
}

impl ScreenshotsView {
    /// Fit the canvas to the space between the panels, then apply zoom.
    fn canvas_scale(&self, window: &Window) -> f32 {
        let viewport = window.viewport_size();
        // Room above for the zoom controls and below for the slide numbers
        // and the floating tools.
        let available_h = f32::from(viewport.height) - TITLE_BAR - HEADER - STAGE_TOP - STAGE_BOTTOM - 30.;
        let panels = if self.left_open { LEFT_PANEL } else { 0. } + if self.right_open { RIGHT_PANEL } else { 0. };
        let available_w = f32::from(viewport.width) - SIDEBAR - panels - 120.;
        let (width, height) = self.target.canvas();
        // Screenshots leave room to glimpse their neighbours; header and
        // search art fit whole.
        let overflow = if self.target.is_screenshot() { 1.6 } else { 1.15 };
        let fit = (available_h / height)
            .min(available_w / width * overflow)
            .clamp(0.05, 0.4);
        fit * self.zoom
    }

    fn slide_canvas(&self, index: usize, slide: &Slide, s: f32, folder: &Path, cx: &mut Context<Self>) -> AnyElement {
        let active = index == self.slide;
        // Own layers plus the parts of neighbours' layers that span onto
        // this slide; the slide clips them at its edges, like the export.
        let (canvas_width, canvas_height) = self.target.canvas();
        let visible = if self.target.is_screenshot() {
            visible_layers(&self.design.slides, index, canvas_width)
        } else {
            slide.layers.iter().filter(|l| !l.hidden).map(|l| (index, l.clone())).collect()
        };
        let layers: Vec<AnyElement> = visible
            .iter()
            .map(|(owner, layer)| {
                let (owner, id, locked) = (*owner, layer.id, layer.locked);
                let element = self
                    .positioned(layer, s, folder, None)
                    .id(SharedString::from(format!("layer-{index}-{owner}-{id}")));
                if active && owner == index {
                    element
                        .when(!locked, |this| this.cursor_move())
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            let hovered = if *hovered { Some(id) } else { None };
                            if this.hovered != hovered && (hovered.is_some() || this.hovered == Some(id)) {
                                this.hovered = hovered;
                                cx.notify();
                            }
                        }))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.begin_drag(id, DragMode::Move, event.position, window, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .into_any_element()
                } else if owner != index {
                    // A spanning layer's other half: grab it to move the whole layer.
                    element
                        .when(!locked, |this| this.cursor_move())
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                this.grab_from_neighbour(owner, id, event.position, window, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .into_any_element()
                } else {
                    element.into_any_element()
                }
            })
            .collect();

        // Slides sit edge to edge, as the export cuts them, so a layer
        // spanning two keeps its true width; only the strip's ends round.
        // Header and search versions stand apart, each fully rounded.
        let strip = self.target.is_screenshot();
        let (first, last) = (
            index == 0 || !strip,
            index + 1 == self.design.slides.len() || !strip,
        );
        let canvas = div()
            .id(("slide-canvas", index))
            .relative()
            .w(px(canvas_width * s))
            .h(px(canvas_height * s))
            .overflow_hidden()
            .map(|this| strip_corners(this, first, last))
            .bg(background_fill(&slide.background))
            .children(layers)
            // Where the App Store cuts: a hairline, dark and light so it
            // shows on any background.
            .when(!first, |this| {
                this.child(div().absolute().left_0().top_0().h_full().w(px(1.)).bg(hsla(0., 0., 0., 0.2)))
                    .child(
                        div()
                            .absolute()
                            .left(px(1.))
                            .top_0()
                            .h_full()
                            .w(px(1.))
                            .bg(hsla(0., 0., 1., 0.22)),
                    )
            });

        if !active {
            return v_flex()
                .gap_2()
                .items_center()
                .child(
                    canvas
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, window, cx| this.select_slide(index, window, cx)),
                        ),
                )
                .child(self.slide_label(index, cx))
                .into_any_element();
        }

        let guides = self.guides.iter().map(|guide| match guide {
            Guide::Vertical(x) => div()
                .absolute()
                .left(px(x * s))
                .top_0()
                .h_full()
                .w(px(1.))
                .bg(guide_color()),
            Guide::Horizontal(y) => div()
                .absolute()
                .top(px(y * s))
                .left_0()
                .w_full()
                .h(px(1.))
                .bg(guide_color()),
        });

        // Outlines and handles sit outside the slide's clip, drawn last, so a
        // layer hanging off the edge can still be seen and resized.
        let hover_outline = self
            .hovered
            .filter(|id| Some(*id) != self.selected && self.drag.is_none())
            .and_then(|id| slide.layers.iter().find(|l| l.id == id && !l.hidden))
            .map(|layer| {
                deferred(
                    div()
                        .absolute()
                        .left(px(layer.x * s))
                        .top(px(layer.y * s))
                        .w(px(layer.width * s))
                        .h(px(layer.height * s))
                        .rounded(px(3.))
                        .border_1()
                        .border_color(selection_color().opacity(0.65))
                        .bg(selection_color().opacity(0.07)),
                )
            });
        let selection = self
            .selected
            .and_then(|id| slide.layers.iter().find(|l| l.id == id && !l.hidden))
            .map(|layer| deferred(self.selection_box(layer, s, cx)).with_priority(1));

        let settle = Animation::new(Duration::from_millis(220)).with_easing(ease_in_out);
        v_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .relative()
                    .child(
                        canvas
                            // The active slide's ring, inside its edges so
                            // the neighbours don't cover it.
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .map(|this| strip_corners(this, first, last))
                                    .border_2()
                                    .border_color(selection_color().opacity(0.9))
                                    .with_animation(("active-ring", index), settle, |this, t| this.opacity(t)),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| {
                                    window.focus(&this.focus, cx);
                                    this.select(None, window, cx);
                                }),
                            )
                            .children(self.target.safe_inset().map(|(inset_x, inset_y)| {
                                // Keep words and faces inside: the App Store
                                // crops the rest on some devices.
                                div()
                                    .absolute()
                                    .left(px(canvas_width * inset_x * s))
                                    .top(px(canvas_height * inset_y * s))
                                    .w(px(canvas_width * (1. - 2. * inset_x) * s))
                                    .h(px(canvas_height * (1. - 2. * inset_y) * s))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_dashed()
                                    .border_color(hsla(0., 0., 1., 0.6))
                            }))
                            .children(guides),
                    )
                    .children(hover_outline)
                    .children(selection),
            )
            .child(self.slide_label(index, cx))
            .into_any_element()
    }

    /// A layer drawn in an absolutely positioned box at scale `s`.
    /// A layer placed on its slide. A device playing a recording shows the
    /// frame at `clip_time`, or at its start when that's `None`.
    fn positioned(&self, layer: &Layer, s: f32, folder: &Path, clip_time: Option<f32>) -> Div {
        let clip = match &layer.kind {
            LayerKind::Device {
                screenshot: Some(file),
                clip_start,
                ..
            } if is_video(file) => self.clips.get(&folder.join(file), clip_time.unwrap_or(*clip_start)),
            _ => None,
        };
        div()
            .absolute()
            .left(px(layer.x * s))
            .top(px(layer.y * s))
            .w(px(layer.width * s))
            .h(px(layer.height * s))
            .opacity(layer.opacity.clamp(0., 1.))
            .child(layer_visual(layer, s, folder, clip))
    }

    fn selection_box(&self, layer: &Layer, s: f32, cx: &mut Context<Self>) -> AnyElement {
        let id = layer.id;
        let handle = |corner: Corner, cx: &mut Context<Self>| {
            let size = px(10.);
            let offset = px(-5.);
            let cursor = match corner {
                Corner::TopLeft | Corner::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                Corner::TopRight | Corner::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            };
            div()
                .id(SharedString::from(format!("handle-{id}-{corner:?}")))
                .absolute()
                .size(size)
                .map(|this| match corner {
                    Corner::TopLeft => this.left(offset).top(offset),
                    Corner::TopRight => this.right(offset).top(offset),
                    Corner::BottomLeft => this.left(offset).bottom(offset),
                    Corner::BottomRight => this.right(offset).bottom(offset),
                })
                .rounded_full()
                .bg(rgb(0xffffff))
                .border_1()
                .border_color(selection_color())
                .shadow_sm()
                .cursor(cursor)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        this.begin_drag(id, DragMode::Resize(corner), event.position, window, cx);
                        cx.stop_propagation();
                    }),
                )
        };
        div()
            .id(("selection", id))
            .absolute()
            .left(px(layer.x * s))
            .top(px(layer.y * s))
            .w(px(layer.width * s))
            .h(px(layer.height * s))
            .rounded(px(3.))
            .border_1()
            .border_color(selection_color())
            .when(!layer.locked, |this| this.cursor_move())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.begin_drag(id, DragMode::Move, event.position, window, cx);
                    cx.stop_propagation();
                }),
            )
            .when(!layer.locked, |this| {
                this.child(handle(Corner::TopLeft, cx))
                    .child(handle(Corner::TopRight, cx))
                    .child(handle(Corner::BottomLeft, cx))
                    .child(handle(Corner::BottomRight, cx))
            })
            .into_any_element()
    }

    fn slide_label(&self, index: usize, cx: &App) -> impl IntoElement {
        let active = index == self.slide;
        div()
            .px_2()
            .py_0p5()
            .rounded_full()
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .when(active, |this| this.bg(selection_color()).text_color(rgb(0xffffff)))
            .when(!active, |this| this.text_color(cx.theme().muted_foreground))
            .child(format!("{}", index + 1))
    }

    /// One tile of the floating tools bar: an icon over a short label.
    fn tool(
        &self,
        id: &'static str,
        icon: impl Into<Icon>,
        label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
        action: fn(&mut Self, &mut Window, &mut Context<Self>),
    ) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .id(id)
            .w(px(52.))
            .py_1p5()
            .gap_1()
            .items_center()
            .rounded(px(10.))
            .text_color(theme.foreground)
            .when(enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(theme.accent))
                    .active(|style| style.bg(theme.accent.opacity(0.7)))
                    .on_click(cx.listener(move |this, _, window, cx| action(this, window, cx)))
            })
            .when(!enabled, |this| this.opacity(0.35))
            .child(icon.into().size(px(17.)))
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme.muted_foreground)
                    .child(label),
            )
    }

    /// The video at the playhead, fitted whole into the stage.
    fn preview_canvas(&self, window: &Window, folder: &Path, cx: &mut Context<Self>) -> AnyElement {
        let (width, height) = self.target.canvas();
        let viewport = window.viewport_size();
        let available_h = f32::from(viewport.height) - TITLE_BAR - HEADER - STAGE_TOP - STAGE_BOTTOM - 30.;
        let panels = if self.left_open { LEFT_PANEL } else { 0. } + if self.right_open { RIGHT_PANEL } else { 0. };
        let available_w = f32::from(viewport.width) - SIDEBAR - panels - 80.;
        let s = (available_h / height).min(available_w / width).clamp(0.05, 1.);

        let video = self.design.video.clone().unwrap_or_default();
        let at = motion::moment(&self.design.slides, &video, self.time);
        let mut parts = Vec::new();
        match at.next {
            Some((next, p)) if video.transition == Transition::Push => {
                self.scene_parts(&mut parts, at.scene, at.time, -p * width, 1., Some(next), s, folder);
                self.scene_parts(&mut parts, next, 0., (1. - p) * width, 1., Some(at.scene), s, folder);
            }
            Some((next, p)) => {
                self.scene_parts(&mut parts, at.scene, at.time, 0., 1., None, s, folder);
                self.scene_parts(&mut parts, next, 0., 0., p, None, s, folder);
            }
            None => self.scene_parts(&mut parts, at.scene, at.time, 0., 1., None, s, folder),
        }
        let theme = cx.theme();
        v_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .relative()
                    .w(px(width * s))
                    .h(px(height * s))
                    .overflow_hidden()
                    .rounded(px(8.))
                    .shadow_lg()
                    .children(parts),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(theme.muted_foreground)
                    .child(format!("Scene {} of {}", at.scene + 1, self.design.slides.len())),
            )
            .into_any_element()
    }

    /// One scene of the preview, `offset` canvas pixels across and faded
    /// by `opacity`, as `render_frame` draws it for export.
    #[allow(clippy::too_many_arguments)]
    fn scene_parts(
        &self,
        parts: &mut Vec<AnyElement>,
        index: usize,
        time: f32,
        offset: f32,
        opacity: f32,
        skip: Option<usize>,
        s: f32,
        folder: &Path,
    ) {
        let Some(slide) = self.design.slides.get(index) else {
            return;
        };
        let canvas = self.target.canvas();
        parts.push(
            div()
                .absolute()
                .top_0()
                .left(px(offset * s))
                .w(px(canvas.0 * s))
                .h(px(canvas.1 * s))
                .bg(background_fill(&slide.background))
                .opacity(opacity)
                .into_any_element(),
        );
        let layers = motion::scene_layers(&self.design.slides, index, time, canvas, self.target.is_screenshot(), skip);
        for (_, mut layer, clip) in layers {
            layer.x += offset;
            layer.opacity *= opacity;
            parts.push(self.positioned(&layer, s, folder, Some(clip)).into_any_element());
        }
    }

    /// Play, the playhead and the time, in place of the tools while
    /// previewing.
    fn transport(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let clock = |t: f32| format!("{}:{:04.1}", (t / 60.).floor() as u32, t % 60.);
        h_flex()
            .px_3()
            .py_2()
            .gap_3()
            .items_center()
            .rounded(px(18.))
            .border_1()
            .border_color(theme.border.opacity(0.7))
            .bg(theme.popover.opacity(0.92))
            .shadow_lg()
            .child(
                Button::new("transport-play")
                    .ghost()
                    .small()
                    .icon(if self.playing {
                        gpui_kit::assets::IconName::Pause
                    } else {
                        gpui_kit::assets::IconName::Play
                    })
                    .tooltip(if self.playing { "Pause  Space" } else { "Play  Space" })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_play(window, cx))),
            )
            .child(
                div()
                    .w(px(96.))
                    .text_size(px(12.))
                    .font_family(theme.mono_font_family.clone())
                    .text_color(theme.muted_foreground)
                    .child(format!("{} / {}", clock(self.time), clock(self.duration()))),
            )
            .child(div().w(px(320.)).child(Slider::new(&self.sliders.time)))
            .child(
                Button::new("transport-edit")
                    .ghost()
                    .small()
                    .label("Edit")
                    .on_click(cx.listener(|this, _, _, cx| this.set_preview(false, cx))),
            )
    }

    fn tools_pill(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let divider = || div().w(px(1.)).h(px(28.)).mx_1().bg(theme.border.opacity(0.7));
        h_flex()
            .px_1p5()
            .py_1()
            .gap_0p5()
            .items_center()
            .rounded(px(18.))
            .border_1()
            .border_color(theme.border.opacity(0.7))
            .bg(theme.popover.opacity(0.9))
            .shadow(vec![
                BoxShadow {
                    color: hsla(0., 0., 0., 0.28),
                    offset: point(px(0.), px(10.)),
                    blur_radius: px(30.),
                    spread_radius: px(0.),
                    inset: false,
                },
                BoxShadow {
                    color: hsla(0., 0., 0., 0.12),
                    offset: point(px(0.), px(1.)),
                    blur_radius: px(2.),
                    spread_radius: px(0.),
                    inset: false,
                },
            ])
            .child(self.tool(
                "tool-undo",
                IconName::Undo2,
                "Undo",
                !self.undo.is_empty(),
                cx,
                Self::undo,
            ))
            .child(self.tool(
                "tool-redo",
                IconName::Redo2,
                "Redo",
                !self.redo.is_empty(),
                cx,
                Self::redo,
            ))
            .child(divider())
            .child(self.tool(
                "tool-text",
                gpui_kit::assets::IconName::Type,
                "Text",
                true,
                cx,
                Self::add_text,
            ))
            .child(self.tool(
                "tool-device",
                match self.target {
                    Target::Ipad => gpui_kit::assets::IconName::Tablet,
                    _ => gpui_kit::assets::IconName::Smartphone,
                },
                match self.target {
                    Target::Iphone => "iPhone",
                    Target::Ipad => "iPad",
                    _ => "Device",
                },
                true,
                cx,
                Self::add_device,
            ))
            .child(self.tool(
                "tool-image",
                gpui_kit::assets::IconName::Image,
                "Image",
                true,
                cx,
                Self::add_image,
            ))
            .child(self.tool("tool-shape", IconName::Square, "Shape", true, cx, Self::add_shape))
            .child(divider())
            .child(self.tool(
                "tool-screenshot",
                IconName::FolderOpen,
                "Screen",
                true,
                cx,
                Self::choose_screenshot,
            ))
    }

    fn zoom_pill(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        h_flex()
            .gap_0p5()
            .p_0p5()
            .items_center()
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border.opacity(0.7))
            .bg(theme.popover.opacity(0.9))
            .shadow_md()
            .child(
                Button::new("zoom-out")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Minus)
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(this.zoom / 1.25, cx))),
            )
            .child(
                Button::new("zoom-reset")
                    .ghost()
                    .xsmall()
                    .label(format!("{:.0}%", self.zoom * 100.))
                    .tooltip("Fit to window")
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(1., cx))),
            )
            .child(
                Button::new("zoom-in")
                    .ghost()
                    .xsmall()
                    .icon(IconName::Plus)
                    .on_click(cx.listener(|this, _, _, cx| this.set_zoom(this.zoom * 1.25, cx))),
            )
    }

    fn panel_heading(label: &str) -> Div {
        h_flex()
            .h(px(28.))
            .px(px(14.))
            .mt(px(10.))
            .gap_0p5()
            .items_center()
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(div().flex_1().child(label.to_owned()))
    }

    fn left_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let hover = theme.foreground.opacity(0.05);
        // Thumbnails keep the canvas's shape inside a 76 × 68 box.
        let (canvas_width, canvas_height) = self.target.canvas();
        let (thumb_w, thumb_h) = if canvas_height >= canvas_width {
            (68. * canvas_width / canvas_height, 68.)
        } else {
            (76., 76. * canvas_height / canvas_width)
        };
        let slides = self.design.slides.iter().enumerate().map(|(ix, slide)| {
            let active = ix == self.slide;
            let thumb = self.thumbs.get(ix).cloned().flatten();
            let count = slide.layers.len();
            h_flex()
                .id(("slide-row", ix))
                .mx(px(8.))
                .px(px(6.))
                .py(px(5.))
                .gap(px(10.))
                .items_center()
                .rounded(px(9.))
                .cursor_pointer()
                .when(active, |this| this.bg(theme.primary.opacity(0.14)))
                .when(!active, |this| this.hover(|style| style.bg(hover)))
                .child(
                    div()
                        .w(px(14.))
                        .text_size(px(11.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if active { theme.primary } else { theme.muted_foreground })
                        .child(format!("{}", ix + 1)),
                )
                .child(
                    div()
                        .w(px(thumb_w))
                        .h(px(thumb_h))
                        .flex_shrink_0()
                        .rounded(px(7.))
                        .p(px(2.))
                        .border_2()
                        .border_color(if active { theme.primary } else { transparent_black() })
                        .child(
                            div()
                                .size_full()
                                .rounded(px(4.))
                                .overflow_hidden()
                                .bg(theme.muted)
                                .when_some(thumb, |this, image| this.child(img(image).size_full())),
                        ),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(format!("{} {}", self.slide_noun(), ix + 1)),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(format!("{count} layer{}", if count == 1 { "" } else { "s" })),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select_slide(ix, window, cx)))
        });

        let layers: Vec<_> = self
            .current()
            .map(|slide| slide.layers.iter().rev().cloned().collect())
            .unwrap_or_default();
        let layer_rows = layers.into_iter().map(|layer| {
            let id = layer.id;
            let selected = self.selected == Some(id);
            let group = SharedString::from(format!("layer-row-{id}"));
            let always = layer.hidden || layer.locked;
            let (fg, muted) = if selected {
                (theme.primary_foreground, theme.primary_foreground.opacity(0.8))
            } else {
                (theme.foreground, theme.muted_foreground)
            };
            let toggle = |button: Button| {
                button
                    .ghost()
                    .xsmall()
                    .when(selected, |b| b.text_color(theme.primary_foreground))
            };
            h_flex()
                .id(("layer-row", id))
                .group(group.clone())
                .mx(px(8.))
                .h(px(30.))
                .pl(px(8.))
                .pr(px(2.))
                .gap(px(8.))
                .items_center()
                .rounded(px(7.))
                .text_size(px(13.))
                .text_color(fg)
                .cursor_pointer()
                .when(selected, |this| this.bg(theme.primary))
                .when(!selected, |this| this.hover(|style| style.bg(hover)))
                .child(layer_icon(&layer).size(px(14.)).text_color(muted))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when(layer.hidden, |this| this.text_color(muted))
                        .child(layer.name.clone()),
                )
                .child(
                    h_flex()
                        .when(!always, |this| {
                            this.opacity(0.).group_hover(group.clone(), |style| style.opacity(1.))
                        })
                        .child(
                            toggle(Button::new(("layer-eye", id)))
                                .icon(if layer.hidden { IconName::EyeOff } else { IconName::Eye })
                                .tooltip(if layer.hidden { "Show" } else { "Hide" })
                                .on_click(cx.listener(move |this, _, window, cx| this.toggle(id, true, window, cx))),
                        )
                        .child(
                            toggle(Button::new(("layer-lock", id)))
                                .icon(Icon::new(if layer.locked {
                                    gpui_kit::assets::IconName::Lock
                                } else {
                                    gpui_kit::assets::IconName::LockOpen
                                }))
                                .tooltip(if layer.locked { "Unlock" } else { "Lock" })
                                .on_click(cx.listener(move |this, _, window, cx| this.toggle(id, false, window, cx))),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    window.focus(&this.focus, cx);
                    this.select(Some(id), window, cx)
                }))
        });

        let small =
            |id: &'static str, icon: Icon, tip: &'static str| Button::new(id).ghost().xsmall().icon(icon).tooltip(tip);
        v_flex()
            .w(px(LEFT_PANEL))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.border)
            .child(
                Self::panel_heading(if self.design.video.is_some() {
                    "Scenes"
                } else if self.target.is_screenshot() {
                    "Screenshots"
                } else {
                    "Versions"
                })
                    .child(
                        small("duplicate-slide", Icon::new(IconName::Copy), "Duplicate screenshot")
                            .on_click(cx.listener(|this, _, window, cx| this.add_slide(true, window, cx))),
                    )
                    .child(
                        small("delete-slide", Icon::new(IconName::Delete), "Delete screenshot")
                            .disabled(self.design.slides.len() <= 1)
                            .on_click(cx.listener(|this, _, window, cx| this.delete_slide(window, cx))),
                    )
                    .child(
                        small("add-slide", Icon::new(IconName::Plus), "New screenshot")
                            .on_click(cx.listener(|this, _, window, cx| this.add_slide(false, window, cx))),
                    ),
            )
            .child(
                v_flex()
                    .id("slide-list")
                    .max_h(px(300.))
                    .overflow_y_scroll()
                    .py(px(2.))
                    .gap(px(2.))
                    .children(slides),
            )
            .child(div().mx(px(14.)).mt(px(8.)).h(px(1.)).bg(theme.border))
            .child(
                Self::panel_heading("Layers").child(
                    div()
                        .text_size(px(11.))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(theme.muted_foreground)
                        .child(format!("{} {}", self.slide_noun(), self.slide + 1)),
                ),
            )
            .child(
                v_flex()
                    .id("layer-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .py(px(2.))
                    .pb_2()
                    .gap(px(1.))
                    .children(layer_rows),
            )
    }
}

impl Render for ScreenshotsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        let Some(folder) = self.folder(cx) else {
            return div().into_any_element();
        };
        let theme = cx.theme().clone();
        let s = self.canvas_scale(window);
        self.scale = s;

        let header = h_flex()
            .h(px(HEADER))
            .flex_shrink_0()
            .px_5()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child(match self.target {
                        Target::Iphone => "iPhone screenshots",
                        Target::Ipad => "iPad screenshots",
                        Target::Header => "Product page header",
                        Target::Search => "Search result",
                    }))
                    .child(div().text_xs().text_color(theme.muted_foreground).child({
                        let count = self.design.slides.len();
                        let noun = self.slide_noun().to_lowercase();
                        let (w, h) = self.target.sizes()[0].pixels();
                        format!(
                            "{} · {count} {noun}{} · {w} × {h}",
                            app.name,
                            if count == 1 { "" } else { "s" }
                        )
                    })),
            )
            .child(div().flex_1())
            .when_some(self.status.clone(), |this, status| {
                let (c, text) = match status {
                    Ok(text) => (meter::up(), text),
                    Err(text) => (theme.danger, text),
                };
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .max_w(px(460.))
                        .text_xs()
                        .child(status_dot(c))
                        .child(div().truncate().text_color(c).child(text)),
                )
            })
            .child(
                Button::new("toggle-left")
                    .ghost()
                    .small()
                    .icon(if self.left_open {
                        IconName::PanelLeftClose
                    } else {
                        IconName::PanelLeftOpen
                    })
                    .tooltip("Screenshots and layers  ⌘\\")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let right = this.right_open;
                        this.set_panels(!this.left_open, right, cx)
                    })),
            )
            .child(
                Button::new("toggle-right")
                    .ghost()
                    .small()
                    .icon(if self.right_open {
                        IconName::PanelRightClose
                    } else {
                        IconName::PanelRightOpen
                    })
                    .tooltip("Properties  ⌘\\")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let left = this.left_open;
                        this.set_panels(left, !this.right_open, cx)
                    })),
            )
            .child(
                Button::new("ai-agent")
                    .ghost()
                    .small()
                    .icon(gpui_kit::assets::IconName::Sparkles)
                    .label("AI agent")
                    .tooltip("Let your AI agent design these")
                    .on_click(cx.listener(|this, _, window, cx| this.open_agent_dialog(window, cx))),
            )
            .when(self.design.video.is_some(), |this| {
                this.child(
                    Button::new("preview-video")
                        .ghost()
                        .small()
                        .icon(if self.preview {
                            Icon::new(IconName::Close)
                        } else {
                            Icon::new(gpui_kit::assets::IconName::Play)
                        })
                        .label(if self.preview { "Edit" } else { "Preview" })
                        .tooltip("Play the video  Space")
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.preview {
                                this.set_preview(false, cx);
                            } else {
                                this.toggle_play(window, cx);
                            }
                        })),
                )
            })
            .child(
                Button::new("export-shots")
                    .small()
                    .when(self.design.video.is_none(), |button| button.primary())
                    .icon(IconName::ArrowDown)
                    .label("Export PNGs")
                    .loading(self.exporting && self.design.video.is_none())
                    .disabled(self.exporting || self.design.slides.is_empty())
                    .on_click(cx.listener(|this, _, window, cx| this.export(window, cx))),
            )
            .when(self.design.video.is_some(), |this| {
                this.child(
                    Button::new("export-video")
                        .small()
                        .primary()
                        .icon(gpui_kit::assets::IconName::Clapperboard)
                        .label("Export video")
                        .loading(self.exporting)
                        .disabled(self.exporting || self.design.slides.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| this.export_video(window, cx))),
                )
            });

        let slides: Vec<_> = if self.preview {
            vec![self.preview_canvas(window, &folder, cx)]
        } else {
            self.design
                .slides
                .clone()
                .iter()
                .enumerate()
                .map(|(ix, slide)| self.slide_canvas(ix, slide, s, &folder, cx))
                .collect()
        };

        let hidden = self.chrome_hidden;
        let fade = Animation::new(Duration::from_millis(if hidden { 140 } else { 260 })).with_easing(ease_in_out);
        let tools = if self.preview { self.transport(cx) } else { self.tools_pill(cx) }
            .with_animation(("tools", self.chrome_epoch), fade.clone(), move |this, d| {
                let t = if hidden { 1. - d } else { d };
                this.opacity(t).mb(px(16. * (1. - t)))
            });
        let zoom = self
            .zoom_pill(cx)
            .with_animation(("zoom", self.chrome_epoch), fade, move |this, d| {
                this.opacity(if hidden { 1. - d } else { d })
            });

        let stage = div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .bg(linear_gradient(
                180.,
                linear_color_stop(theme.muted.opacity(0.55), 0.),
                linear_color_stop(theme.background, 1.),
            ))
            .child(
                // Flexible spacers centre the row when it fits and let it
                // scroll fully when it doesn't (centring an overflowing row
                // would push its first slide out of reach).
                h_flex()
                    .id("stage")
                    .size_full()
                    .overflow_scroll()
                    .track_scroll(&self.stage_scroll)
                    .items_center()
                    .pt(px(STAGE_TOP))
                    .pb(px(STAGE_BOTTOM))
                    .child(div().flex_1().flex_shrink_0().min_w(px(40.)).h(px(1.)))
                    .children(
                        slides
                            .into_iter()
                            .map(|slide| {
                                div()
                                    .flex_shrink_0()
                                    .when(!self.target.is_screenshot(), |this| this.mx(px(18.)))
                                    .child(slide)
                            }),
                    )
                    .child(div().flex_1().flex_shrink_0().min_w(px(40.)).h(px(1.))),
            )
            .child(div().absolute().top(px(12.)).right(px(12.)).child(zoom))
            .child(
                h_flex()
                    .absolute()
                    .bottom(px(18.))
                    .left_0()
                    .right_0()
                    .justify_center()
                    .child(tools),
            );

        let slide_panel = |open: bool, width: f32, epoch: usize, id: &'static str, panel: AnyElement| {
            div()
                .h_full()
                .flex_shrink_0()
                .overflow_hidden()
                .child(panel)
                .with_animation(
                    (id, epoch),
                    Animation::new(Duration::from_millis(240)).with_easing(ease_in_out),
                    move |this, d| {
                        let t = if open { d } else { 1. - d };
                        this.w(px(width * t)).opacity(t)
                    },
                )
        };
        let left = slide_panel(
            self.left_open,
            LEFT_PANEL,
            self.panel_epoch,
            "left-panel",
            self.left_panel(cx).into_any_element(),
        );
        let right = slide_panel(
            self.right_open,
            RIGHT_PANEL,
            self.panel_epoch,
            "right-panel",
            self.inspector(cx).into_any_element(),
        );

        v_flex()
            .size_full()
            .track_focus(&self.focus)
            .key_context("ScreenshotEditor")
            .on_key_down(cx.listener(Self::key_down))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                if this.drag.is_some() {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.drag_to(event.position, event.modifiers, window, cx);
                    } else {
                        this.end_drag(cx);
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.end_drag(cx)))
            .child(header)
            .child(h_flex().flex_1().min_h_0().child(left).child(stage).child(right))
            .into_any_element()
    }
}
