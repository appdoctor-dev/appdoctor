//! Inspector controls in the manner of macOS's own inspectors (Keynote,
//! Freeform): grouped cards of label/value rows, segmented controls with a
//! sliding thumb, and round colour wells.

use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// The translucent fill behind grouped controls, readable in both modes.
pub fn fill(cx: &App) -> Hsla {
    cx.theme().foreground.opacity(0.045)
}

/// A rounded group of rows, like a grouped list in System Settings.
pub fn group(cx: &App) -> Div {
    v_flex()
        .rounded(px(10.))
        .bg(fill(cx))
        .border_1()
        .border_color(cx.theme().foreground.opacity(0.05))
}

/// A section heading above a group.
pub fn heading(text: impl Into<SharedString>, cx: &App) -> Div {
    h_flex()
        .px(px(4.))
        .pb(px(6.))
        .items_center()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().foreground)
        .child(text.into())
}

fn label(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(13.))
        .text_color(cx.theme().foreground.opacity(0.85))
        .child(text.into())
}

/// A row in a group: its label on the left, its control on the right.
pub fn row(name: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> Div {
    h_flex()
        .min_h(px(38.))
        .px(px(10.))
        .gap_2()
        .items_center()
        .justify_between()
        .child(label(name, cx))
        .child(control)
}

/// A row whose control is too wide to sit beside its label (a slider, a
/// swatch strip): the label and its current value above, the control below.
pub fn stacked(name: impl Into<SharedString>, value: Option<String>, control: impl IntoElement, cx: &App) -> Div {
    v_flex()
        .px(px(10.))
        .pt(px(9.))
        .pb(px(10.))
        .gap(px(8.))
        .child(
            h_flex()
                .justify_between()
                .child(label(name, cx))
                .when_some(value, |this, value| {
                    this.child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(value),
                    )
                }),
        )
        .child(control)
}

/// A hairline between rows, inset from the left like AppKit's.
pub fn separator(cx: &App) -> Div {
    div().ml(px(10.)).h(px(1.)).bg(cx.theme().foreground.opacity(0.07))
}

/// A small editable number with its letter (X, Y, W, H) inside the field.
pub fn number_field(letter: &'static str, input: &Entity<InputState>, cx: &App) -> Div {
    h_flex()
        .flex_1()
        .h(px(28.))
        .pl(px(9.))
        .items_center()
        .rounded(px(7.))
        .bg(cx.theme().foreground.opacity(0.06))
        .child(
            div()
                .w(px(12.))
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(letter),
        )
        .child(Input::new(input).appearance(false).xsmall())
}

/// A round colour well; the chosen one gets a ring.
pub fn swatch(id: impl Into<ElementId>, color: Hsla, chosen: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id)
        .size(px(24.))
        .p(px(2.))
        .flex_shrink_0()
        .rounded_full()
        .border_2()
        .border_color(if chosen { theme.primary } else { transparent_black() })
        .cursor_pointer()
        .child(
            div()
                .size_full()
                .rounded_full()
                .bg(color)
                .border_1()
                .border_color(theme.foreground.opacity(0.12)),
        )
}

const HEIGHT: f32 = 28.;
const INSET: f32 = 2.;

type SelectHandler = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

struct Segment {
    content: AnyElement,
    tooltip: Option<SharedString>,
}

