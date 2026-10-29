//! The Apps page: add an app by App Store link or ID, and manage tracked apps.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;
use crate::ui::widgets::{self, CountrySelect, app_icon, card, compact, country_select, flag};

pub struct AppsView {
    state: Entity<AppState>,
    link: Entity<InputState>,
    country: Entity<CountrySelect>,
    adding: bool,
    _subscriptions: Vec<Subscription>,
}

impl AppsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let link = cx.new(|cx| {
            InputState::new(window, cx).placeholder("https://apps.apple.com/us/app/…/id1234567890  or  1234567890")
        });
        let country = cx.new(|cx| country_select("us", window, cx));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe_in(&link, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.add(window, cx);
                }
            }),
        ];
        Self {
            state,
            link,
            country,
            adding: false,
            _subscriptions: subscriptions,
        }
    }

    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.link.read(cx).value().trim().to_owned();
        if input.is_empty() || self.adding {
            return;
        }
        let country = self
            .country
            .read(cx)
            .selected_value()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "us".into());
        self.adding = true;
        cx.notify();
        let task = self.state.update(cx, |state, cx| state.add_app(input, country, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.adding = false;
                match result {
                    Ok(app) => {
                        this.link.update(cx, |input, cx| input.set_value("", window, cx));
                        window.push_notification(Notification::success(format!("Tracking {}", app.name)), cx);
                    }
                    Err(error) => window.push_notification(Notification::error(format!("{error:#}")), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

impl Render for AppsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let apps = state.apps.clone();
        let selected = state.selected_app;
        let icons = state.icons.clone();
        let muted = cx.theme().muted_foreground;

        let cards: Vec<_> = apps
            .into_iter()
            .map(|app| {
                let id = app.id;
                let icon = app
                    .icon_url
                    .clone()
                    .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
                let keyword_count = self.state.read(cx).keyword_count(id);
                card(cx)
                    .id(("app-card", id))
                    .w(px(300.))
                    .gap_3()
                    .when(selected == Some(id), |this| this.border_color(cx.theme().primary))
                    .child(
                        h_flex()
                            .gap_3()
                            .items_center()
                            .child(app_icon(icon, px(48.), cx))
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(app.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(muted)
                                            .truncate()
                                            .child(app.developer.clone().unwrap_or_default()),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap_3()
                            .text_xs()
                            .text_color(muted)
                            .child(format!(
                                "★ {:.1} · {} ratings",
                                app.average_rating,
                                compact(app.rating_count)
                            ))
                            .child(format!("{} {}", flag(&app.country), app.country.to_uppercase()))
                            .child(format!("{keyword_count} keywords")),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(("open-app", id))
                                    .small()
                                    .primary()
                                    .label("Open keywords")
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.state.update(cx, |state, cx| {
                                            state.select_app(id, cx);
                                            state.set_section(crate::state::Section::Keywords, cx);
                                        })
                                    })),
                            )
                            .child(
                                Button::new(("remove-app", id))
                                    .small()
                                    .ghost()
                                    .label("Remove")
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Err(error) = this.state.update(cx, |state, cx| state.remove_app(id, cx))
                                        {
                                            window.push_notification(Notification::error(error.to_string()), cx);
                                        }
                                    })),
                            ),
                    )
            })
            .collect();

        v_flex()
            .id("apps-page")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_5()
            .child(widgets::page_header(
                "Apps",
                "Track your own apps and your competitors'. Data comes straight from the App Store to this Mac.",
                cx,
            ))
            .child(
                card(cx)
                    .child(div().text_sm().font_weight(FontWeight::MEDIUM).child("Add an app"))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(div().flex_1().child(Input::new(&self.link)))
                            .child(div().w(px(200.)).child(Select::new(&self.country).menu_width(px(260.))))
                            .child(
                                Button::new("add-app")
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label("Add app")
                                    .loading(self.adding)
                                    .disabled(self.adding)
                                    .on_click(cx.listener(|this, _, window, cx| this.add(window, cx))),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Paste a link from the App Store or an app's numeric ID. The country picks which storefront to look it up in when the link doesn't name one."),
                    ),
            )
            .child(h_flex().flex_wrap().gap_4().children(cards))
    }
}
