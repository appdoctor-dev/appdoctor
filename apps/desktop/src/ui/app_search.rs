//! A search field that finds App Store apps by name as you type, with a
//! dropdown of matches. Links and IDs pass straight through on Enter.
//! Used to add apps and to add competitors.

use std::time::Duration;

use appdoctor_appstore::{SearchHit, parse_app_ref};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::runtime;
use crate::state::AppState;
use crate::ui::widgets::{self, app_icon, compact};

const SUGGESTIONS: usize = 8;

pub enum AppSearchEvent {
    /// A match was clicked (or Enter took the top match).
    Picked(SearchHit),
    /// Enter on a link or app ID.
    Reference(String),
}

pub struct AppSearch {
    state: Entity<AppState>,
    input: Entity<InputState>,
    country: String,
    /// Apps to mark as already added.
    taken: Vec<u64>,
    suggestions: Vec<SearchHit>,
    suggested_for: String,
    /// The dropdown shows while true; a click elsewhere closes it, so it
    /// never sits over the page blocking clicks.
    open: bool,
    searching: bool,
    _search: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<AppSearchEvent> for AppSearch {}

impl AppSearch {
    pub fn new(state: Entity<AppState>, placeholder: &str, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(placeholder.to_owned())
                .clean_on_escape()
        });
        let subscriptions = vec![cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::PressEnter { .. } => this.submit(cx),
            InputEvent::Change => this.search(cx),
            InputEvent::Focus => {
                this.open = !this.suggested_for.is_empty();
                cx.notify();
            }
            _ => {}
        })];
        Self {
            state,
            input,
            country: "us".into(),
            taken: Vec::new(),
            suggestions: Vec::new(),
            suggested_for: String::new(),
            open: false,
            searching: false,
            _search: None,
            _subscriptions: subscriptions,
        }
    }

    /// The storefront to search in.
    pub fn set_country(&mut self, country: &str, cx: &mut Context<Self>) {
        if self.country != country {
            self.country = country.to_owned();
            self.search(cx);
        }
    }

    pub fn set_taken(&mut self, taken: Vec<u64>) {
        self.taken = taken;
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_value("", window, cx));
        self.suggestions.clear();
        self.suggested_for.clear();
        self._search = None;
        self.searching = false;
        cx.notify();
    }

    fn search(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_owned();
        if query.chars().count() < 2 || parse_app_ref(&query).is_ok() {
            self._search = None;
            self.suggestions.clear();
            self.suggested_for.clear();
            self.searching = false;
            cx.notify();
            return;
        }
        let services = self.state.read(cx).services.clone();
        let country = self.country.clone();
        self.open = true;
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

    /// For an "Add" button beside the field: same as pressing Enter.
    pub fn submit_from_button(&mut self, cx: &mut Context<Self>) {
        self.submit(cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).value().trim().to_owned();
        if query.is_empty() {
            return;
        }
        if parse_app_ref(&query).is_ok() {
            cx.emit(AppSearchEvent::Reference(query));
        } else if let Some(first) = self.suggestions.first().cloned() {
            cx.emit(AppSearchEvent::Picked(first));
        }
    }

    fn dropdown(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let query = self.input.read(cx).value().trim().to_owned();
        if !self.open || self.suggested_for.is_empty() || query.is_empty() || parse_app_ref(&query).is_ok() {
            return None;
        }
        let icons = self.state.read(cx).icons.clone();
        let theme = cx.theme().clone();
        let rows: Vec<_> = self
            .suggestions
            .iter()
            .map(|hit| {
                let taken = self.taken.contains(&hit.id.0);
                let icon = hit
                    .icon_url
                    .clone()
                    .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
                let picked = hit.clone();
                h_flex()
                    .id(("app-search-hit", hit.id.0))
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
                    .child(if taken {
                        widgets::pill("Added", theme.muted_foreground).into_any_element()
                    } else {
                        Icon::new(IconName::Plus)
                            .small()
                            .text_color(theme.primary)
                            .into_any_element()
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open = false;
                        cx.emit(AppSearchEvent::Picked(picked.clone()));
                    }))
            })
            .collect();
        let empty = rows.is_empty();
        Some(deferred(
            v_flex()
                .id("app-search-results")
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

impl Render for AppSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        div()
            .relative()
            .w_full()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.open {
                    this.open = false;
                    cx.notify();
                }
            }))
            .child(
                Input::new(&self.input)
                    .prefix(Icon::new(IconName::Search).small().text_color(muted))
                    .when(self.searching, |input| {
                        input.suffix(Icon::new(IconName::LoaderCircle).small().text_color(muted))
                    }),
            )
            .when_some(self.dropdown(cx), |this, list| this.child(list))
    }
}
