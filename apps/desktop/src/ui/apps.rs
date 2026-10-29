//! The Apps page: find an app by name (type-ahead App Store search) or by
//! link or ID, and manage tracked apps.

use std::time::Duration;

use appdoctor_appstore::{SearchHit, parse_app_ref};

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::Select;
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::runtime;
use crate::state::AppState;
use crate::ui::widgets::{self, CountrySelect, app_icon, card, compact, country_select, flag};

/// How many search results the dropdown shows.
const SUGGESTIONS: usize = 8;

pub struct AppsView {
    state: Entity<AppState>,
    link: Entity<InputState>,
    country: Entity<CountrySelect>,
    adding: bool,
    /// Type-ahead results for the current query, best match first.
    suggestions: Vec<SearchHit>,
    /// The query `suggestions` answer, so a stale response is dropped.
    suggested_for: String,
    searching: bool,
    _search: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl AppsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let link = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search the App Store by name, or paste a link or app ID")
                .clean_on_escape()
        });
        let country = cx.new(|cx| country_select("us", window, cx));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.subscribe_in(&link, window, |this, _, event, window, cx| match event {
                InputEvent::PressEnter { .. } => this.add(window, cx),
                InputEvent::Change => this.search(cx),
                _ => {}
            }),
        ];
        Self {
            state,
            link,
            country,
            adding: false,
            suggestions: Vec::new(),
            suggested_for: String::new(),
            searching: false,
            _search: None,
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

    /// Searches the App Store as the user types (debounced). Links and IDs
    /// skip the search: Enter adds them directly.
    fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.link.read(cx).value().trim().to_owned();
        if query.chars().count() < 2 || parse_app_ref(&query).is_ok() {
            self._search = None;
            self.suggestions.clear();
            self.suggested_for.clear();
            self.searching = false;
            cx.notify();
            return;
        }
        let services = self.state.read(cx).services.clone();
        let country = self.country(cx);
        self.searching = true;
        cx.notify();
        // Replacing the task cancels the previous keystroke's search.
        self._search = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_millis(250)).await;
            let term = query.clone();
            let results =
                runtime::spawn(async move { Ok(services.itunes.search(&term, &country, SUGGESTIONS).await?) }).await;
            this.update(cx, |this, cx| {
                this.searching = false;
                match results {
                    Ok(hits) => {
                        this.suggestions = hits;
                        this.suggested_for = query;
                    }
                    Err(error) => tracing::warn!(%error, "app search failed"),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    /// Enter: a link or ID is added as typed; a name adds the top match.
    fn add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.link.read(cx).value().trim().to_owned();
        if input.is_empty() {
            return;
        }
        if parse_app_ref(&input).is_ok() {
            self.add_ref(input, window, cx);
        } else if let Some(first) = self.suggestions.first() {
            self.add_ref(first.id.to_string(), window, cx);
        }
    }

    fn add_ref(&mut self, input: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.adding {
            return;
        }
        let country = self.country(cx);
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
                        this.suggestions.clear();
                        this.suggested_for.clear();
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

impl AppsView {
    /// The type-ahead list under the search field, drawn above the page.
    fn suggestion_list(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let query = self.link.read(cx).value().trim().to_owned();
        if self.suggested_for.is_empty() || query.is_empty() || parse_app_ref(&query).is_ok() {
            return None;
        }
        let state = self.state.read(cx);
        let icons = state.icons.clone();
        let tracked: Vec<u64> = state.apps.iter().map(|app| app.id).collect();
        let theme = cx.theme().clone();
        let rows: Vec<_> = self
            .suggestions
            .iter()
            .map(|hit| {
                let id = hit.id.0;
                let already = tracked.contains(&id);
                let icon = hit
                    .icon_url
                    .clone()
                    .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
                h_flex()
                    .id(("suggestion", id))
                    .px_3()
                    .py_2()
                    .gap_3()
                    .items_center()
                    .rounded(theme.radius)
                    .cursor_pointer()
                    .hover(|style| style.bg(theme.accent))
                    .child(app_icon(icon, px(32.), cx))
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(hit.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .truncate()
                                    .child(hit.developer.clone().unwrap_or_default()),
                            ),
                    )
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
                        "★ {:.1} · {}",
                        hit.average_rating,
                        compact(hit.rating_count)
                    )))
                    .child(if already {
                        widgets::pill("Tracked", theme.muted_foreground).into_any_element()
                    } else {
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.primary)
                            .into_any_element()
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if already {
                            this.state.update(cx, |state, cx| {
                                state.select_app(id, cx);
                                state.set_section(crate::state::Section::Keywords, cx);
                            });
                        } else {
                            this.add_ref(id.to_string(), window, cx);
                        }
                    }))
            })
            .collect();
        let empty = rows.is_empty();
        Some(deferred(
            v_flex()
                .id("app-suggestions")
                .absolute()
                .top(px(40.))
                .left_0()
                .right_0()
                .max_h(px(380.))
                .overflow_y_scroll()
                .p_1()
                .rounded(theme.radius_lg)
                .border_1()
                .border_color(theme.border)
                .bg(theme.popover)
                .shadow_lg()
                .occlude()
                .when(empty, |this| {
                    this.child(
                        div()
                            .p_3()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(format!("No apps found for “{query}”.")),
                    )
                })
                .children(rows),
        ))
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
                            .child(
                                div()
                                    .flex_1()
                                    .relative()
                                    .child(Input::new(&self.link).prefix(
                                        Icon::new(IconName::Search).small().text_color(muted),
                                    ).when(self.searching, |input| {
                                        input.suffix(Icon::new(IconName::LoaderCircle).small().text_color(muted))
                                    }))
                                    .when_some(self.suggestion_list(cx), |this, list| this.child(list)),
                            )
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
                            .child("Type an app's name to search, or paste an App Store link or ID. The country picks the storefront to search and look it up in."),
                    ),
            )
            .child(h_flex().flex_wrap().gap_4().children(cards))
    }
}
