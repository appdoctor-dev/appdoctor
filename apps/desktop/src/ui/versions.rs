//! What the two version histories share: Screenshots and Localization each
//! list earlier versions in a panel, to look at, restore or name.

use std::rc::Rc;

use chrono::{Datelike as _, Local, TimeZone as _};
use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

/// When a version was saved: "Today 14:02", "Yesterday 09:12", "3 Nov 14:02",
/// with the year once it isn't this one.
pub fn when(timestamp: i64) -> String {
    let Some(at) = Local.timestamp_opt(timestamp, 0).single() else {
        return String::new();
    };
    let now = Local::now();
    let days = now.date_naive().signed_duration_since(at.date_naive()).num_days();
    match days {
        0 => at.format("Today %H:%M").to_string(),
        1 => at.format("Yesterday %H:%M").to_string(),
        _ if at.year() == now.year() => at.format("%-d %b %H:%M").to_string(),
        _ => at.format("%-d %b %Y").to_string(),
    }
}

/// A version's name in a list.
pub fn title(label: &str) -> String {
    if label.is_empty() {
        "Autosaved".into()
    } else {
        label.to_owned()
    }
}

/// The top of a history panel: its title, a close button and a field to name
/// and save what's there now.
pub fn panel_top(
    subtitle: impl Into<SharedString>,
    name: &Entity<InputState>,
    close: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    save: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    v_flex()
        .flex_shrink_0()
        .px(px(12.))
        .pt(px(12.))
        .pb(px(12.))
        .gap(px(10.))
        .border_b_1()
        .border_color(theme.border)
        .child(
            h_flex()
                .items_start()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Version History"),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child(subtitle.into()),
                        ),
                )
                .child(
                    Button::new("close-history")
                        .ghost()
                        .xsmall()
                        .icon(IconName::Close)
                        .tooltip("Close")
                        .on_click(close),
                ),
        )
        .child(
            h_flex()
                .gap(px(6.))
                .child(div().flex_1().min_w_0().child(Input::new(name).small()))
                .child(
                    Button::new("save-version")
                        .small()
                        .icon(ExtraIcon::Bookmark)
                        .label("Save")
                        .tooltip("Keep what's here now as a named version")
                        .on_click(save),
                ),
        )
}

/// One row of the list, without its contents.
pub fn row(id: impl Into<ElementId>, selected: bool, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    let hover = theme.foreground.opacity(0.05);
    v_flex()
        .id(id)
        .px(px(9.))
        .py(px(8.))
        .gap(px(6.))
        .rounded(px(9.))
        .cursor_pointer()
        .when(selected, |this| this.bg(theme.primary.opacity(0.14)))
        .when(!selected, |this| this.hover(|style| style.bg(hover)))
}

/// A version's name, with when it was saved and anything else worth saying.
pub fn heading(title: String, named: bool, detail: String, cx: &App) -> Div {
    let theme = cx.theme();
    v_flex()
        .min_w_0()
        .child(
            h_flex()
                .gap(px(5.))
                .items_center()
                .when(named, |this| {
                    this.child(Icon::new(ExtraIcon::Bookmark).size(px(11.)).text_color(theme.primary))
                })
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(11.))
                .text_color(theme.muted_foreground)
                .child(detail),
        )
}

/// The buttons on the selected row. Restoring is safe to click: what's there
/// now is saved first.
pub fn actions(
    restore: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    delete: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    h_flex()
        .gap(px(6.))
        .when_some(restore, |this, restore| {
            this.child(
                Button::new("restore-version")
                    .primary()
                    .small()
                    .icon(ExtraIcon::RotateCcw)
                    .label("Restore")
                    .on_click(restore),
            )
        })
        .child(div().flex_1())
        .child(
            Button::new("delete-version")
                .ghost()
                .small()
                .icon(IconName::Delete)
                .tooltip("Delete this version")
                .on_click(delete),
        )
}

/// Asks before deleting a version for good.
pub fn confirm_delete(
    version: String,
    window: &mut Window,
    cx: &mut App,
    delete: impl Fn(&mut Window, &mut App) + 'static,
) {
    confirm(
        "Delete this version?".into(),
        format!("“{version}” can't be brought back after this. What's there now doesn't change."),
        "Delete",
        window,
        cx,
        delete,
    );
}

/// A dialog that asks before something destructive.
pub fn confirm(
    title: String,
    body: String,
    action: &'static str,
    window: &mut Window,
    cx: &mut App,
    confirmed: impl Fn(&mut Window, &mut App) + 'static,
) {
    let confirmed = Rc::new(confirmed);
    window.open_dialog(cx, move |dialog, _, cx| {
        let confirmed = confirmed.clone();
        dialog
            .title(title.clone())
            .w(px(420.))
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(cx.theme().muted_foreground)
                    .child(body.clone()),
            )
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-delete")
                            .ghost()
                            .label("Cancel")
                            .on_click(|_, window, cx| window.close_dialog(cx)),
                    )
                    .child(
                        Button::new("confirm-delete")
                            .danger()
                            .label(action)
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                confirmed(window, cx);
                            }),
                    ),
            )
    });
}
