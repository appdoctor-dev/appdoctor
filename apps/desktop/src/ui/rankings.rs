//! Rankings: how the selected app's positions moved over time, overall and
//! per keyword, from the snapshots every rank check records.

use std::collections::BTreeMap;

use appdoctor_store::{KeywordId, KeywordRow, Snapshot};
use gpui_kit::component::chart::LineChart;
use gpui_kit::component::{ActiveTheme as _, IconName, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::AppState;
use crate::theme::meter;
use crate::ui::widgets::{self, card, flag};

/// One point on a chart: a check and a value.
#[derive(Clone)]
struct Point {
    label: SharedString,
    value: f64,
}

/// One "Check ranks" run: every keyword checked at the same moment.
struct Run {
    checked_at: i64,
    top10: usize,
    ranked: usize,
    average: Option<f64>,
}

pub struct RankingsView {
    state: Entity<AppState>,
    history: Vec<Snapshot>,
    selected: Option<KeywordId>,
    _subscriptions: Vec<Subscription>,
}

impl RankingsView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            _subscriptions: vec![cx.observe(&state, |this, _, cx| this.reload(cx))],
            state,
            history: Vec::new(),
            selected: None,
        };
        this.reload(cx);
        this
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        self.history = match state.selected_app {
            Some(app) => state
                .services
                .store
                .app_history(app, &state.country)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        let visible: Vec<KeywordId> = state.visible_keywords().map(|row| row.id).collect();
        if self.selected.is_none_or(|id| !visible.contains(&id)) {
            self.selected = visible.first().copied();
        }
        cx.notify();
    }

    fn runs(&self) -> Vec<Run> {
        let mut by_time: BTreeMap<i64, Vec<&Snapshot>> = BTreeMap::new();
        for snapshot in &self.history {
            by_time.entry(snapshot.checked_at).or_default().push(snapshot);
        }
        by_time
            .into_iter()
            .map(|(checked_at, checks)| {
                let ranks: Vec<u32> = checks.iter().filter_map(|s| s.rank).collect();
                Run {
                    checked_at,
                    top10: ranks.iter().filter(|rank| **rank <= 10).count(),
                    ranked: ranks.len(),
                    average: (!ranks.is_empty())
                        .then(|| ranks.iter().map(|r| *r as f64).sum::<f64>() / ranks.len() as f64),
                }
            })
            .collect()
    }

    fn overview(&self, cx: &App) -> impl IntoElement {
        let runs = self.runs();
        let theme = cx.theme();
        let latest = runs.last();
        let stat = |label: &str, value: String| {
            v_flex()
                .gap_0p5()
                .child(div().text_xl().font_weight(FontWeight::SEMIBOLD).child(value))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label.to_owned()),
                )
        };
        let points: Vec<Point> = runs
            .iter()
            .map(|run| Point {
                label: short_date(run.checked_at).into(),
                value: run.top10 as f64,
            })
            .collect();
        card(cx)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Keywords in the top 10"),
            )
            .child(
                h_flex()
                    .gap_8()
                    .child(stat(
                        "in the top 10",
                        latest.map_or("—".into(), |r| r.top10.to_string()),
                    ))
                    .child(stat(
                        "ranked at all",
                        latest.map_or("—".into(), |r| r.ranked.to_string()),
                    ))
                    .child(stat(
                        "average position",
                        latest
                            .and_then(|r| r.average)
                            .map_or("—".into(), |a| format!("#{a:.0}")),
                    ))
                    .child(stat("checks", runs.len().to_string())),
            )
            .child(div().h(px(180.)).child(if points.len() < 2 {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Run Check ranks again on another day to see the trend.")
                    .into_any_element()
            } else {
                LineChart::new(points)
                    .id("top10-trend")
                    .x(|p: &Point| p.label.clone())
                    .y(|p: &Point| p.value)
                    .stroke(theme.primary)
                    .linear()
                    .dot()
                    .tooltip_value(|_, v| format!("{v:.0} keywords").into())
                    .into_any_element()
            }))
    }

    fn keyword_list(&self, rows: &[KeywordRow], cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        v_flex()
            .id("ranking-keywords")
            .w(px(280.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.border)
            .p_2()
            .gap_0p5()
            .children(rows.iter().map(|row| {
                let id = row.id;
                let active = self.selected == Some(id);
                let (change, color) = widgets::change_label(row.change(), cx);
                h_flex()
                    .id(("ranking-keyword", id as u64))
                    .px_2()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .rounded(theme.radius)
                    .cursor_pointer()
                    .text_sm()
                    .map(|this| {
                        if active {
                            this.bg(theme.accent)
                        } else {
                            this.hover(|style| style.bg(theme.accent.opacity(0.6)))
                        }
                    })
                    .child(div().flex_1().min_w_0().truncate().child(row.term.clone()))
                    .child(div().text_xs().text_color(color).child(change))
                    .child(
                        div()
                            .w(px(44.))
                            .text_right()
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(row.rank().is_none(), |this| this.text_color(theme.muted_foreground))
                            .child(row.rank().map_or("—".into(), |rank| format!("#{rank}"))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected = Some(id);
                        cx.notify();
                    }))
            }))
    }

    fn keyword_detail(&self, row: &KeywordRow, cx: &App) -> impl IntoElement {
        let theme = cx.theme();
        let checks: Vec<&Snapshot> = self.history.iter().filter(|s| s.keyword_id == row.id).collect();
        // One series per source: the latest check's source.
        let source = checks.last().map(|s| s.source.clone()).unwrap_or_default();
        let series: Vec<&Snapshot> = checks.iter().copied().filter(|s| s.source == source).collect();
        let ranks: Vec<u32> = series.iter().filter_map(|s| s.rank).collect();
        let points: Vec<Point> = series
            .iter()
            .filter_map(|s| {
                s.rank.map(|rank| Point {
                    label: short_date(s.checked_at).into(),
                    // Inverted so #1 sits at the top.
                    value: -(rank as f64),
                })
            })
            .collect();
        let deepest = ranks.iter().max().copied().unwrap_or(10).max(10);
        let stat = |label: &str, value: String| {
            v_flex()
                .gap_0p5()
                .child(div().text_lg().font_weight(FontWeight::SEMIBOLD).child(value))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label.to_owned()),
                )
        };

        v_flex()
            .id("ranking-detail")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_5()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(row.term.clone()),
                    )
                    .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
                        "{} {} · {}",
                        flag(&row.country),
                        row.country.to_uppercase(),
                        source_label(&source)
                    ))),
            )
            .child(
                h_flex()
                    .gap_8()
                    .child(stat("now", row.rank().map_or("—".into(), |r| format!("#{r}"))))
                    .child(stat("best", ranks.iter().min().map_or("—".into(), |r| format!("#{r}"))))
                    .child(stat(
                        "worst",
                        ranks.iter().max().map_or("—".into(), |r| format!("#{r}")),
                    ))
                    .child(stat("checks", series.len().to_string())),
            )
            .child(
                card(cx).child(
                    div().h(px(220.)).child(match points.len() {
                        0 | 1 => div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child(if points.is_empty() {
                                "Not ranked in any check yet."
                            } else {
                                "One check so far. The trend appears after the next Check ranks."
                            })
                            .into_any_element(),
                        _ => LineChart::new(points)
                            .id(("keyword-trend", row.id as u64))
                            .x(|p: &Point| p.label.clone())
                            .y(|p: &Point| p.value)
                            .y_domain(-(deepest as f64), -1.)
                            .y_tick_format(|v| format!("#{:.0}", -v))
                            .stroke(theme.primary)
                            .linear()
                            .dot()
                            .tooltip_value(|_, v| format!("#{:.0}", -v).into())
                            .into_any_element(),
                    }),
                ),
            )
            .child(
                card(cx)
                    .gap_1()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).pb_1().child("Checks"))
                    .children(checks.iter().rev().take(30).map(|s| {
                        h_flex()
                            .gap_4()
                            .text_sm()
                            .child(
                                div()
                                    .w(px(130.))
                                    .text_color(theme.muted_foreground)
                                    .child(long_date(s.checked_at)),
                            )
                            .child(
                                div()
                                    .w(px(70.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(s.rank.map_or(format!("{}+", s.results_count), |r| format!("#{r}"))),
                            )
                            .child(
                                div()
                                    .w(px(110.))
                                    .text_color(s.difficulty.map_or(theme.muted_foreground, meter::difficulty))
                                    .child(s.difficulty.map_or("—".into(), |d| format!("difficulty {d:.0}"))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(source_label(&s.source)),
                            )
                    })),
            )
    }
}

