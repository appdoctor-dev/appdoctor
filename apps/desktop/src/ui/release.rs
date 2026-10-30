//! App Review: pick a processed build for the editable version, choose how
//! it releases, and submit, always through a confirmation dialog that shows
//! exactly what will be sent. Also lists submissions and can withdraw one.

use std::sync::Arc;

use appdoctor_asc::submission::Build;
use appdoctor_asc::{AppStoreVersion, Platform, ReleaseType, ReviewSubmission, SubmissionPlan};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::runtime;
use crate::services::Services;
use crate::state::{AppState, Section};
use crate::theme::meter;
use crate::ui::controls;
use crate::ui::widgets::{self, pill};

/// What App Store Connect says about the selected app.
#[derive(Clone)]
struct Loaded {
    app_id: u64,
    app_name: String,
    version: Option<AppStoreVersion>,
    builds: Vec<Build>,
    submissions: Vec<ReviewSubmission>,
}

pub struct ReleaseView {
    state: Entity<AppState>,
    loaded: Option<Loaded>,
    loading: bool,
    busy: bool,
    selected_build: Option<String>,
    release_type: ReleaseType,
    status: Option<Result<String, String>>,
    _subscriptions: Vec<Subscription>,
}

impl ReleaseView {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        Self {
            _subscriptions: vec![cx.observe(&state, |this, state, cx| {
                // A different app is selected: what was loaded is stale.
                if this.loaded.as_ref().map(|l| l.app_id) != state.read(cx).selected_app {
                    this.loaded = None;
                    this.selected_build = None;
                    this.status = None;
                }
                cx.notify();
            })],
            state,
            loaded: None,
            loading: false,
            busy: false,
            selected_build: None,
            release_type: ReleaseType::Manual,
            status: None,
        }
    }

    fn services(&self, cx: &App) -> Arc<Services> {
        self.state.read(cx).services.clone()
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return;
        };
        let services = self.services(cx);
        self.loading = true;
        self.status = None;
        cx.notify();
        let fetch = runtime::spawn(async move {
            let client = services.asc_client()?;
            let app_id = app.id.to_string();
            let version = match client.editable_version(&app_id, Platform::Ios).await {
                Ok(version) => Some(version),
                Err(appdoctor_asc::Error::NoEditableVersion) => None,
                Err(error) => return Err(error.into()),
            };
            let builds = client.builds(&app_id).await?;
            let submissions = client.review_submissions(&app_id).await?;
            Ok(Loaded {
                app_id: app.id,
                app_name: app.name,
                version,
                builds,
                submissions,
            })
        });
        cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(loaded) => {
                        this.selected_build = loaded.builds.iter().find(|b| b.is_ready()).map(|b| b.id.clone());
                        if let Some(release_type) = loaded.version.as_ref().and_then(|v| v.release_type)
                            && release_type != ReleaseType::Scheduled
                        {
                            this.release_type = release_type;
                        }
                        this.loaded = Some(loaded);
                    }
                    Err(error) => this.status = Some(Err(format!("{error:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Reads the plan (nothing changes yet) and opens the confirmation.
    fn review(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(loaded), Some(build_id)) = (self.loaded.clone(), self.selected_build.clone()) else {
            return;
        };
        let Some(version) = loaded.version.clone() else {
            return;
        };
        let services = self.services(cx);
        let release_type = self.release_type;
        self.busy = true;
        cx.notify();
        let prepare = runtime::spawn(async move {
            let client = services.asc_client()?;
            Ok(client
                .prepare_submission(
                    &loaded.app_id.to_string(),
                    &loaded.app_name,
                    &version.id,
                    &build_id,
                    release_type,
                    None,
                )
                .await?)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = prepare.await;
            this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match result {
                    Ok(plan) => this.confirm_submit(plan, window, cx),
                    Err(error) => this.status = Some(Err(format!("{error:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn confirm_submit(&mut self, plan: SubmissionPlan, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let theme = cx.theme();
            let rows = plan
                .summary_rows()
                .into_iter()
                .map(|(label, value)| (label.to_owned(), value));
            let (submit_view, submit_plan) = (view.clone(), plan.clone());
            dialog
                .title("Submit to App Review?")
                .w(px(520.))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(div().text_size(px(13.)).text_color(theme.muted_foreground).child(
                            "This sends the version below to Apple for review. You can withdraw it while it waits, but Apple sees the submission.",
                        ))
                        .child(controls::summary(rows, cx))
                        .when(plan.uses_non_exempt_encryption.is_none(), |this| {
                            this.child(controls::banner(
                                &Err("Export compliance isn't answered for this build; App Review may hold it until it is.".into()),
                                cx,
                            ))
                        }),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-submit")
                                .ghost()
                                .label("Cancel")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(Button::new("confirm-submit").primary().label("Submit to App Review").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let plan = submit_plan.clone();
                                submit_view.update(cx, |this, cx| this.submit(plan, cx)).ok();
                            },
                        )),
                )
        });
    }

    /// Only reachable from the confirmation dialog's button.
    fn submit(&mut self, plan: SubmissionPlan, cx: &mut Context<Self>) {
        let services = self.services(cx);
        self.busy = true;
        cx.notify();
        let submit = runtime::spawn(async move {
            let client = services.asc_client()?;
            Ok(client.submit_for_review(&plan, true).await?)
        });
        cx.spawn(async move |this, cx| {
            let result = submit.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                this.status = Some(match result {
                    Ok(submission) => Ok(format!("Submitted. App Review status: {}.", submission.state.label())),
                    Err(error) => Err(format!("{error:#}")),
                });
                this.load(cx);
            })
            .ok();
        })
        .detach();
    }

    fn confirm_withdraw(&mut self, submission: ReviewSubmission, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().downgrade();
        let app_name = self.loaded.as_ref().map(|l| l.app_name.clone()).unwrap_or_default();
        window.open_dialog(cx, move |dialog, _, cx| {
            let (view, id) = (view.clone(), submission.id.clone());
            let rows = [
                ("App".to_owned(), app_name.clone()),
                ("Status".to_owned(), submission.state.label().to_string()),
                (
                    "Submitted".to_owned(),
                    submission
                        .submitted_date
                        .as_deref()
                        .map_or_else(|| "Not sent".to_owned(), short_date),
                ),
            ];
            dialog
                .title("Withdraw from App Review?")
                .w(px(460.))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(div().text_size(px(13.)).text_color(cx.theme().muted_foreground).child(
                            "The submission goes back to a draft. You'll need to submit again, which restarts the review queue.",
                        ))
                        .child(controls::summary(rows, cx)),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-withdraw")
                                .ghost()
                                .label("Keep in review")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(Button::new("confirm-withdraw").danger().label("Withdraw").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let id = id.clone();
                                view.update(cx, |this, cx| this.withdraw(id, cx)).ok();
                            },
                        )),
                )
        });
    }

    fn withdraw(&mut self, submission_id: String, cx: &mut Context<Self>) {
        let services = self.services(cx);
        self.busy = true;
        cx.notify();
        let cancel = runtime::spawn(async move {
            let client = services.asc_client()?;
            Ok(client.cancel_submission(&submission_id, true).await?)
        });
        cx.spawn(async move |this, cx| {
            let result = cancel.await;
            this.update(cx, |this, cx| {
                this.busy = false;
                this.status = Some(match result {
                    Ok(submission) => Ok(format!("Withdrawn. Status: {}.", submission.state.label())),
                    Err(error) => Err(format!("{error:#}")),
                });
                this.load(cx);
            })
            .ok();
        })
        .detach();
    }

    fn version_group(&self, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let control = match &loaded.version {
            Some(version) => h_flex()
                .gap_2()
                .items_center()
                .text_size(px(13.))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .child(version.version.clone().unwrap_or_default()),
                )
                .child(pill(state_label(version.state.as_deref()), theme.primary)),
            None => h_flex()
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child("None being prepared"),
        };
        let section = section("Version", controls::rows([controls::row("iOS", control, cx).into_any_element()], cx), cx);
        if loaded.version.is_none() {
            section.child(controls::footnote(
                "Create the next version in App Store Connect, then refresh.",
                cx,
            ))
        } else {
            section
        }
    }

    fn builds_group(&self, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows: Vec<AnyElement> = loaded
            .builds
            .iter()
            .take(12)
            .map(|build| {
                let selected = self.selected_build.as_deref() == Some(build.id.as_str());
                let ready = build.is_ready();
                let id = build.id.clone();
                h_flex()
                    .id(SharedString::from(format!("build-{}", build.id)))
                    .min_h(px(38.))
                    .px(px(10.))
                    .gap(px(10.))
                    .items_center()
                    .text_size(px(13.))
                    .when(!ready, |this| this.opacity(0.5))
                    .when(ready, |this| {
                        this.cursor_pointer().on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_build = Some(id.clone());
                            cx.notify();
                        }))
                    })
                    .child(controls::radio(selected, cx))
                    .child(div().font_weight(FontWeight::MEDIUM).child(build.label()))
                    .child(pill(
                        if build.expired {
                            "Expired".to_owned()
                        } else {
                            state_label(Some(build.processing_state.as_str()))
                        },
                        if ready { meter::up() } else { theme.muted_foreground },
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(build.uploaded_date.clone().map(|date| short_date(&date)).unwrap_or_default()),
                    )
                    .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            return section(
                "Build",
                controls::rows(
                    [div()
                        .px(px(10.))
                        .py(px(11.))
                        .text_size(px(13.))
                        .text_color(theme.muted_foreground)
                        .child("No builds uploaded yet.")
                        .into_any_element()],
                    cx,
                ),
                cx,
            )
            .child(controls::footnote(
                "Upload one from Xcode, Transporter or `xcrun altool --upload-package`, then refresh.",
                cx,
            ));
        }
        section("Build", controls::rows(rows, cx), cx).child(controls::footnote(
            "Only builds Apple has finished processing can be submitted.",
            cx,
        ))
    }

    fn release_group(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let release = controls::Segmented::new("release-type", 230.)
            .label("Manually")
            .label("When Approved")
            .selected(usize::from(self.release_type == ReleaseType::AfterApproval))
            .on_select(cx.listener(|this, ix: &usize, _, cx| {
                this.release_type = if *ix == 0 {
                    ReleaseType::Manual
                } else {
                    ReleaseType::AfterApproval
                };
                cx.notify();
            }));
        section(
            "Release",
            controls::rows([controls::row("Release", release, cx).into_any_element()], cx),
            cx,
        )
        .child(controls::footnote(self.release_type.describe(), cx))
    }

    fn submissions_group(&self, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let mut rows: Vec<AnyElement> = loaded
            .submissions
            .iter()
            .take(8)
            .map(|submission| {
                let can_cancel = submission.state.can_cancel()
                    && submission.state != appdoctor_asc::ReviewSubmissionState::ReadyForReview;
                let target = submission.clone();
                h_flex()
                    .min_h(px(38.))
                    .px(px(10.))
                    .gap(px(10.))
                    .items_center()
                    .text_size(px(13.))
                    .child(pill(submission.state.label(), theme.primary))
                    .child(
                        div().text_color(theme.muted_foreground).child(
                            submission
                                .submitted_date
                                .as_deref()
                                .map_or_else(|| "Not sent".to_owned(), short_date),
                        ),
                    )
                    .child(div().flex_1())
                    .when(can_cancel, |this| {
                        this.child(
                            Button::new(SharedString::from(format!("withdraw-{}", submission.id)))
                                .small()
                                .label("Withdraw…")
                                .disabled(self.busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_withdraw(target.clone(), window, cx)
                                })),
                        )
                    })
                    .into_any_element()
            })
            .collect();
        if rows.is_empty() {
            rows.push(
                div()
                    .px(px(10.))
                    .py(px(11.))
                    .text_size(px(13.))
                    .text_color(theme.muted_foreground)
                    .child("None yet.")
                    .into_any_element(),
            );
        }
        section("Review submissions", controls::rows(rows, cx), cx)
    }
}

