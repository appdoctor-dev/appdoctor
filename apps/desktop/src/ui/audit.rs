//! ASO audit: a score out of 100 for the selected app, in five areas, and
//! what to fix first, each finding with a way to fix it.
//!
//! It reads the live product page (the public App Store lookup, saved so
//! AI agents can audit too), the listing Localization last pulled, the
//! keywords and their ranks, competitors and the app's designs, then runs
//! [`appdoctor_audit::audit`]. *Fix with AI* hands the findings to the
//! AI chat beside Localization.

use std::collections::{HashMap, HashSet};

use appdoctor_audit::{Designs, Finding, Fix, Inputs, Level, Live, Report, audit, live_key};
use appdoctor_screenshots::Target;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::rank_check::now;
use crate::runtime;
use crate::state::{AppState, Section};
use crate::theme::meter;
use crate::ui::{controls, widgets};

/// One app's audit.
#[derive(Clone)]
struct Checked {
    report: Report,
    at: i64,
    /// Why the live page couldn't be read, if it couldn't.
    offline: Option<String>,
}

pub struct AuditView {
    state: Entity<AppState>,
    /// Each app's latest audit, so one app never shows another's.
    checked: HashMap<u64, Checked>,
    /// Apps being audited now.
    running: HashSet<u64>,
    /// Apps audited since Audit was last opened; the rest check again.
    fresh: HashSet<u64>,
    /// Whether Audit was showing at the last change, to notice it opening.
    showing: bool,
    _subscriptions: Vec<Subscription>,
}

