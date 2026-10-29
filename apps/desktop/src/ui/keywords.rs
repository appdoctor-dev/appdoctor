//! The Keywords page: the selected app's keywords in one storefront, with
//! the toolbar for adding keywords and checking ranks.

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::select::{Select, SelectEvent};
use gpui_kit::component::table::{DataTable, TableState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::rank_check::now;
use crate::state::{AppState, Section};
use crate::ui::keyword_table::KeywordTable;
use crate::ui::widgets::{self, CountrySelect, app_icon, country_select, flag};

pub struct KeywordsView {
    state: Entity<AppState>,
    table: Entity<TableState<KeywordTable>>,
    country: Entity<CountrySelect>,
    search: Entity<InputState>,
    new_keywords: Entity<InputState>,
    adding: bool,
    shown_country: String,
    /// The picker follows the storefront when the selected app changes; the
    /// picker needs a window to update, so the change is applied on render.
    pending_country: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl KeywordsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let icons = state.read(cx).icons.clone();
        let current_country = state.read(cx).country.clone();
        let table = cx.new(|cx| {
            TableState::new(KeywordTable::new(state.clone(), icons.clone()), window, cx)
                .row_header(false)
                .col_movable(false)
                .col_selectable(false)
        });
        let country = cx.new(|cx| country_select(&current_country, window, cx));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Filter keywords"));
        let new_keywords = cx.new(|cx| {
            InputState::new(window, cx).placeholder("habit tracker, streaks, daily routine — separate with commas")
        });

        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.sync(cx)),
            cx.observe(&icons, |this, _, cx| this.table.update(cx, |_, cx| cx.notify())),
            cx.subscribe_in(&search, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.sync(cx);
                }
            }),
            cx.subscribe_in(&new_keywords, window, |this, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.add_keywords(window, cx);
                }
            }),
            cx.subscribe_in(&country, window, |this, _, event, _, cx| {
                let SelectEvent::Confirm(Some(code)) = event else {
                    return;
                };
                let code = *code;
                this.state.update(cx, |state, cx| state.set_country(code, cx));
            }),
        ];

        let mut this = Self {
            state,
            table,
            country,
            search,
            new_keywords,
            adding: false,
            shown_country: current_country,
            pending_country: None,
            _subscriptions: subscriptions,
        };
        this.sync(cx);
        this
    }

    /// Pushes the current keywords, filtered by storefront and search, into the table.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let query = self.search.read(cx).value().trim().to_lowercase();
        let rows: Vec<_> = state
            .visible_keywords()
            .filter(|row| query.is_empty() || row.term.contains(&query))
            .cloned()
            .collect();
        let country = state.country.clone();
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows);
            cx.notify();
        });
        if country != self.shown_country {
            self.shown_country = country.clone();
            self.pending_country = Some(country);
        }
        cx.notify();
    }

    fn add_keywords(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.new_keywords.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        match self.state.update(cx, |state, cx| state.add_keywords(&text, cx)) {
            Ok(added) => {
                self.new_keywords
                    .update(cx, |input, cx| input.set_value("", window, cx));
                let message = match added {
                    0 => "Those keywords are already tracked".to_owned(),
                    1 => "Added 1 keyword".to_owned(),
                    n => format!("Added {n} keywords"),
                };
                window.push_notification(Notification::success(message), cx);
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
    }

    fn check_ranks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let task = self.state.update(cx, |state, cx| state.check_ranks(cx));
        cx.spawn_in(window, async move |_, cx| {
            let result = task.await;
            cx.update(|window, cx| {
                let note = match result {
                    Ok(outcome) if outcome.failed == 0 => {
                        let mut message = format!("Checked {} keywords", outcome.checked);
                        if let Some(Err(error)) = &outcome.popularity {
                            message.push_str(&format!(" · popularity unavailable: {error}"));
                        }
                        Notification::success(message)
                    }
                    Ok(outcome) => Notification::warning(format!(
                        "Checked {}, {} failed: {}",
                        outcome.checked,
                        outcome.failed,
                        outcome.first_error.unwrap_or_default()
                    )),
                    Err(error) => Notification::error(error.to_string()),
                };
                window.push_notification(note, cx);
            })
            .ok();
        })
        .detach();
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let app = state.selected().cloned();
        let checking = state.check.is_some();
        let count = state.visible_keywords().count();
        let last_check = state
            .visible_keywords()
            .filter_map(|row| row.latest.map(|c| c.checked_at))
            .max();
        let country = state.country.clone();
        let icons = state.icons.clone();
        let icon = app
            .as_ref()
            .and_then(|app| app.icon_url.clone())
            .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
        let muted = cx.theme().muted_foreground;

        h_flex()
            .h(px(60.))
            .flex_shrink_0()
            .px_5()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(app_icon(icon, px(36.), cx))
            .child(
                v_flex()
                    .min_w_0()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .truncate()
                            .child(app.as_ref().map(|app| app.name.clone()).unwrap_or_default()),
                    )
                    .child(div().text_xs().text_color(muted).child(match last_check {
                        Some(at) => format!(
                            "{count} keywords in {} {} · checked {}",
                            flag(&country),
                            country.to_uppercase(),
                            widgets::relative_time(at, now())
                        ),
                        None => format!("{count} keywords in {} {}", flag(&country), country.to_uppercase()),
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .w(px(200.))
                    .child(Select::new(&self.country).small().menu_width(px(260.))),
            )
            .child(
                div().w(px(190.)).child(
                    Input::new(&self.search)
                        .small()
                        .cleanable(true)
                        .prefix(Icon::new(IconName::Search).small().text_color(muted)),
                ),
            )
            .child(
                Button::new("check-ranks")
                    .small()
                    .outline()
                    .icon(IconName::RefreshCw)
                    .label(if checking { "Checking…" } else { "Check ranks" })
                    .loading(checking)
                    .disabled(checking || count == 0)
                    .on_click(cx.listener(|this, _, window, cx| this.check_ranks(window, cx))),
            )
            .child(
                Button::new("add-keywords")
                    .small()
                    .primary()
                    .icon(IconName::Plus)
                    .label("Add keywords")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.adding = !this.adding;
                        if this.adding {
                            this.new_keywords.update(cx, |input, cx| input.focus(window, cx));
                        }
                        cx.notify();
                    })),
            )
    }

    fn progress_strip(&self, cx: &App) -> Option<impl IntoElement> {
        let progress = self.state.read(cx).check?;
        let percent = if progress.total == 0 {
            0.
        } else {
            progress.done as f32 * 100. / progress.total as f32
        };
        Some(
            h_flex()
                .px_5()
                .py_2()
                .gap_3()
                .items_center()
                .border_b_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted.opacity(0.4))
                .child(div().w(px(220.)).child(Progress::new("rank-progress").value(percent)))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Checking {} of {} keywords…", progress.done, progress.total)),
                ),
        )
    }

    fn add_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let country = self.state.read(cx).country.clone();
        h_flex()
            .px_5()
            .py_3()
            .gap_2()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted.opacity(0.4))
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!(
                "{} {}",
                flag(&country),
                country.to_uppercase()
            )))
            .child(div().flex_1().child(Input::new(&self.new_keywords).small()))
            .child(
                Button::new("add-keywords-confirm")
                    .small()
                    .primary()
                    .label("Add")
                    .on_click(cx.listener(|this, _, window, cx| this.add_keywords(window, cx))),
            )
            .child(
                Button::new("add-keywords-cancel")
                    .small()
                    .ghost()
                    .label("Done")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.adding = false;
                        cx.notify();
                    })),
            )
    }

    fn summary(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let has_popularity = state.visible_keywords().any(|row| row.popularity.is_some());
        let (mut top10, mut top50, mut ranked, mut unranked) = (0, 0, 0, 0);
        for row in state.visible_keywords() {
            match row.rank() {
                Some(rank) if rank <= 10 => top10 += 1,
                Some(rank) if rank <= 50 => top50 += 1,
                Some(_) => ranked += 1,
                None if row.latest.is_some() => unranked += 1,
                None => {}
            }
        }
        let muted = cx.theme().muted_foreground;
        let stat = |label: &str, value: usize| {
            h_flex()
                .gap_1()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(value.to_string()))
                .child(div().text_color(muted).child(label.to_owned()))
        };
        h_flex()
            .h(px(32.))
            .flex_shrink_0()
            .px_5()
            .gap_5()
            .items_center()
            .text_xs()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(stat("top 10", top10))
            .child(stat("top 11–50", top50))
            .child(stat("ranked 51+", ranked))
            .child(stat("not ranked", unranked))
            .child(div().flex_1())
            .map(|this| {
                if has_popularity {
                    this.child(
                        div()
                            .text_color(muted)
                            .child("Ranks from App Store search (iPhone), checked from this Mac"),
                    )
                } else {
                    this.child(
                        div()
                            .id("connect-apple-ads")
                            .cursor_pointer()
                            .text_color(cx.theme().primary)
                            .child("Connect Apple Ads in Settings to see search popularity")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state
                                    .update(cx, |state, cx| state.set_section(Section::Settings, cx))
                            })),
                    )
                }
            })
    }
}

