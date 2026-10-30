//! The Apps page: find an app by name (type-ahead App Store search) or by
//! link or ID, and manage tracked apps.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;
use crate::ui::app_search::{AppSearch, AppSearchEvent};
use crate::ui::widgets::{self, CountrySelect, app_icon, card, compact, country_select, flag};

pub struct AppsView {
    state: Entity<AppState>,
    search: Entity<AppSearch>,
    country: Entity<CountrySelect>,
    adding: bool,
    _subscriptions: Vec<Subscription>,
}

impl AppsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            AppSearch::new(
                state.clone(),
                "Search the App Store by name, or paste a link or app ID",
                window,
                cx,
            )
        });
        let country = cx.new(|cx| country_select("us", window, cx));
        let subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                let taken = state.read(cx).apps.iter().map(|app| app.id).collect();
                this.search.update(cx, |search, _| search.set_taken(taken));
                cx.notify();
            }),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                AppSearchEvent::Picked(hit) => {
                    let id = hit.id.0;
                    if this.state.read(cx).app(id).is_some() {
                        this.state.update(cx, |state, cx| {
                            state.select_app(id, cx);
                            state.set_section(crate::state::Section::Keywords, cx);
                        });
                    } else {
                        this.add_ref(id.to_string(), window, cx);
                    }
                }
                AppSearchEvent::Reference(text) => this.add_ref(text.clone(), window, cx),
            }),
            cx.subscribe(&country, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(code)) = event {
                    let code = *code;
                    this.search.update(cx, |search, cx| search.set_country(code, cx));
                }
            }),
        ];
        Self {
            state,
            search,
            country,
            adding: false,
            _subscriptions: subscriptions,
        }
    }

    fn country(&self, cx: &App) -> String {
        self.country
            .read(cx)
            .selected_value()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "us".into())
    }

    fn add_ref(&mut self, input: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.adding {
            return;
        }
        let country = self.country(cx);
        self.adding = true;
        cx.notify();
        let tracked = self.state.read(cx).apps.iter().map(|app| app.id).collect::<Vec<_>>();
        let task = self.state.update(cx, |state, cx| state.add_app(input, country, cx));
        cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            this.update_in(cx, |this, window, cx| {
                this.adding = false;
                match result {
                    Ok(app) => {
                        this.search.update(cx, |search, cx| search.clear(window, cx));
                        let message = if tracked.contains(&app.id) {
                            format!(
                                "Tracking {} in {} {} too",
                                app.name,
                                flag(&app.country),
                                appdoctor_appstore::country(&app.country).map_or_else(|| app.country.to_uppercase(), |c| c.name.to_owned())
                            )
                        } else {
                            format!("Tracking {}", app.name)
                        };
                        window.push_notification(Notification::success(message), cx);
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

impl AppsView {
    /// The storefronts an app is tracked in: each opens its keywords there,
    /// and all but the first can be removed.
    fn country_chips(&self, id: u64, home: &str, countries: Vec<String>, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        h_flex()
            .flex_wrap()
            .gap(px(5.))
            .children(countries.into_iter().map(|country| {
                let removable = country != home;
                let code = country.clone();
                h_flex()
                    .id(SharedString::from(format!("app-{id}-country-{country}")))
                    .gap(px(4.))
                    .pl(px(7.))
                    .pr(px(if removable { 3. } else { 7. }))
                    .py(px(2.))
                    .rounded(px(6.))
                    .text_xs()
                    .bg(theme.foreground.opacity(0.06))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.foreground.opacity(0.1)))
                    .child(format!("{} {}", flag(&country), country.to_uppercase()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.state.update(cx, |state, cx| state.open_app_in(id, &code, cx));
                    }))
                    .when(removable, |this| {
                        let code = country.clone();
                        this.child(
                            Button::new(SharedString::from(format!("app-{id}-drop-{country}")))
                                .ghost()
                                .xsmall()
                                .icon(IconName::Close)
                                .tooltip("Stop tracking it here")
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    let result = this.state.update(cx, |state, cx| state.remove_country(id, &code, cx));
                                    if let Err(error) = result {
                                        window.push_notification(Notification::error(format!("{error:#}")), cx);
                                    }
                                })),
                        )
                    })
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Add it again from another country to track it there too."),
            )
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
                let countries = self.state.read(cx).countries(id);
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
                            .child(format!("{keyword_count} keywords")),
                    )
                    .child(self.country_chips(id, &app.country, countries, cx))
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
                                        confirm_remove_app(this.state.clone(), id, window, cx)
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
                            .child(div().flex_1().child(self.search.clone()))
                            .child(div().w(px(200.)).child(Select::new(&self.country).menu_width(px(260.))))
                            .child(
                                Button::new("add-app")
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label("Add app")
                                    .loading(self.adding)
                                    .disabled(self.adding)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.search.update(cx, |search, cx| search.submit_from_button(cx))
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("Type an app's name to search, or paste an App Store link or ID. The country picks the storefront to search and look it up in."),
                    ),
            )
            .child(h_flex().flex_wrap().gap_4().children(cards))
    }
}

/// Asks before removing an app: it deletes the app's keywords and their
/// rank history from this computer.
pub fn confirm_remove_app(state: Entity<AppState>, id: u64, window: &mut Window, cx: &mut App) {
    let Some(app) = state.read(cx).app(id).cloned() else {
        return;
    };
    let keywords = state.read(cx).keyword_count(id);
    window.open_dialog(cx, move |dialog, _, cx| {
        let state = state.clone();
        dialog
            .title(format!("Remove {}?", app.name))
            .w(px(440.))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!(
                "This deletes its {keywords} keyword{} and their rank history from this Mac. The app on the App Store isn't affected.",
                if keywords == 1 { "" } else { "s" }
            )))
            .footer(
                h_flex()
                    .w_full()
                    .justify_end()
                    .gap_2()
                    .child(Button::new("keep-app").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
                    .child(Button::new("confirm-remove-app").danger().label("Remove").on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        if let Err(error) = state.update(cx, |state, cx| state.remove_app(id, cx)) {
                            window.push_notification(Notification::error(error.to_string()), cx);
                        }
                    })),
            )
    });
}