impl Render for RankingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let Some(app) = state.selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        let rows: Vec<KeywordRow> = state.visible_keywords().cloned().collect();
        let country = state.country.clone();
        if self.history.is_empty() {
            return widgets::empty_state(
                gpui_kit::assets::IconName::ChartLine,
                "No rank history yet",
                "Run Check ranks on the Keywords page. Every check is kept, so the history builds up from there.",
                cx,
            )
            .into_any_element();
        }
        let selected = self
            .selected
            .and_then(|id| rows.iter().find(|row| row.id == id))
            .cloned();

        v_flex()
            .size_full()
            .child(
                h_flex()
                    .h(px(60.))
                    .flex_shrink_0()
                    .px_5()
                    .gap_3()
                    .items_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        v_flex()
                            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Rankings"))
                            .child(div().text_xs().text_color(cx.theme().muted_foreground).child(format!(
                                "{} in {} {}",
                                app.name,
                                flag(&country),
                                country.to_uppercase()
                            ))),
                    ),
            )
            .child(div().flex_shrink_0().p_5().pb_0().child(self.overview(cx)))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .mt_4()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(self.keyword_list(&rows, cx))
                    .when_some(selected, |this, row| this.child(self.keyword_detail(&row, cx))),
            )
            .into_any_element()
    }
}

fn local(timestamp: i64) -> Option<chrono::DateTime<chrono::Local>> {
    chrono::DateTime::from_timestamp(timestamp, 0).map(|utc| utc.with_timezone(&chrono::Local))
}

/// "Oct 29".
fn short_date(timestamp: i64) -> String {
    local(timestamp)
        .map(|t| t.format("%b %-d %H:%M").to_string())
        .unwrap_or_default()
}

/// "Oct 29, 21:09".
fn long_date(timestamp: i64) -> String {
    local(timestamp)
        .map(|t| t.format("%b %-d, %H:%M").to_string())
        .unwrap_or_default()
}

fn source_label(source: &str) -> &'static str {
    match source {
        appdoctor_appstore::NATIVE_SOURCE => "App Store search (iPhone)",
        appdoctor_appstore::ITUNES_SOURCE => "iTunes Search API",
        _ => "other source",
    }
}