/// A macOS segmented control. Its thumb slides to the new choice.
#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    width: f32,
    items: Vec<Segment>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl Segmented {
    pub fn new(id: impl Into<SharedString>, width: f32) -> Self {
        Self {
            id: id.into(),
            width,
            items: Vec::new(),
            selected: 0,
            on_select: None,
        }
    }

    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.items.push(Segment {
            content: text.into().into_any_element(),
            tooltip: None,
        });
        self
    }

    /// A segment drawn by `content` (an icon, a sample), named by `tooltip`.
    pub fn item(mut self, content: impl IntoElement, tooltip: impl Into<SharedString>) -> Self {
        self.items.push(Segment {
            content: content.into_any_element(),
            tooltip: Some(tooltip.into()),
        });
        self
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// Called with the chosen segment's index; `cx.listener` makes one.
    pub fn on_select(mut self, handler: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Segmented {
            id,
            width,
            items,
            selected,
            on_select,
        } = self;
        let theme = cx.theme().clone();
        let count = items.len().max(1);
        let segment = (width - INSET * 2.) / count as f32;
        let selected = selected.min(count - 1);

        // The thumb slides from the choice this control showed last.
        let state = window.use_keyed_state(ElementId::Name(id.clone()), cx, |_, _| (selected, selected));
        let (mut from, mut to) = *state.read(cx);
        if to != selected {
            (from, to) = (to, selected);
            state.update(cx, |state, _| *state = (from, to));
        }

        let thumb = div()
            .absolute()
            .top(px(INSET))
            .h(px(HEIGHT - INSET * 2.))
            .w(px(segment))
            .rounded(px(6.))
            .bg(if theme.mode.is_dark() {
                hsla(0., 0., 1., 0.17)
            } else {
                hsla(0., 0., 1., 1.)
            })
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.18),
                offset: point(px(0.), px(1.)),
                blur_radius: px(3.),
                spread_radius: px(0.),
                inset: false,
            }])
            .with_animation(
                ElementId::NamedInteger(SharedString::from(format!("{id}-thumb")), (from * 1000 + to) as u64),
                Animation::new(Duration::from_millis(260)).with_easing(ease_out_quint()),
                move |this, delta| {
                    let at = from as f32 + (to as f32 - from as f32) * delta;
                    this.left(px(INSET + segment * at))
                },
            );

        let segments = items.into_iter().enumerate().map(|(ix, item)| {
            let on_select = on_select.clone();
            let chosen = ix == selected;
            div()
                .id(ElementId::NamedInteger(id.clone(), ix as u64))
                .w(px(segment))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .font_weight(if chosen {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::MEDIUM
                })
                .text_color(if chosen {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .cursor_pointer()
                .when(!chosen, |this| this.hover(|style| style.text_color(theme.foreground)))
                .when_some(item.tooltip, |this, tip| {
                    this.tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                })
                .on_click(move |_, window, cx| {
                    if let Some(handler) = &on_select {
                        handler(&ix, window, cx);
                    }
                })
                .child(item.content)
        });

        div()
            .relative()
            .flex_shrink_0()
            .w(px(width))
            .h(px(HEIGHT))
            .rounded(px(8.))
            .bg(theme.foreground.opacity(0.07))
            .child(thumb)
            .child(h_flex().absolute().top_0().left(px(INSET)).h_full().children(segments))
    }
}

// Form pieces for the settings-style pages.

/// The soft rounded well a borderless input sits in.
pub fn well(cx: &App) -> Div {
    h_flex()
        .min_h(px(28.))
        .px(px(4.))
        .items_center()
        .rounded(px(7.))
        .bg(cx.theme().foreground.opacity(0.06))
}

/// A single-line text field for a group row; size it with `.w()`.
pub fn text_field(input: &Entity<InputState>, cx: &App) -> Div {
    well(cx).child(
        div()
            .flex_1()
            .min_w_0()
            .child(Input::new(input).appearance(false).small()),
    )
}

/// Explanatory text under a group, as System Settings sets it.
pub fn footnote(text: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .px(px(4.))
        .pt(px(7.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
}

/// A group whose rows are divided by separators.
pub fn rows(items: impl IntoIterator<Item = AnyElement>, cx: &App) -> Div {
    let mut list = group(cx);
    for (ix, item) in items.into_iter().enumerate() {
        if ix > 0 {
            list = list.child(separator(cx));
        }
        list = list.child(item);
    }
    list
}

/// A read-only label/value list, for confirmation dialogs.
pub fn summary(items: impl IntoIterator<Item = (String, String)>, cx: &App) -> Div {
    let muted = cx.theme().muted_foreground;
    rows(
        items.into_iter().map(|(name, value)| {
            h_flex()
                .px(px(10.))
                .py(px(8.))
                .gap(px(16.))
                .items_start()
                .justify_between()
                .text_size(px(13.))
                .child(div().flex_shrink_0().text_color(muted).child(name))
                .child(div().min_w_0().text_right().child(value))
                .into_any_element()
        }),
        cx,
    )
}

/// A radio button: the accent with a white dot when chosen.
pub fn radio(chosen: bool, cx: &App) -> Div {
    let theme = cx.theme();
    div()
        .size(px(16.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .map(|this| {
            if chosen {
                this.bg(theme.primary)
                    .child(div().size(px(6.)).rounded_full().bg(hsla(0., 0., 1., 1.)))
            } else {
                this.bg(theme.foreground.opacity(0.06))
                    .border_1()
                    .border_color(theme.foreground.opacity(0.25))
            }
        })
}

/// The outcome of an action in a tinted strip: a green dot when it worked,
/// red text when it didn't.
pub fn banner(result: &Result<String, String>, cx: &App) -> Div {
    let theme = cx.theme();
    let (color, text, text_color) = match result {
        Ok(text) => (crate::theme::meter::up(), text, theme.foreground),
        Err(text) => (theme.danger, text, theme.danger),
    };
    h_flex()
        .gap(px(8.))
        .px(px(10.))
        .py(px(7.))
        .items_center()
        .rounded(px(8.))
        .bg(color.opacity(0.1))
        .text_size(px(12.))
        .child(div().size(px(7.)).flex_shrink_0().rounded_full().bg(color))
        .child(div().flex_1().min_w_0().text_color(text_color).child(text.clone()))
}
