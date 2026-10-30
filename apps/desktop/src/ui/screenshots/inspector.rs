//! The right-hand inspector: the selected layer's properties, or the
//! screenshot's background when nothing is selected.

use appdoctor_screenshots::{Align, BACKGROUND_PRESETS, Background, Drift, Effect, Frame, Layer, LayerKind, Target, Transition};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState, Textarea};
use gpui_kit::component::select::Select;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::ScreenshotsView;
use super::view::{RIGHT_PANEL, color, layer_icon};
use crate::ui::controls::{self, Segmented, group, heading, row, separator, stacked};

const SWATCHES: &[&str] = &[
    "#ffffff", "#0f172a", "#4f46e5", "#f97316", "#10b981", "#f43f5e", "#facc15",
];
const WEIGHTS: &[(&str, u16)] = &[
    ("Regular", 400),
    ("Medium", 500),
    ("Semibold", 600),
    ("Bold", 700),
    ("Heavy", 800),
];
const ALIGNS: &[(&str, Align)] = &[
    ("Left", Align::Left),
    ("Centre", Align::Center),
    ("Right", Align::Right),
];
/// Drifts offered in the inspector (agents can also zoom out).
const DRIFTS: [(Drift, &str); 6] = [
    (Drift::None, "None"),
    (Drift::Up, "Up"),
    (Drift::Down, "Down"),
    (Drift::Left, "Left"),
    (Drift::Right, "Right"),
    (Drift::ZoomIn, "Zoom"),
];
const ANGLES: &[(&str, &str, i32)] = &[
    ("↓", "Top to bottom", 180),
    ("↘", "Diagonal down", 135),
    ("→", "Left to right", 90),
    ("↗", "Diagonal up", 45),
    ("↑", "Bottom to top", 0),
];

/// Inner width of the inspector, and of a control inside a group's row.
const CONTENT: f32 = RIGHT_PANEL - 24.;
const ROW_CONTROL: f32 = 150.;

fn kind_name(layer: &Layer) -> &'static str {
    match layer.kind {
        LayerKind::Text { .. } => "Text",
        LayerKind::Device { model, .. } => model.label(),
        LayerKind::Image { .. } => "Image",
        LayerKind::Shape { .. } => "Shape",
    }
}

/// A colour field: a well showing the colour, then its hex value.
fn colour_field(hex: &str, input: &Entity<InputState>, width: f32, cx: &App) -> Div {
    h_flex()
        .w(px(width))
        .h(px(28.))
        .pl(px(6.))
        .gap(px(4.))
        .items_center()
        .rounded(px(7.))
        .bg(cx.theme().foreground.opacity(0.06))
        .child(
            div()
                .size(px(16.))
                .flex_shrink_0()
                .rounded_full()
                .bg(color(hex))
                .border_1()
                .border_color(cx.theme().foreground.opacity(0.15)),
        )
        .child(Input::new(input).appearance(false).xsmall())
}