impl AuditView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&state, |this, state, cx| {
            // Opening Audit checks again: keywords, the listing or the
            // App Store page may have changed since.
            let showing = state.read(cx).section == Section::Audit;
            if showing && !this.showing {
                this.fresh.clear();
            }
            this.showing = showing;
            cx.notify();
        })];
        Self {
            state,
            checked: HashMap::new(),
            running: HashSet::new(),
            fresh: HashSet::new(),
            showing: false,
            _subscriptions: subscriptions,
        }
    }

    fn run(&mut self, cx: &mut Context<Self>) {
        let state = self.state.read(cx);
        let Some(app) = state.selected().cloned() else {
            return;
        };
        let services = state.services.clone();
        let countries = state.countries(app.id);
        let app_id = app.id;
        if !self.running.insert(app_id) {
            return;
        }
        self.fresh.insert(app_id);
        cx.notify();
        let work = runtime::spawn(async move {
            let store = &services.store;
            let key = live_key(app.id);
            // The live page, fresh when the App Store answers, else as last seen.
            let (live, offline) = match services.itunes.lookup(appdoctor_appstore::AppId(app.id), &app.country).await {
                Ok(details) => {
                    let live = Live {
                        country: app.country.clone(),
                        name: details.name,
                        description: details.description,
                        release_notes: details.release_notes,
                        released: details.released,
                        screenshots: details.screenshots,
                        ipad_screenshots: details.ipad_screenshots,
                        languages: details.languages,
                        content_rating: details.content_rating,
                        rating_count: details.rating_count,
                        average_rating: details.average_rating,
                        fetched_at: now(),
                    };
                    if let Ok(json) = serde_json::to_string(&live) {
                        let _ = store.set_setting(&key, &json);
                    }
                    (Some(live), None)
                }
                Err(error) => {
                    let saved = store
                        .setting(&key)
                        .ok()
                        .flatten()
                        .and_then(|json| serde_json::from_str::<Live>(&json).ok());
                    (saved, Some(format!("{error:#}")))
                }
            };
            let listings = store.listings(app.id)?;
            let keywords = store.keywords(app.id)?;
            let competitors = store.competitors(app.id)?.len();
            let designs = Designs::read(&services.data_dir.join("screenshots").join(app.id.to_string()));
            let report = audit(&Inputs {
                app: &app,
                live: live.as_ref(),
                listings: &listings,
                keywords: &keywords,
                competitors,
                countries: &countries,
                designs,
                now: now(),
            });
            Ok((report, offline))
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.running.remove(&app_id);
                match result {
                    // Kept for its own app, whichever is showing now.
                    Ok((report, offline)) => {
                        this.checked.insert(app_id, Checked { report, at: now(), offline });
                    }
                    Err(error) => {
                        if let Some(checked) = this.checked.get_mut(&app_id) {
                            checked.offline = Some(format!("{error:#}"));
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Opens the AI beside Localization with the audit to work through.
    fn fix_with_ai(&mut self, cx: &mut Context<Self>) {
        let prompt = "Run aso_audit for this app, then fix what you can in the listing with suggest_listing, \
                      most important first. Tell me briefly what you changed and what I need to do myself.";
        self.state.update(cx, |state, cx| {
            state.ai_request = Some(prompt.to_owned());
            state.set_section(Section::Localization, cx);
        });
    }

    fn open(&mut self, fix: Fix, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| match fix {
            Fix::Localization => state.set_section(Section::Localization, cx),
            Fix::Keywords => state.set_section(Section::Keywords, cx),
            Fix::Competitors => state.set_section(Section::Competitors, cx),
            Fix::Settings => state.set_section(Section::Settings, cx),
            Fix::Screenshots | Fix::Video => state.open_screenshots(Target::Iphone, cx),
            Fix::Header => state.open_screenshots(Target::Header, cx),
            Fix::None => {}
        });
    }

    fn level_icon(level: Level, cx: &App) -> Icon {
        let theme = cx.theme();
        match level {
            Level::Critical => Icon::new(IconName::CircleX).text_color(theme.danger),
            Level::Warning => Icon::new(IconName::TriangleAlert).text_color(theme.warning),
            Level::Tip => Icon::new(IconName::Info).text_color(theme.primary),
            Level::Good => Icon::new(IconName::CircleCheck).text_color(meter::up()),
        }
    }

    fn score_color(score: u8, cx: &App) -> Hsla {
        let theme = cx.theme();
        match score {
            90.. => meter::up(),
            75..90 => theme.primary,
            55..75 => theme.warning,
            _ => theme.danger,
        }
    }

    fn finding_row(&self, index: usize, finding: &Finding, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let fix = finding.fix;
        let action = match fix {
            Fix::Localization => Some("Open Localization"),
            Fix::Keywords => Some("Open Keywords"),
            Fix::Screenshots => Some("Open Screenshots"),
            Fix::Video => Some("Make a video"),
            Fix::Header => Some("Design a header"),
            Fix::Competitors => Some("Add competitors"),
            Fix::Settings => Some("Open Settings"),
            Fix::None => None,
        };
        h_flex()
            .px(px(14.))
            .py(px(11.))
            .gap(px(12.))
            .items_start()
            .when(index > 0, |this| this.border_t_1().border_color(theme.foreground.opacity(0.06)))
            .child(div().mt(px(1.)).child(Self::level_icon(finding.level, cx).size(px(16.))))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_baseline()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(finding.title.clone()))
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(theme.muted_foreground)
                                    .child(finding.area.label()),
                            ),
                    )
                    .when(!finding.detail.is_empty(), |this| {
                        this.child(
                            div()
                                .text_size(px(12.))
                                .line_height(rems(1.35))
                                .text_color(theme.muted_foreground)
                                .child(finding.detail.clone()),
                        )
                    }),
            )
            .when_some(action, |this, label| {
                this.child(
                    Button::new(("audit-fix", index))
                        .small()
                        .ghost()
                        .label(label)
                        .on_click(cx.listener(move |this, _, _, cx| this.open(fix, cx))),
                )
            })
    }

    fn summary(&self, report: &Report, cx: &App) -> Div {
        let theme = cx.theme();
        let color = Self::score_color(report.score, cx);
        let to_fix = report.to_fix().filter(|f| f.level != Level::Tip).count();
        let tips = report.to_fix().filter(|f| f.level == Level::Tip).count();
        let areas = v_flex().flex_1().min_w_0().gap(px(10.)).children(report.areas.iter().map(|area| {
            let bar = Self::score_color(area.score, cx);
            h_flex()
                .gap(px(12.))
                .items_center()
                .child(div().w(px(170.)).text_size(px(12.5)).child(area.label))
                .child(
                    div()
                        .flex_1()
                        .h(px(6.))
                        .rounded_full()
                        .bg(theme.foreground.opacity(0.08))
                        .child(div().h_full().rounded_full().bg(bar).w(relative((area.score as f32 / 100.).clamp(0.02, 1.)))),
                )
                .child(
                    div()
                        .w(px(28.))
                        .text_right()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::MEDIUM)
                        .child(area.score.to_string()),
                )
        }));
        controls::group(cx)
            .p(px(20.))
            .child(
                h_flex()
                    .gap(px(32.))
                    .items_center()
                    .child(
                        v_flex()
                            .w(px(150.))
                            .flex_shrink_0()
                            .child(
                                h_flex()
                                    .items_baseline()
                                    .gap(px(4.))
                                    .child(
                                        div()
                                            .text_size(px(56.))
                                            .line_height(px(60.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(color)
                                            .child(report.score.to_string()),
                                    )
                                    .child(div().text_size(px(16.)).text_color(theme.muted_foreground).child("/100")),
                            )
                            .child(div().text_size(px(15.)).font_weight(FontWeight::SEMIBOLD).child(report.grade))
                            .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(match (to_fix, tips) {
                                (0, 0) => "Nothing to fix".to_owned(),
                                (0, tips) => format!("{tips} tips"),
                                (fix, 0) => format!("{fix} to fix"),
                                (fix, tips) => format!("{fix} to fix, {tips} tips"),
                            })),
                    )
                    .child(areas),
            )
    }
}

