//! Competitors: who ranks beside the selected app, how they rank on its
//! keywords, and which keywords they rank for that it doesn't track yet.
//!
//! Every rank check keeps the full result order, so a competitor's rank on
//! your keywords costs no extra requests. Finding *their* keywords does
//! search, through `discover`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use appdoctor_store::{Competitor, KeywordId, KeywordRow, Snapshot, TopApp};
use futures::StreamExt as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::discover::{self, Found};
use crate::rank_check::now;
use crate::runtime;
use crate::services::Services;
use crate::state::AppState;
use crate::theme::meter;
use crate::ui::app_search::{AppSearch, AppSearchEvent};
use crate::ui::widgets::{self, app_icon, card, flag};

/// Competitors shown side by side in the comparison.
const COMPARED: usize = 4;

struct Discovery {
    competitor: Competitor,
    country: String,
    done: usize,
    total: usize,
    running: bool,
    results: Vec<Found>,
    error: Option<String>,
}

pub struct CompetitorsView {
    state: Entity<AppState>,
    search: Entity<AppSearch>,
    competitors: Vec<Competitor>,
    latest: HashMap<KeywordId, Snapshot>,
    discovery: Option<Discovery>,
    _discovery: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl CompetitorsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| AppSearch::new(state.clone(), "Add a competitor by name, link or ID", window, cx));
        let subscriptions = vec![
            cx.observe(&state, |this, _, cx| this.reload(cx)),
            cx.subscribe_in(&search, window, |this, _, event, window, cx| match event {
                AppSearchEvent::Picked(hit) => {
                    this.add(
                        TopApp {
                            id: hit.id.0,
                            name: hit.title.clone(),
                            icon_url: hit.icon_url.clone(),
                        },
                        window,
                        cx,
                    );
                }
                AppSearchEvent::Reference(text) => this.add_by_reference(text.clone(), window, cx),
            }),
        ];
        let mut this = Self {
            state,
            search,
            competitors: Vec::new(),
            latest: HashMap::new(),
            discovery: None,
            _discovery: None,
            _subscriptions: subscriptions,
        };
        this.reload(cx);
        this
    }

    fn services(&self, cx: &App) -> Arc<Services> {
        self.state.read(cx).services.clone()
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let (competitors, latest) = match state.selected_app {
            Some(app) => (
                state.services.store.competitors(app).unwrap_or_default(),
                state
                    .services
                    .store
                    .latest_rankings(app, &state.country)
                    .unwrap_or_default(),
            ),
            None => (Vec::new(), HashMap::new()),
        };
        let country = state.country.clone();
        let mut taken: Vec<u64> = competitors.iter().map(|c| c.competitor_id).collect();
        taken.extend(state.selected_app);
        self.competitors = competitors;
        self.latest = latest;
        self.search.update(cx, |search, cx| {
            search.set_taken(taken);
            search.set_country(&country, cx);
        });
        if let Some(discovery) = &self.discovery
            && (discovery.country != country
                || !self
                    .competitors
                    .iter()
                    .any(|c| c.competitor_id == discovery.competitor.competitor_id))
            && !discovery.running
        {
            self.discovery = None;
        }
        cx.notify();
    }

    fn add(&mut self, app: TopApp, window: &mut Window, cx: &mut Context<Self>) {
        let Some(me) = self.state.read(cx).selected_app else {
            return;
        };
        if app.id == me {
            window.push_notification(Notification::warning("That's the app you're tracking."), cx);
            return;
        }
        let competitor = Competitor {
            app_id: me,
            competitor_id: app.id,
            name: app.name,
            icon_url: app.icon_url,
            added_at: now(),
        };
        match self.services(cx).store.add_competitor(&competitor) {
            Ok(()) => {
                self.search.update(cx, |search, cx| search.clear(window, cx));
                self.reload(cx);
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
    }

    fn add_by_reference(&mut self, text: String, window: &mut Window, cx: &mut Context<Self>) {
        let services = self.services(cx);
        let country = self.state.read(cx).country.clone();
        let lookup = runtime::spawn(async move {
            let app_ref = appdoctor_appstore::parse_app_ref(&text)?;
            Ok(services.itunes.lookup(app_ref.id, &country).await?)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = lookup.await;
            this.update_in(cx, |this, window, cx| match result {
                Ok(details) => this.add(
                    TopApp {
                        id: details.id.0,
                        name: details.name,
                        icon_url: details.icon_url,
                    },
                    window,
                    cx,
                ),
                Err(error) => window.push_notification(Notification::error(format!("{error:#}")), cx),
            })
            .ok();
        })
        .detach();
    }

    fn remove(&mut self, competitor_id: u64, cx: &mut Context<Self>) {
        let services = self.services(cx);
        if let Some(me) = self.state.read(cx).selected_app {
            let _ = services.store.delete_competitor(me, competitor_id);
        }
        if self
            .discovery
            .as_ref()
            .is_some_and(|d| d.competitor.competitor_id == competitor_id)
        {
            self.discovery = None;
            self._discovery = None;
        }
        self.reload(cx);
    }

    fn discover(&mut self, competitor: Competitor, cx: &mut Context<Self>) {
        let Some(me) = self.state.read(cx).selected_app else {
            return;
        };
        let country = self.state.read(cx).country.clone();
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let job = runtime::spawn(discover::run(
            self.services(cx),
            competitor.competitor_id,
            me,
            country.clone(),
            sender,
        ));
        self.discovery = Some(Discovery {
            competitor,
            country,
            done: 0,
            total: 0,
            running: true,
            results: Vec::new(),
            error: None,
        });
        cx.notify();
        self._discovery = Some(cx.spawn(async move |this, cx| {
            while let Some((done, total)) = receiver.next().await {
                let updated = this.update(cx, |this, cx| {
                    if let Some(discovery) = &mut this.discovery {
                        discovery.done = done;
                        discovery.total = total;
                    }
                    cx.notify();
                });
                if updated.is_err() {
                    return;
                }
            }
            let result = job.await;
            this.update(cx, |this, cx| {
                if let Some(discovery) = &mut this.discovery {
                    discovery.running = false;
                    match result {
                        Ok(found) => discovery.results = found,
                        Err(error) => discovery.error = Some(format!("{error:#}")),
                    }
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn track(&mut self, term: String, window: &mut Window, cx: &mut Context<Self>) {
        match self.state.update(cx, |state, cx| state.add_keywords(&term, cx)) {
            Ok(_) => window.push_notification(Notification::success(format!("Tracking “{term}”")), cx),
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
    }

    /// Apps in the top 10 of the most of your keywords, not yet tracked.
    fn suggested(&self, rows: &[KeywordRow], me: u64) -> Vec<(TopApp, usize)> {
        let tracked: HashSet<u64> = self.competitors.iter().map(|c| c.competitor_id).collect();
        let mut counts: HashMap<u64, (TopApp, usize)> = HashMap::new();
        for row in rows {
            for app in &row.top_apps {
                if app.id != me && !tracked.contains(&app.id) {
                    counts.entry(app.id).or_insert_with(|| (app.clone(), 0)).1 += 1;
                }
            }
        }
        let mut list: Vec<_> = counts.into_values().filter(|(_, n)| *n >= 2).collect();
        list.sort_by(|(a, x), (b, y)| y.cmp(x).then_with(|| a.name.cmp(&b.name)));
        list.truncate(6);
        list
    }

    /// A competitor's rank for a keyword from the latest check: the full
    /// order when stored, else the kept top 10.
    fn rank_of(&self, row: &KeywordRow, app: u64) -> Option<u32> {
        match self.latest.get(&row.id) {
            Some(snapshot) if !snapshot.ranked_ids.is_empty() => snapshot.rank_of(app),
            _ => row
                .top_apps
                .iter()
                .position(|top| top.id == app)
                .map(|ix| ix as u32 + 1),
        }
    }

    fn icon(&self, url: &Option<String>, size: Pixels, cx: &mut Context<Self>) -> AnyElement {
        let icons = self.state.read(cx).icons.clone();
        let image = url
            .clone()
            .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
        app_icon(image, size, cx)
    }

    fn competitor_cards(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let discovering = self.discovery.as_ref().is_some_and(|d| d.running);
        let cards: Vec<_> = self
            .competitors
            .clone()
            .into_iter()
            .map(|competitor| {
                let id = competitor.competitor_id;
                let active = self
                    .discovery
                    .as_ref()
                    .is_some_and(|d| d.competitor.competitor_id == id);
                let icon = self.icon(&competitor.icon_url, px(36.), cx);
                let target = competitor.clone();
                card(cx)
                    .id(("competitor", id))
                    .w(px(260.))
                    .gap_2()
                    .when(active, |this| this.border_color(cx.theme().primary))
                    .child(
                        h_flex().gap_3().items_center().child(icon).child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child(competitor.name.clone()),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(("find-keywords", id))
                                    .small()
                                    .primary()
                                    .icon(IconName::Search)
                                    .label("Find their keywords")
                                    .disabled(discovering)
                                    .on_click(cx.listener(move |this, _, _, cx| this.discover(target.clone(), cx))),
                            )
                            .child(
                                Button::new(("remove-competitor", id))
                                    .small()
                                    .ghost()
                                    .label("Remove")
                                    .on_click(cx.listener(move |this, _, _, cx| this.remove(id, cx))),
                            ),
                    )
            })
            .collect();
        h_flex().flex_wrap().gap_3().children(cards)
    }

    fn suggestions_card(&self, rows: &[KeywordRow], me: u64, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let suggested = self.suggested(rows, me);
        if suggested.is_empty() {
            return None;
        }
        let total = rows.len();
        let items: Vec<_> = suggested
            .into_iter()
            .map(|(app, count)| {
                let icon = self.icon(&app.icon_url, px(28.), cx);
                let name = app.name.clone();
                let id = app.id;
                h_flex()
                    .id(("suggested-competitor", id))
                    .w(px(300.))
                    .px_2()
                    .py_1p5()
                    .gap_2()
                    .items_center()
                    .rounded(cx.theme().radius)
                    .hover(|style| style.bg(cx.theme().accent.opacity(0.6)))
                    .child(icon)
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(div().text_sm().font_weight(FontWeight::MEDIUM).truncate().child(name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!("top 10 for {count} of your {total} keywords")),
                            ),
                    )
                    .child(
                        Button::new(("track-competitor", id))
                            .small()
                            .outline()
                            .label("Track")
                            .on_click(cx.listener(move |this, _, window, cx| this.add(app.clone(), window, cx))),
                    )
            })
            .collect();
        Some(
            card(cx)
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Apps ranking beside you"),
                )
                .child(h_flex().flex_wrap().gap_1().children(items)),
        )
    }

    fn comparison(&self, rows: &[KeywordRow], my_icon: Option<String>, cx: &mut Context<Self>) -> impl IntoElement {
        let compared: Vec<Competitor> = self.competitors.iter().take(COMPARED).cloned().collect();
        let theme = cx.theme().clone();
        let header_cell = |icon: AnyElement, label: String| {
            h_flex()
                .w(px(130.))
                .gap_2()
                .items_center()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(icon)
                .child(div().min_w_0().truncate().child(label))
        };
        let mut header = h_flex()
            .px_3()
            .py_2()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .child(
                div()
                    .flex_1()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Keyword"),
            )
            .child(header_cell(self.icon(&my_icon, px(18.), cx), "You".into()));
        for competitor in &compared {
            let icon = self.icon(&competitor.icon_url, px(18.), cx);
            header = header.child(header_cell(icon, competitor.name.clone()));
        }
        let missing_full = rows
            .iter()
            .any(|row| self.latest.get(&row.id).is_some_and(|s| s.ranked_ids.is_empty()));

        let body = rows.iter().map(|row| {
            let mut ranks = vec![row.rank()];
            ranks.extend(compared.iter().map(|c| self.rank_of(row, c.competitor_id)));
            let best = ranks.iter().flatten().min().copied();
            let mut line = h_flex()
                .px_3()
                .py_1p5()
                .gap_2()
                .text_sm()
                .border_b_1()
                .border_color(theme.border.opacity(0.5))
                .child(div().flex_1().min_w_0().truncate().child(row.term.clone()));
            for rank in ranks {
                let leading = rank.is_some() && rank == best;
                line = line.child(
                    div()
                        .w(px(130.))
                        .when(leading, |this| {
                            this.text_color(meter::up()).font_weight(FontWeight::SEMIBOLD)
                        })
                        .when(rank.is_none(), |this| this.text_color(theme.muted_foreground))
                        .child(rank.map_or("—".into(), |r| format!("#{r}"))),
                );
            }
            line
        });

        card(cx)
            .gap_0()
            .p_0()
            .child(
                h_flex()
                    .px_3()
                    .pt_3()
                    .pb_2()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Your keywords, side by side"),
                    )
                    .when(missing_full, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child("· ranks past the top 10 appear after the next Check ranks"),
                        )
                    }),
            )
            .child(header)
            .children(body)
    }

    fn discovery_card(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let discovery = self.discovery.as_ref()?;
        let theme = cx.theme().clone();
        let tracked: HashSet<String> = self
            .state
            .read(cx)
            .visible_keywords()
            .map(|row| row.term.clone())
            .collect();
        let percent = if discovery.total == 0 {
            0.
        } else {
            discovery.done as f32 * 100. / discovery.total as f32
        };
        let rows: Vec<_> = discovery
            .results
            .iter()
            .map(|found| {
                let is_tracked = tracked.contains(&found.term);
                let term = found.term.clone();
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_2()
                    .text_sm()
                    .items_center()
                    .border_b_1()
                    .border_color(theme.border.opacity(0.5))
                    .child(div().flex_1().min_w_0().truncate().child(found.term.clone()))
                    .child(
                        div()
                            .w(px(110.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!("#{}", found.competitor_rank)),
                    )
                    .child(
                        div()
                            .w(px(110.))
                            .when(found.your_rank.is_none(), |this| {
                                this.text_color(theme.muted_foreground)
                            })
                            .child(found.your_rank.map_or("—".into(), |r| format!("#{r}"))),
                    )
                    .child(div().w(px(130.)).child(widgets::meter(
                        found.difficulty,
                        meter::difficulty(found.difficulty),
                        cx,
                    )))
                    .child(div().w(px(80.)).child(if is_tracked {
                        widgets::pill("Tracked", theme.muted_foreground).into_any_element()
                    } else {
                        Button::new(SharedString::from(format!("track-{}", found.term)))
                            .small()
                            .outline()
                            .label("Track")
                            .on_click(cx.listener(move |this, _, window, cx| this.track(term.clone(), window, cx)))
                            .into_any_element()
                    }))
            })
            .collect();
        let column = |label: &str, width: f32| {
            div()
                .w(px(width))
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(label.to_owned())
        };
        Some(
            card(cx)
                .gap_0()
                .p_0()
                .child(
                    v_flex()
                        .px_3()
                        .pt_3()
                        .pb_2()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("Keywords {} ranks for", discovery.competitor.name)),
                        )
                        .child(div().text_xs().text_color(theme.muted_foreground).child(format!(
                            "Searched {} {} with phrases from its title and description and App Store suggestions",
                            flag(&discovery.country),
                            discovery.country.to_uppercase()
                        ))),
                )
                .when(discovery.running, |this| {
                    this.child(
                        h_flex()
                            .px_3()
                            .pb_3()
                            .gap_3()
                            .items_center()
                            .child(
                                div()
                                    .w(px(220.))
                                    .child(Progress::new("discover-progress").value(percent)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("Checking {} of {} searches…", discovery.done, discovery.total)),
                            ),
                    )
                })
                .when_some(discovery.error.clone(), |this, error| {
                    this.child(div().px_3().pb_3().text_sm().text_color(theme.danger).child(error))
                })
                .when(!discovery.running && discovery.error.is_none(), |this| {
                    this.when(rows.is_empty(), |this| {
                        this.child(
                            div()
                                .px_3()
                                .pb_3()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("It didn't rank for any of the searched phrases."),
                        )
                    })
                    .when(!rows.is_empty(), |this| {
                        this.child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .gap_2()
                                .border_b_1()
                                .border_color(theme.border)
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Keyword"),
                                )
                                .child(column("Their rank", 110.))
                                .child(column("Your rank", 110.))
                                .child(column("Difficulty", 130.))
                                .child(column("", 80.)),
                        )
                        .children(rows)
                    })
                }),
        )
    }
}

impl Render for CompetitorsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let Some(app) = state.selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        let rows: Vec<KeywordRow> = state.visible_keywords().cloned().collect();
        let country = state.country.clone();
        let muted = cx.theme().muted_foreground;

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
                            .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("Competitors"))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("{} in {} {}", app.name, flag(&country), country.to_uppercase())),
                            ),
                    )
                    .child(div().flex_1())
                    .child(div().w(px(380.)).child(self.search.clone())),
            )
            .child(
                v_flex()
                    .id("competitors-page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_5()
                    .gap_4()
                    .when(self.competitors.is_empty(), |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(muted)
                                .child("Track the apps you compete with. Their ranks on your keywords come from your own rank checks, and “Find their keywords” searches for what they rank for."),
                        )
                    })
                    .child(self.competitor_cards(cx))
                    .when_some(self.discovery_card(cx), |this, card| this.child(card))
                    .when_some(self.suggestions_card(&rows, app.id, cx), |this, card| this.child(card))
                    .when(!self.competitors.is_empty() && !rows.is_empty(), |this| {
                        this.child(self.comparison(&rows, app.icon_url.clone(), cx))
                    }),
            )
            .into_any_element()
    }
}