impl ScreenshotsView {
    pub(super) fn inspector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let body = match self.selected_layer().cloned() {
            Some(layer) => self.layer_properties(&layer, cx).into_any_element(),
            None => self.slide_properties(cx).into_any_element(),
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
                    .id("inspector")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(12.))
                    .pt(px(14.))
                    .pb(px(20.))
                    .gap(px(18.))
                    .child(body),
            )
    }

    fn swatch_strip(&self, current: &str, cx: &mut Context<Self>) -> Div {
        h_flex()
            .gap(px(5.))
            .children(SWATCHES.iter().enumerate().map(|(ix, hex)| {
                controls::swatch(("swatch", ix), color(hex), current.eq_ignore_ascii_case(hex), cx)
                    .on_click(cx.listener(move |this, _, window, cx| this.set_color_value(hex, window, cx)))
            }))
    }

    fn colour_block(&self, current: &str, cx: &mut Context<Self>) -> Div {
        v_flex()
            .gap(px(8.))
            .child(self.swatch_strip(current, cx))
            .child(colour_field(current, &self.inputs.color, 120., cx))
    }

    fn layer_properties(&self, layer: &Layer, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let icon_button =
            |id: &'static str, icon: Icon, tip: &'static str| Button::new(id).ghost().small().icon(icon).tooltip(tip);

        let header = h_flex()
            .gap(px(10.))
            .items_center()
            .child(
                div()
                    .size(px(32.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(9.))
                    .bg(theme.primary.opacity(0.16))
                    .child(layer_icon(layer).size(px(16.)).text_color(theme.primary)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(layer.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child(kind_name(layer)),
                    ),
            )
            .child(
                icon_button("layer-copy", Icon::new(IconName::Copy), "Duplicate  ⌘D")
                    .on_click(cx.listener(|this, _, window, cx| this.duplicate_selected(window, cx))),
            )
            .child(
                icon_button("layer-delete", Icon::new(IconName::Delete), "Delete  ⌫")
                    .on_click(cx.listener(|this, _, window, cx| this.delete_selected(window, cx))),
            );

        let [x, y, w, h] = &self.inputs.geometry;
        let arrange = v_flex().child(heading("Arrange", cx)).child(
            group(cx)
                .child(
                    v_flex()
                        .p(px(8.))
                        .gap(px(6.))
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .child(controls::number_field("X", x, cx))
                                .child(controls::number_field("Y", y, cx)),
                        )
                        .child(
                            h_flex()
                                .gap(px(6.))
                                .child(controls::number_field("W", w, cx))
                                .child(controls::number_field("H", h, cx)),
                        ),
                )
                .child(separator(cx))
                .child(row(
                    "Order",
                    h_flex()
                        .gap_1()
                        .child(
                            Button::new("layer-down")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ChevronDown)
                                .label("Back")
                                .tooltip("Send backward  ⌘[")
                                .on_click(cx.listener(|this, _, window, cx| this.restack(-1, false, window, cx))),
                        )
                        .child(
                            Button::new("layer-up")
                                .ghost()
                                .xsmall()
                                .icon(IconName::ChevronUp)
                                .label("Front")
                                .tooltip("Bring forward  ⌘]")
                                .on_click(cx.listener(|this, _, window, cx| this.restack(1, false, window, cx))),
                        ),
                    cx,
                )),
        );

        let shadow_switch = |on: bool, cx: &mut Context<Self>| {
            Switch::new("layer-shadow")
                .checked(on)
                .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                    if *checked != on {
                        this.toggle_shadow(window, cx);
                    }
                }))
        };

        let kind = match &layer.kind {
            LayerKind::Text {
                size,
                weight,
                align,
                color: current,
                italic,
                ..
            } => {
                let italic = *italic;
                let weight_ix = WEIGHTS.iter().position(|(_, w)| w == weight).unwrap_or(3);
                let align_ix = ALIGNS.iter().position(|(_, a)| a == align).unwrap_or(1);
                let weights = WEIGHTS
                    .iter()
                    .fold(Segmented::new("weight", ROW_CONTROL), |control, (name, w)| {
                        control.item(div().font_weight(FontWeight(*w as f32)).child("Aa"), *name)
                    });
                let aligns = [
                    gpui_kit::assets::IconName::TextAlignStart,
                    gpui_kit::assets::IconName::TextAlignCenter,
                    gpui_kit::assets::IconName::TextAlignEnd,
                ]
                .into_iter()
                .zip(ALIGNS)
                .fold(Segmented::new("align", 108.), |control, (icon, (name, _))| {
                    control.item(Icon::new(icon).size(px(15.)), *name)
                });
                v_flex()
                    .child(heading("Text", cx))
                    .child(
                        group(cx)
                            .child(
                                div()
                                    .px(px(4.))
                                    .py(px(2.))
                                    .text_size(px(13.))
                                    .child(Textarea::new(&self.inputs.text).appearance(false)),
                            )
                            .child(separator(cx))
                            .child(row(
                                "Font",
                                div()
                                    .w(px(ROW_CONTROL + 14.))
                                    .h(px(28.))
                                    .rounded(px(7.))
                                    .bg(theme.foreground.opacity(0.06))
                                    .child(
                                        Select::new(&self.fonts)
                                            .appearance(false)
                                            .small()
                                            .menu_width(px(250.))
                                            .search_placeholder("Search fonts")
                                            .placeholder("Helvetica Neue"),
                                    ),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(stacked(
                                "Size",
                                Some(format!("{size:.0} pt")),
                                Slider::new(&self.sliders.size),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(row(
                                "Weight",
                                weights
                                    .selected(weight_ix)
                                    .on_select(cx.listener(|this, ix: &usize, window, cx| {
                                        this.set_weight(WEIGHTS[*ix].1, window, cx)
                                    })),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(row(
                                "Italic",
                                Switch::new("text-italic").checked(italic).on_click(cx.listener(
                                    move |this, checked: &bool, window, cx| {
                                        if *checked != italic {
                                            this.toggle_italic(window, cx);
                                        }
                                    },
                                )),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(row(
                                "Alignment",
                                aligns
                                    .selected(align_ix)
                                    .on_select(cx.listener(|this, ix: &usize, window, cx| {
                                        this.set_align(ALIGNS[*ix].1, window, cx)
                                    })),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(stacked("Colour", None, self.colour_block(current, cx), cx)),
                    )
                    .into_any_element()
            }
            LayerKind::Device {
                model,
                frame,
                shadow,
                screenshot,
                ..
            } => {
                let model = *model;
                // Every iPhone and iPad, newest first; type to search.
                let model_control = div()
                    .w_full()
                    .h(px(28.))
                    .rounded(px(7.))
                    .bg(theme.foreground.opacity(0.06))
                    .child(
                        Select::new(&self.models)
                            .appearance(false)
                            .small()
                            .menu_width(px(CONTENT))
                            .search_placeholder("Search devices")
                            .placeholder(model.label()),
                    );
                let framed = *frame != Frame::None;
                let finishes = h_flex().gap(px(5.)).children(
                    Frame::ALL
                        .into_iter()
                        .filter(|f| *f != Frame::None)
                        .enumerate()
                        .map(|(ix, finish)| {
                            let edge = finish.edge_color().unwrap_or("#000000");
                            controls::swatch(("finish", ix), color(edge), finish == *frame, cx)
                                .tooltip(move |window, cx| Tooltip::new(finish.label()).build(window, cx))
                                .on_click(cx.listener(move |this, _, window, cx| this.set_frame(finish, window, cx)))
                        }),
                );
                v_flex()
                    .child(heading(model.target().label(), cx))
                    .child(
                        group(cx)
                            .child(row(
                                "Screen",
                                Button::new("device-screenshot")
                                    .small()
                                    .icon(IconName::FolderOpen)
                                    .label(if screenshot.is_some() {
                                        "Replace…"
                                    } else {
                                        "Choose…"
                                    })
                                    .on_click(cx.listener(|this, _, window, cx| this.choose_screenshot(window, cx))),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(stacked("Model", None, model_control, cx))
                            .child(separator(cx))
                            .child(row(
                                "Frame",
                                Switch::new("device-frame").checked(framed).on_click(cx.listener(
                                    move |this, checked: &bool, window, cx| {
                                        if *checked != framed {
                                            this.set_frame(if *checked { Frame::Black } else { Frame::None }, window, cx)
                                        }
                                    },
                                )),
                                cx,
                            ))
                            .when(framed, |this| {
                                this.child(separator(cx)).child(stacked(
                                    "Finish",
                                    Some(frame.label().to_owned()),
                                    finishes,
                                    cx,
                                ))
                            })
                            .child(separator(cx))
                            .child(row("Shadow", shadow_switch(*shadow, cx), cx)),
                    )
                    .into_any_element()
            }
            LayerKind::Image { radius, shadow, .. } => v_flex()
                .child(heading("Image", cx))
                .child(
                    group(cx)
                        .child(row(
                            "Picture",
                            Button::new("image-replace")
                                .small()
                                .icon(IconName::FolderOpen)
                                .label("Replace…")
                                .on_click(cx.listener(|this, _, window, cx| this.replace_image(window, cx))),
                            cx,
                        ))
                        .child(separator(cx))
                        .child(stacked(
                            "Corner radius",
                            Some(format!("{radius:.0}")),
                            Slider::new(&self.sliders.radius),
                            cx,
                        ))
                        .child(separator(cx))
                        .child(row("Shadow", shadow_switch(*shadow, cx), cx)),
                )
                .into_any_element(),
            LayerKind::Shape { radius, fill, .. } => v_flex()
                .child(heading("Shape", cx))
                .child(
                    group(cx)
                        .child(stacked("Fill", None, self.colour_block(fill, cx), cx))
                        .child(separator(cx))
                        .child(stacked(
                            "Corner radius",
                            Some(format!("{radius:.0}")),
                            Slider::new(&self.sliders.radius),
                            cx,
                        )),
                )
                .into_any_element(),
        };

        let appearance = v_flex().child(heading("Appearance", cx)).child(group(cx).child(stacked(
            "Opacity",
            Some(format!("{:.0}%", layer.opacity * 100.)),
            Slider::new(&self.sliders.opacity),
            cx,
        )));

        v_flex()
            .gap(px(18.))
            .child(header)
            .when(layer.locked, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .px(px(10.))
                        .py(px(8.))
                        .rounded(px(10.))
                        .bg(theme.warning.opacity(0.12))
                        .text_size(px(12.))
                        .child(
                            Icon::new(gpui_kit::assets::IconName::Lock)
                                .size(px(14.))
                                .text_color(theme.warning),
                        )
                        .child("Locked. Unlock it in Layers to move it."),
                )
            })
            .child(kind)
            .child(arrange)
            .child(appearance)
            .when(self.design.video.is_some(), |this| this.child(self.motion_properties(layer, cx)))
    }

    /// When the layer enters, leaves and drifts in the video.
    fn motion_properties(&self, layer: &Layer, cx: &mut Context<Self>) -> Div {
        let motion = &layer.motion;
        let effects = |id: &'static str, current: Effect, leaving: bool, cx: &mut Context<Self>| {
            Effect::ALL
                .iter()
                .fold(Segmented::new(id, CONTENT - 20.), |control, effect| control.label(effect.label()))
                .selected(Effect::ALL.iter().position(|e| *e == current).unwrap_or(0))
                .on_select(cx.listener(move |this, ix: &usize, window, cx| {
                    this.set_effect(Effect::ALL[*ix], leaving, window, cx)
                }))
        };
        let drifts = DRIFTS
            .iter()
            .fold(Segmented::new("drift", CONTENT - 20.), |control, (_, label)| control.label(*label))
            .selected(DRIFTS.iter().position(|(d, _)| *d == motion.drift).unwrap_or(0))
            .on_select(cx.listener(|this, ix: &usize, window, cx| this.set_drift(DRIFTS[*ix].0, window, cx)));
        v_flex().child(heading("Motion", cx)).child(
            group(cx)
                .child(stacked("Enter", None, effects("enter", motion.enter, false, cx), cx))
                .child(separator(cx))
                .child(stacked(
                    "Comes in at",
                    Some(format!("{:.1} s", motion.start)),
                    Slider::new(&self.sliders.start),
                    cx,
                ))
                .child(separator(cx))
                .child(stacked("Exit", None, effects("exit", motion.exit, true, cx), cx))
                .child(separator(cx))
                .child(stacked(
                    "Leaves at",
                    Some(motion.leave.map_or_else(|| "Stays".to_owned(), |t| format!("{t:.1} s"))),
                    Slider::new(&self.sliders.leave),
                    cx,
                ))
                .child(separator(cx))
                .child(stacked("Drift", None, drifts, cx)),
        )
    }

    /// Plays the slides as scenes: on or off, this scene's length, the
    /// transition, and the total against Apple's limits.
    fn video_properties(&self, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let video = self.design.video.clone();
        let length = self.duration();
        let problem = video.as_ref().and_then(|_| appdoctor_video::length_problem(&self.design));
        let scene = self.current().map_or(0., |slide| slide.duration);
        let about = match self.target {
            Target::Iphone | Target::Ipad => {
                "Plays these slides as scenes: an App Store app preview, 15–30 seconds, exported at Apple's size."
            }
            Target::Header | Target::Search => {
                "A looping, muted 5–30 second video for this spot. iOS 27 plays it in place of the image."
            }
        };
        let on = video.is_some();
        v_flex()
            .child(heading("Video", cx))
            .child(
                group(cx)
                    .child(row(
                        "Make a video",
                        Switch::new("video-on")
                            .checked(on)
                            .on_click(cx.listener(move |this, checked: &bool, window, cx| {
                                if *checked != on {
                                    this.set_video(*checked, window, cx)
                                }
                            })),
                        cx,
                    ))
                    .when_some(video, |this, video| {
                        let transitions = Transition::ALL
                            .iter()
                            .fold(Segmented::new("transition", ROW_CONTROL), |control, t| control.label(t.label()))
                            .selected(Transition::ALL.iter().position(|t| *t == video.transition).unwrap_or(0))
                            .on_select(cx.listener(|this, ix: &usize, window, cx| {
                                this.set_transition(Transition::ALL[*ix], window, cx)
                            }));
                        this.child(separator(cx))
                            .child(stacked(
                                "This scene",
                                Some(format!("{scene:.1} s")),
                                Slider::new(&self.sliders.scene),
                                cx,
                            ))
                            .child(separator(cx))
                            .child(row("Transition", transitions, cx))
                            .child(separator(cx))
                            .child(row(
                                "Length",
                                div()
                                    .text_size(px(13.))
                                    .text_color(if problem.is_some() { theme.warning } else { theme.muted_foreground })
                                    .child(format!("{length:.1} s")),
                                cx,
                            ))
                    }),
            )
            .child(
                div()
                    .px(px(4.))
                    .pt(px(6.))
                    .text_size(px(11.))
                    .text_color(if problem.is_some() { theme.warning } else { theme.muted_foreground })
                    .child(problem.unwrap_or_else(|| about.to_owned())),
            )
    }

    fn slide_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(slide) = self.current().cloned() else {
            return v_flex();
        };
        let (gradient, from, to, angle) = match &slide.background {
            Background::Gradient { from, to, angle } => (true, from.clone(), to.clone(), angle.round() as i32),
            Background::Solid { color } => (false, color.clone(), color.clone(), 180),
        };

        // Three across; a pixel spare for the panel's border.
        let tile_w = ((CONTENT - 18.) / 3.).floor();
        let presets = BACKGROUND_PRESETS.iter().enumerate().map(|(ix, (name, p_from, p_to, text))| {
            let chosen = gradient && from.eq_ignore_ascii_case(p_from) && to.eq_ignore_ascii_case(p_to);
            v_flex()
                .id(("preset", ix))
                .w(px(tile_w))
                .gap(px(5.))
                .items_center()
                .cursor_pointer()
                .child(
                    div()
                        .w_full()
                        .h(px(46.))
                        .rounded(px(10.))
                        .border_2()
                        .border_color(if chosen { theme.primary } else { transparent_black() })
                        .p(px(2.))
                        .child(
                            div()
                                .size_full()
                                .rounded(px(7.))
                                .bg(linear_gradient(
                                    160.,
                                    linear_color_stop(color(p_from), 0.),
                                    linear_color_stop(color(p_to), 1.),
                                ))
                                .border_1()
                                .border_color(theme.foreground.opacity(0.08)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(if chosen {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(*name),
                )
                .tooltip(move |window, cx| Tooltip::new(format!("{name} background")).build(window, cx))
                .on_click(cx.listener(move |this, _, window, cx| {
                    let angle = match this.current().map(|s| &s.background) {
                        Some(Background::Gradient { angle, .. }) => *angle,
                        _ => 160.,
                    };
                    let text = text.to_string();
                    this.apply(window, cx, |this| {
                        if let Some(slide) = this.current_mut() {
                            slide.background = Background::Gradient {
                                from: p_from.to_string(),
                                to: p_to.to_string(),
                                angle,
                            };
                            // Keep text readable on the new background.
                            for layer in &mut slide.layers {
                                if let LayerKind::Text { color, .. } = &mut layer.kind
                                    && (color.eq_ignore_ascii_case("#ffffff") || color.eq_ignore_ascii_case("#0f172a"))
                                {
                                    *color = text.clone();
                                }
                            }
                        }
                    });
                }))
        });

        let kind = Segmented::new("bg-kind", 140.)
            .label("Gradient")
            .label("Solid")
            .selected(if gradient { 0 } else { 1 })
            .on_select(cx.listener(|this, ix: &usize, window, cx| {
                let next = match (this.current().map(|s| s.background.clone()), *ix == 0) {
                    (Some(Background::Solid { color }), true) => Background::Gradient {
                        from: color.clone(),
                        to: color,
                        angle: 160.,
                    },
                    (Some(Background::Gradient { from, .. }), false) => Background::Solid { color: from },
                    (Some(background), _) => background,
                    (None, _) => return,
                };
                this.set_background(next, window, cx);
            }));
        let angle_ix = ANGLES.iter().position(|(_, _, a)| *a == angle);
        let angles = ANGLES
            .iter()
            .fold(Segmented::new("angle", ROW_CONTROL), |control, (arrow, name, _)| {
                control.item(div().text_size(px(13.)).child(*arrow), *name)
            })
            .selected(angle_ix.unwrap_or(0))
            .on_select(cx.listener(|this, ix: &usize, window, cx| {
                if let Some(Background::Gradient { from, to, .. }) = this.current().map(|s| s.background.clone()) {
                    let angle = ANGLES[*ix].2 as f32;
                    this.set_background(Background::Gradient { from, to, angle }, window, cx);
                }
            }));

        let fill = group(cx)
            .child(row("Fill", kind, cx))
            .child(separator(cx))
            .child(row(
                if gradient { "From" } else { "Colour" },
                colour_field(&from, &self.inputs.from, 128., cx),
                cx,
            ))
            .when(gradient, |this| {
                this.child(separator(cx))
                    .child(row("To", colour_field(&to, &self.inputs.to, 128., cx), cx))
                    .child(separator(cx))
                    .child(row("Direction", angles, cx))
            });

        let tips = v_flex()
            .gap(px(6.))
            .px(px(4.))
            .text_size(px(12.))
            .text_color(theme.muted_foreground)
            .child("Click a layer to edit it. Drag to move it (hold ⌥ to skip snapping), drag a corner to resize it, and use the arrow keys to nudge.")
            .when(self.target.is_screenshot(), |this| {
                this.child("Drag a device past the edge to carry it onto the next screenshot.")
            })
            .when(self.target == Target::Header, |this| {
                this.child(
                    "The product page header shows on iOS 27 and iPadOS 27. Keep words inside the dashed line: \
                     the App Store crops the rest on some devices. Each version is an alternative to try.",
                )
            })
            .when(self.target == Target::Search, |this| {
                this.child(
                    "Search result art shows in App Store search on iOS 27 and iPadOS 27. Make what the app does \
                     obvious at a glance.",
                )
            })
            .child("Ask your AI agent to design them too: AI agent, top right.");

        v_flex()
            .gap(px(18.))
            .child(
                v_flex()
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("{} {}", self.slide_noun(), self.slide + 1)),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme.muted_foreground)
                            .child({
                                let sizes = self.target.sizes();
                                let (w, h) = sizes[0].pixels();
                                match sizes.get(1) {
                                    Some(also) => format!("{} · {w} × {h}, also exports {}", sizes[0].label(), also.label()),
                                    None => format!("{} · {w} × {h}", sizes[0].label()),
                                }
                            }),
                    ),
            )
            .child(
                v_flex()
                    .child(heading("Background", cx))
                    .child(h_flex().flex_wrap().gap(px(8.)).pb(px(12.)).children(presets))
                    .child(fill),
            )
            .child(self.video_properties(cx))
            .child(tips)
    }
}