impl Render for AuditView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx).into_any_element();
        };
        if !self.fresh.contains(&app.id) {
            self.run(cx);
        }
        let running = self.running.contains(&app.id);
        let checked = self.checked.get(&app.id).cloned();
        let theme = cx.theme().clone();
        let subtitle = match (&checked, running) {
            (_, true) => format!("{} · checking…", app.name),
            (Some(checked), false) => format!("{} · checked {}", app.name, widgets::relative_time(checked.at, now())),
            (None, false) => app.name.clone(),
        };
        let header = h_flex()
            .h(px(60.))
            .flex_shrink_0()
            .px_5()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(theme.border)
            .child(
                v_flex()
                    .child(div().text_base().font_weight(FontWeight::SEMIBOLD).child("ASO audit"))
                    .child(div().text_xs().text_color(theme.muted_foreground).child(subtitle)),
            )
            .child(div().flex_1())
            .child(
                Button::new("audit-run")
                    .small()
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .label("Check again")
                    .loading(running)
                    .disabled(running)
                    .on_click(cx.listener(|this, _, _, cx| this.run(cx))),
            )
            .child(
                Button::new("audit-ai")
                    .small()
                    .primary()
                    .icon(gpui_kit::assets::IconName::Sparkles)
                    .label("Fix with AI")
                    .disabled(checked.is_none())
                    .on_click(cx.listener(|this, _, _, cx| this.fix_with_ai(cx))),
            );

        let Some(Checked { report, offline, .. }) = checked else {
            return v_flex()
                .size_full()
                .child(header)
                .child(widgets::empty_state(
                    gpui_kit::assets::IconName::Stethoscope,
                    "Checking your app…",
                    "Reading the App Store page, your listing, keywords and designs.",
                    cx,
                ))
                .into_any_element();
        };

        let to_fix: Vec<Finding> = report.to_fix().cloned().collect();
        let good: Vec<Finding> = report.findings.iter().filter(|f| f.level == Level::Good).cloned().collect();
        let mut fix_rows = controls::group(cx);
        for (index, finding) in to_fix.iter().enumerate() {
            fix_rows = fix_rows.child(self.finding_row(index, finding, cx));
        }
        let mut good_rows = controls::group(cx);
        for (index, finding) in good.iter().enumerate() {
            good_rows = good_rows.child(self.finding_row(1000 + index, finding, cx));
        }
        let listing_missing = report
            .findings
            .iter()
            .any(|f| f.title == "Subtitle and keyword field not checked");

        let body = v_flex()
            .w_full()
            .max_w(px(860.))
            .px_6()
            .pt(px(22.))
            .pb(px(40.))
            .gap(px(22.))
            .child(self.summary(&report, cx))
            .when_some(offline, |this, why| {
                this.child(controls::footnote(
                    format!("Couldn't reach the App Store, so this uses the page as last seen ({why})."),
                    cx,
                ))
            })
            .when(!to_fix.is_empty(), |this| {
                this.child(v_flex().child(controls::heading("Fix first", cx)).child(fix_rows))
            })
            .when(!good.is_empty(), |this| {
                this.child(v_flex().child(controls::heading("What's working", cx)).child(good_rows))
            })
            .child(controls::footnote(
                if listing_missing {
                    "The audit reads the live App Store page, your tracked keywords and your AppDoctor designs. Open Localization once to include the subtitle and hidden keyword field."
                } else {
                    "The audit reads the live App Store page, the listing Localization pulled, your tracked keywords and your AppDoctor designs."
                },
                cx,
            ));

        v_flex()
            .size_full()
            .child(header)
            .child(
                div()
                    .id("audit-page")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(h_flex().w_full().justify_center().child(body)),
            )
            .into_any_element()
    }
}