impl Render for KeywordsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(country) = self.pending_country.take() {
            self.country.update(cx, |select, cx| {
                select.set_selected_value(&country_code(&country), window, cx)
            });
        }
        let state = self.state.read(cx);
        if state.selected().is_none() {
            return widgets::empty_state(
                IconName::Inbox,
                "No app selected",
                "Add an app by its App Store link or ID, then track the keywords you want it to rank for.",
                cx,
            )
            .child(
                Button::new("go-add-app")
                    .primary()
                    .icon(IconName::Plus)
                    .label("Add an app")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.update(cx, |state, cx| state.set_section(Section::Apps, cx))
                    })),
            )
            .into_any_element();
        }
        let empty = state.visible_keywords().next().is_none() && self.search.read(cx).value().is_empty();
        let country = state.country.clone();

        v_flex()
            .size_full()
            .child(self.toolbar(cx))
            .when_some(self.progress_strip(cx), |this, strip| this.child(strip))
            .when(self.adding || empty, |this| this.child(self.add_bar(cx)))
            .child(div().flex_1().min_h_0().map(|this| {
                if empty {
                    this.child(widgets::empty_state(
                        IconName::Search,
                        &format!("No keywords in {} {} yet", flag(&country), country.to_uppercase()),
                        "Type the searches you want this app to rank for above. Then run Check ranks to see its position, the difficulty and the apps ranking ahead of it.",
                        cx,
                    ))
                } else {
                    this.child(DataTable::new(&self.table).stripe(false).bordered(false))
                }
            }))
            .child(self.summary(cx))
            .into_any_element()
    }
}

fn country_code(code: &str) -> &'static str {
    appdoctor_appstore::country(code)
        .map(|country| country.code)
        .unwrap_or("us")
}