/// The widest the page grows.
const COLUMN: f32 = 680.;

/// A heading over a group.
fn section(title: &str, body: impl IntoElement, cx: &App) -> Div {
    v_flex().child(controls::heading(title.to_owned(), cx)).child(body)
}

/// App Store Connect's SCREAMING_CASE states as words: "Prepare for submission".
fn state_label(state: Option<&str>) -> String {
    let words = state.unwrap_or_default().replace('_', " ").to_lowercase();
    let mut chars = words.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// "2026-10-29T14:03:11-07:00" → "2026-10-29 14:03".
fn short_date(date: &str) -> String {
    match date.split_once('T') {
        Some((day, time)) => format!("{day} {}", time.get(..5).unwrap_or(time)),
        None => date.to_owned(),
    }
}

impl Render for ReleaseView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let services = self.services(cx);
        let app = self.state.read(cx).selected().cloned();
        let has_key = services.has_asc_key();

        if !has_key {
            return widgets::empty_state(
                IconName::CircleCheck,
                "Connect App Store Connect",
                "Add your App Store Connect API key in Settings to pick builds and submit versions for review from here.",
                cx,
            )
            .child(
                Button::new("go-settings")
                    .primary()
                    .label("Open Settings")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.state.update(cx, |state, cx| state.set_section(Section::Settings, cx))
                    })),
            )
            .into_any_element();
        }
        let Some(app) = app else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };

        let loaded = self.loaded.clone();
        let can_review = loaded.as_ref().is_some_and(|l| l.version.is_some()) && self.selected_build.is_some();

        div()
            .id("release-page")
            .size_full()
            .overflow_y_scroll()
            .child(
                h_flex().w_full().justify_center().child(
                    v_flex()
                        .w_full()
                        .max_w(px(COLUMN))
                        .px_6()
                        .pt(px(28.))
                        .pb(px(40.))
                        .gap(px(24.))
                        .child(
                            h_flex()
                                .gap_4()
                                .items_start()
                                .child(div().flex_1().child(widgets::page_header(
                                    &format!("Submit {} for review", app.name),
                                    "Choose a processed build and how it releases. Nothing is sent until you confirm the summary.",
                                    cx,
                                )))
                                .child(
                                    Button::new("load-release")
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .label(if loaded.is_some() {
                                            "Refresh"
                                        } else {
                                            "Load from App Store Connect"
                                        })
                                        .loading(self.loading)
                                        .disabled(self.loading)
                                        .on_click(cx.listener(|this, _, _, cx| this.load(cx))),
                                ),
                        )
                        .when_some(self.status.as_ref(), |this, status| {
                            this.child(controls::banner(status, cx))
                        })
                        .when_some(loaded, |this, loaded| {
                            this.child(self.version_group(&loaded, cx))
                                .child(self.builds_group(&loaded, cx))
                                .child(
                                    v_flex().child(self.release_group(cx)).child(
                                        h_flex().pt(px(14.)).justify_end().child(
                                            Button::new("review-submission")
                                                .primary()
                                                .label("Review and Submit…")
                                                .loading(self.busy)
                                                .disabled(self.busy || !can_review)
                                                .on_click(cx.listener(|this, _, window, cx| this.review(window, cx))),
                                        ),
                                    ),
                                )
                                .child(self.submissions_group(&loaded, cx))
                        }),
                ),
            )
            .into_any_element()
    }
}
