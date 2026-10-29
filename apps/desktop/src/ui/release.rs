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
use crate::ui::widgets::{self, card, pill, status_dot};

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
            let rows = plan.summary_rows().into_iter().map(|(label, value)| {
                h_flex()
                    .gap_3()
                    .text_sm()
                    .child(div().w(px(100.)).flex_shrink_0().text_color(theme.muted_foreground).child(label))
                    .child(div().flex_1().child(value))
            });
            let (submit_view, submit_plan) = (view.clone(), plan.clone());
            dialog
                .title("Submit to App Review?")
                .w(px(520.))
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().text_color(theme.muted_foreground).child(
                            "This sends the version below to Apple for review. You can withdraw it while it waits, but Apple sees the submission.",
                        ))
                        .children(rows)
                        .when(plan.uses_non_exempt_encryption.is_none(), |this| {
                            this.child(div().text_sm().text_color(theme.warning).child(
                                "Export compliance isn't answered for this build; App Review may hold it until it is.",
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
        window.open_dialog(cx, move |dialog, _, cx| {
            let (view, id) = (view.clone(), submission.id.clone());
            dialog
                .title("Withdraw from App Review?")
                .w(px(460.))
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format!(
                    "The submission ({}) goes back to a draft. You'll need to submit again, which restarts the review queue.",
                    submission.state.label()
                )))
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

    fn builds_card(&self, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows = loaded.builds.iter().take(12).map(|build| {
            let selected = self.selected_build.as_deref() == Some(build.id.as_str());
            let ready = build.is_ready();
            let id = build.id.clone();
            h_flex()
                .id(SharedString::from(format!("build-{}", build.id)))
                .px_3()
                .py_2()
                .gap_3()
                .items_center()
                .rounded(theme.radius)
                .text_sm()
                .when(selected, |this| this.bg(theme.primary.opacity(0.1)))
                .when(ready, |this| {
                    this.cursor_pointer()
                        .hover(|style| style.bg(theme.muted.opacity(0.6)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_build = Some(id.clone());
                            cx.notify();
                        }))
                })
                .child(
                    div()
                        .size(px(14.))
                        .rounded_full()
                        .border_1()
                        .border_color(if selected { theme.primary } else { theme.border })
                        .when(selected, |this| this.border_4()),
                )
                .child(div().w(px(140.)).font_weight(FontWeight::MEDIUM).child(build.label()))
                .child(pill(
                    if build.expired {
                        "EXPIRED".to_owned()
                    } else {
                        build.processing_state.clone()
                    },
                    if ready { meter::up() } else { theme.muted_foreground },
                ))
                .child(div().flex_1())
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(build.uploaded_date.clone().unwrap_or_default()),
                )
        });
        card(cx)
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Build"))
            .when(loaded.builds.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme.muted_foreground).child(
                    "No builds uploaded yet. Upload one from Xcode, Transporter or `xcrun altool --upload-package`.",
                ))
            })
            .children(rows)
    }

    fn release_card(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let option = |id: &'static str, label: &'static str, value: ReleaseType, cx: &mut Context<Self>| {
            Button::new(id)
                .small()
                .label(label)
                .when(self.release_type == value, |button| button.primary())
                .when(self.release_type != value, |button| button.outline())
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.release_type = value;
                    cx.notify();
                }))
        };
        card(cx)
            .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Release"))
            .child(
                h_flex()
                    .gap_2()
                    .child(option("release-manual", "Manually", ReleaseType::Manual, cx))
                    .child(option("release-auto", "When approved", ReleaseType::AfterApproval, cx)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(self.release_type.describe()),
            )
    }

    fn submissions_card(&self, loaded: &Loaded, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rows: Vec<_> = loaded
            .submissions
            .iter()
            .take(8)
            .map(|submission| {
                let can_cancel = submission.state.can_cancel()
                    && submission.state != appdoctor_asc::ReviewSubmissionState::ReadyForReview;
                let target = submission.clone();
                h_flex()
                    .gap_3()
                    .items_center()
                    .text_sm()
                    .child(pill(submission.state.label(), theme.primary))
                    .child(
                        div()
                            .text_color(theme.muted_foreground)
                            .child(submission.submitted_date.clone().unwrap_or_else(|| "Not sent".into())),
                    )
                    .child(div().flex_1())
                    .when(can_cancel, |this| {
                        this.child(
                            Button::new(SharedString::from(format!("withdraw-{}", submission.id)))
                                .small()
                                .ghost()
                                .label("Withdraw…")
                                .disabled(self.busy)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.confirm_withdraw(target.clone(), window, cx)
                                })),
                        )
                    })
            })
            .collect();
        card(cx)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Review submissions"),
            )
            .when(rows.is_empty(), |this| {
                this.child(div().text_sm().text_color(theme.muted_foreground).child("None yet."))
            })
            .children(rows)
    }
}

impl Render for ReleaseView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let services = self.services(cx);
        let app = self.state.read(cx).selected().cloned();
        let has_key = services.has_asc_key();
        let muted = cx.theme().muted_foreground;

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

        v_flex()
            .id("release-page")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_4()
            .child(
                h_flex()
                    .items_start()
                    .child(div().flex_1().child(widgets::page_header(
                        &format!("Submit {} for review", app.name),
                        "Choose a processed build and how it releases. Nothing is sent until you confirm the summary.",
                        cx,
                    )))
                    .child(
                        Button::new("load-release")
                            .small()
                            .outline()
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
            .when_some(self.status.clone(), |this, status| {
                let (color, text) = match status {
                    Ok(text) => (meter::up(), text),
                    Err(text) => (cx.theme().danger, text),
                };
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_sm()
                        .child(status_dot(color))
                        .child(div().text_color(color).child(text)),
                )
            })
            .when_some(loaded, |this, loaded| {
                this.child(
                    card(cx)
                        .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Version"))
                        .child(match &loaded.version {
                            Some(version) => h_flex()
                                .gap_2()
                                .items_center()
                                .text_sm()
                                .child(
                                    div()
                                        .font_weight(FontWeight::MEDIUM)
                                        .child(version.version.clone().unwrap_or_default()),
                                )
                                .child(pill(version.state.clone().unwrap_or_default(), cx.theme().primary)),
                            None => h_flex().text_sm().text_color(muted).child(
                                "No editable iOS version. Create the next version in App Store Connect, then refresh.",
                            ),
                        }),
                )
                .child(self.builds_card(&loaded, cx))
                .child(self.release_card(cx))
                .child(
                    h_flex().child(
                        Button::new("review-submission")
                            .primary()
                            .label("Review and submit…")
                            .loading(self.busy)
                            .disabled(self.busy || !can_review)
                            .on_click(cx.listener(|this, _, window, cx| this.review(window, cx))),
                    ),
                )
                .child(self.submissions_card(&loaded, cx))
            })
            .into_any_element()
    }
}
