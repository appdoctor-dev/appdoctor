//! Settings: the AppDoctor account, App Store Connect and Apple Ads keys,
//! updates and where data lives. Every secret goes to the OS keychain.

use std::path::PathBuf;

use appdoctor_entitlement::Status;
use appdoctor_secrets::{AppleAdsKey, AscKey, Secret};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::entitlement;
use crate::runtime;
use crate::state::{AppState, UpdateStatus};
use crate::theme::meter;
use crate::ui::widgets::{self, card, pill, status_dot};

pub struct SettingsView {
    state: Entity<AppState>,
    api_base: Entity<InputState>,

    asc_issuer: Entity<InputState>,
    asc_key_id: Entity<InputState>,
    asc_file: Option<(String, String)>,
    asc_saved: Option<(String, String)>,
    asc_busy: bool,
    asc_result: Option<Result<String, String>>,

    ads_key: Option<AppleAdsKey>,
    ads_public_pem: Option<String>,
    ads_client_id: Entity<InputState>,
    ads_team_id: Entity<InputState>,
    ads_key_id: Entity<InputState>,
    ads_busy: bool,
    ads_result: Option<Result<String, String>>,

    _subscriptions: Vec<Subscription>,
}

fn input(placeholder: &str, value: &str, window: &mut Window, cx: &mut Context<InputState>) -> InputState {
    InputState::new(window, cx)
        .placeholder(placeholder.to_owned())
        .default_value(value.to_owned())
}

impl SettingsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let services = state.read(cx).services.clone();
        let asc: Option<AscKey> = services.vault.get(Secret::AppStoreConnect).ok().flatten();
        let ads: Option<AppleAdsKey> = services.vault.get(Secret::AppleAds).ok().flatten();
        let ads_public_pem = ads
            .as_ref()
            .and_then(|key| appdoctor_jwt::EcKey::from_pem(&key.private_key_pem).ok())
            .and_then(|key| key.public_key_pem().ok());
        let api_base = services.api_base();
        let ads_ids = ads.clone().unwrap_or_default();

        Self {
            api_base: cx.new(|cx| input("http://localhost:8787", &api_base, window, cx)),
            asc_issuer: cx.new(|cx| {
                input(
                    "Issuer ID, e.g. 57246542-96fe-1a63-e053-0824d011072a",
                    asc.as_ref().map_or("", |key| &key.issuer_id),
                    window,
                    cx,
                )
            }),
            asc_key_id: cx.new(|cx| {
                input(
                    "Key ID, e.g. 2X9R4HXF34",
                    asc.as_ref().map_or("", |key| &key.key_id),
                    window,
                    cx,
                )
            }),
            asc_file: None,
            asc_saved: asc.map(|key| (key.issuer_id, key.key_id)),
            asc_busy: false,
            asc_result: None,
            ads_client_id: cx.new(|cx| input("clientId  SEARCHADS.…", &ads_ids.client_id, window, cx)),
            ads_team_id: cx.new(|cx| input("teamId  SEARCHADS.…", &ads_ids.team_id, window, cx)),
            ads_key_id: cx.new(|cx| input("keyId", &ads_ids.key_id, window, cx)),
            ads_key: ads,
            ads_public_pem,
            ads_busy: false,
            ads_result: None,
            _subscriptions: vec![cx.observe(&state, |_, _, cx| cx.notify())],
            state,
        }
    }

    fn services(&self, cx: &App) -> std::sync::Arc<crate::services::Services> {
        self.state.read(cx).services.clone()
    }

    // Account

    fn sign_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let services = self.services(cx);
        let state = self.state.clone();
        state.update(cx, |state, cx| {
            state.account.busy = true;
            state.account.error = None;
            cx.notify();
        });
        let start = runtime::spawn(entitlement::start_sign_in(services.clone()));
        cx.spawn_in(window, async move |_, cx| {
            let code = match start.await {
                Ok(code) => code,
                Err(error) => {
                    state.update(cx, |state, cx| {
                        state.account.busy = false;
                        state.account.error = Some(format!("{error:#}"));
                        cx.notify();
                    });
                    return;
                }
            };
            cx.update(|_, cx| cx.open_url(code.browser_url())).ok();
            state.update(cx, |state, cx| {
                state.account.pending = Some(code.clone());
                cx.notify();
            });
            let result = runtime::spawn(entitlement::finish_sign_in(services, code)).await;
            state.update(cx, |state, cx| {
                state.account.busy = false;
                state.account.pending = None;
                match result {
                    Ok(status) => state.account.status = status,
                    Err(error) => state.account.error = Some(format!("{error:#}")),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn sign_out(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let services = self.services(cx);
        if let Err(error) = entitlement::sign_out(&services) {
            window.push_notification(Notification::error(error.to_string()), cx);
        }
        self.state.update(cx, |state, cx| {
            state.account = entitlement::Account::load(&services);
            cx.notify();
        });
    }

    fn save_api_base(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let base = self.api_base.read(cx).value().to_string();
        match self.services(cx).set_api_base(&base) {
            Ok(()) => window.push_notification(Notification::success("Saved the API address"), cx),
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
    }

    // App Store Connect

    fn choose_p8(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Use Key".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path): Option<PathBuf> = paths.into_iter().next() else {
                return;
            };
            let contents = std::fs::read_to_string(&path);
            this.update_in(cx, |this, window, cx| {
                match contents {
                    Ok(pem) if appdoctor_jwt::EcKey::from_pem(&pem).is_ok() => {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        // AuthKey_<KEYID>.p8: fill the Key ID in for the user.
                        if let Some(key_id) = name.strip_prefix("AuthKey_").and_then(|n| n.strip_suffix(".p8"))
                            && this.asc_key_id.read(cx).value().is_empty()
                        {
                            let key_id = key_id.to_owned();
                            this.asc_key_id
                                .update(cx, |input, cx| input.set_value(key_id, window, cx));
                        }
                        this.asc_file = Some((name, pem));
                    }
                    Ok(_) => window.push_notification(
                        Notification::error("That file isn't an App Store Connect API key (.p8)."),
                        cx,
                    ),
                    Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn save_asc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let issuer_id = self.asc_issuer.read(cx).value().trim().to_owned();
        let key_id = self.asc_key_id.read(cx).value().trim().to_owned();
        let services = self.services(cx);
        let existing: Option<AscKey> = services.vault.get(Secret::AppStoreConnect).ok().flatten();
        let private_key_pem = match (&self.asc_file, existing) {
            (Some((_, pem)), _) => pem.clone(),
            (None, Some(key)) => key.private_key_pem,
            (None, None) => {
                window.push_notification(Notification::error("Choose the .p8 key file first."), cx);
                return;
            }
        };
        if issuer_id.is_empty() || key_id.is_empty() {
            window.push_notification(Notification::error("Enter both the Issuer ID and the Key ID."), cx);
            return;
        }
        let key = AscKey {
            issuer_id,
            key_id,
            private_key_pem,
        };
        match services.vault.set(Secret::AppStoreConnect, &key) {
            Ok(()) => {
                self.asc_saved = Some((key.issuer_id, key.key_id));
                self.asc_file = None;
                window.push_notification(Notification::success("App Store Connect key saved to the keychain"), cx);
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
        cx.notify();
    }

    fn test_asc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let services = self.services(cx);
        self.asc_busy = true;
        self.asc_result = None;
        cx.notify();
        let test = runtime::spawn(async move {
            let key: AscKey = services
                .vault
                .get(Secret::AppStoreConnect)?
                .ok_or_else(|| anyhow::anyhow!("save a key first"))?;
            let credentials = appdoctor_asc::Credentials::new(key.issuer_id, key.key_id, &key.private_key_pem)?;
            let apps = appdoctor_asc::Client::new(credentials).apps().await?;
            Ok(apps)
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = test.await;
            this.update(cx, |this, cx| {
                this.asc_busy = false;
                this.asc_result = Some(match result {
                    Ok(apps) => Ok(format!(
                        "Connected. {} app{} on this account.",
                        apps.len(),
                        if apps.len() == 1 { "" } else { "s" }
                    )),
                    Err(error) => Err(format!("{error:#}")),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_asc(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.services(cx).vault.delete(Secret::AppStoreConnect) {
            Ok(()) => {
                self.asc_saved = None;
                self.asc_result = None;
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
        cx.notify();
    }

    // Apple Ads

    fn generate_ads_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = appdoctor_apple_ads::generate_key();
        let (Ok(private_key_pem), Ok(public_pem)) = (key.to_pkcs8_pem(), key.public_key_pem()) else {
            window.push_notification(Notification::error("Couldn't encode the new key."), cx);
            return;
        };
        let stored = AppleAdsKey {
            private_key_pem,
            ..Default::default()
        };
        match self.services(cx).vault.set(Secret::AppleAds, &stored) {
            Ok(()) => {
                self.ads_key = Some(stored);
                self.ads_public_pem = Some(public_pem);
                self.ads_result = None;
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
        cx.notify();
    }

    fn save_ads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut key) = self.ads_key.clone() else {
            return;
        };
        key.client_id = self.ads_client_id.read(cx).value().trim().to_owned();
        key.team_id = self.ads_team_id.read(cx).value().trim().to_owned();
        key.key_id = self.ads_key_id.read(cx).value().trim().to_owned();
        if !key.is_complete() {
            window.push_notification(
                Notification::error("Enter the clientId, teamId and keyId from Apple Ads."),
                cx,
            );
            return;
        }
        match self.services(cx).vault.set(Secret::AppleAds, &key) {
            Ok(()) => {
                self.ads_key = Some(key);
                window.push_notification(Notification::success("Apple Ads credentials saved to the keychain"), cx);
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
        cx.notify();
    }

    fn test_ads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let services = self.services(cx);
        self.ads_busy = true;
        self.ads_result = None;
        cx.notify();
        let test = runtime::spawn(async move {
            let mut key: AppleAdsKey = services
                .vault
                .get(Secret::AppleAds)?
                .filter(AppleAdsKey::is_complete)
                .ok_or_else(|| anyhow::anyhow!("save the IDs from Apple Ads first"))?;
            let private_key = appdoctor_jwt::EcKey::from_pem(&key.private_key_pem)?;
            let client = appdoctor_apple_ads::Client::new(appdoctor_apple_ads::Credentials::new(
                &key.client_id,
                &key.team_id,
                &key.key_id,
                private_key,
            ));
            let acls = client.acls().await?;
            let first = acls.first().ok_or(appdoctor_apple_ads::Error::NoAdAccount)?;
            key.ad_account_id = Some(first.ad_account.id);
            services.vault.set(Secret::AppleAds, &key)?;
            Ok(first
                .ad_account
                .name
                .clone()
                .unwrap_or_else(|| first.ad_account.id.to_string()))
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = test.await;
            this.update(cx, |this, cx| {
                this.ads_busy = false;
                this.ads_result = Some(match result {
                    Ok(account) => Ok(format!("Connected to ad account “{account}”.")),
                    Err(error) => Err(format!("{error:#}")),
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn remove_ads(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.services(cx).vault.delete(Secret::AppleAds) {
            Ok(()) => {
                self.ads_key = None;
                self.ads_public_pem = None;
                self.ads_result = None;
            }
            Err(error) => window.push_notification(Notification::error(error.to_string()), cx),
        }
        cx.notify();
    }

    // Rendering

    fn section(&self, title: &str, description: &str, cx: &App) -> Div {
        card(cx).w_full().max_w(px(760.)).child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(description.to_owned()),
                ),
        )
    }

    fn field(label: &str, child: impl IntoElement, cx: &App) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(110.))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(label.to_owned()),
            )
            .child(div().flex_1().child(child))
    }

    fn result_line(result: &Option<Result<String, String>>, cx: &App) -> Option<impl IntoElement> {
        let result = result.as_ref()?;
        let (color, text) = match result {
            Ok(text) => (meter::up(), text.clone()),
            Err(text) => (cx.theme().danger, text.clone()),
        };
        Some(
            h_flex()
                .gap_2()
                .items_center()
                .text_sm()
                .child(status_dot(color))
                .child(div().text_color(color).child(text)),
        )
    }

    fn account_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.state.read(cx).account.clone();
        let services = self.services(cx);
        let signed_in = entitlement::Account::has_session(&services);
        let (color, headline) = match &account.status {
            Status::DevBuild => (
                meter::up(),
                "Developer build: every feature is unlocked, no account needed.".to_owned(),
            ),
            Status::Active { plan, expires_at, .. } => (
                meter::up(),
                format!(
                    "Active · {} plan · works offline for {} more days",
                    capitalize(plan),
                    ((*expires_at - crate::rank_check::now()) / 86_400).max(0)
                ),
            ),
            Status::Expired { plan } => (
                cx.theme().warning,
                format!(
                    "Your {} licence needs a refresh. Connect to the internet and refresh.",
                    capitalize(plan)
                ),
            ),
            Status::NotActivated => (cx.theme().muted_foreground, "Not activated on this Mac.".to_owned()),
            Status::Invalid(reason) => (cx.theme().danger, format!("Licence problem: {reason}")),
        };

        self.section(
            "AppDoctor account",
            "Sign in to activate signed builds and updates. Only your device ID, device name and app version are sent; keywords and ranks never leave this Mac.",
            cx,
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .text_sm()
                .child(status_dot(color))
                .child(headline),
        )
        .when_some(account.pending.clone(), |this, code| {
            this.child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted.opacity(0.5))
                    .child(
                        div()
                            .text_2xl()
                            .font_family(cx.theme().mono_font_family.clone())
                            .font_weight(FontWeight::BOLD)
                            .child(code.user_code.clone()),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .text_sm()
                            .child("Confirm this code in your browser to finish signing in.")
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(code.verification_uri.clone()),
                            ),
                    )
                    .child(div().flex_1())
                    .child(Button::new("reopen-browser").small().outline().label("Open browser").on_click({
                        let url = code.browser_url().to_owned();
                        move |_, _, cx| cx.open_url(&url)
                    })),
            )
        })
        .when_some(account.error.clone(), |this, error| {
            this.child(div().text_sm().text_color(cx.theme().danger).child(error))
        })
        .when(!matches!(account.status, Status::DevBuild), |this| {
            this.child(
                h_flex()
                    .gap_2()
                    .when(!signed_in, |this| {
                        this.child(
                            Button::new("sign-in")
                                .small()
                                .primary()
                                .icon(IconName::User)
                                .label("Sign in")
                                .loading(account.busy)
                                .disabled(account.busy)
                                .on_click(cx.listener(|this, _, window, cx| this.sign_in(window, cx))),
                        )
                    })
                    .when(signed_in, |this| {
                        this.child(
                            Button::new("refresh-licence")
                                .small()
                                .outline()
                                .icon(IconName::RefreshCw)
                                .label("Refresh licence")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    let task = this.state.update(cx, |state, cx| state.refresh_entitlement(cx));
                                    cx.spawn_in(window, async move |_, cx| {
                                        if let Err(error) = task.await {
                                            cx.update(|window, cx| {
                                                window.push_notification(Notification::error(format!("{error:#}")), cx)
                                            })
                                            .ok();
                                        }
                                    })
                                    .detach();
                                })),
                        )
                        .child(
                            Button::new("sign-out")
                                .small()
                                .ghost()
                                .label("Sign out")
                                .on_click(cx.listener(|this, _, window, cx| this.sign_out(window, cx))),
                        )
                    }),
            )
        })
        .child(Self::field(
            "API server",
            h_flex().gap_2().child(div().flex_1().child(Input::new(&self.api_base).small())).child(
                Button::new("save-api")
                    .small()
                    .outline()
                    .label("Save")
                    .on_click(cx.listener(|this, _, window, cx| this.save_api_base(window, cx))),
            ),
            cx,
        ))
    }

    fn asc_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let file_label = match (&self.asc_file, &self.asc_saved) {
            (Some((name, _)), _) => name.clone(),
            (None, Some(_)) => "Stored in the keychain".to_owned(),
            (None, None) => "No key chosen".to_owned(),
        };
        self.section(
            "App Store Connect",
            "Lets AppDoctor read and write your app's name, subtitle, keywords and description, and submit versions to App Review after you confirm a summary. Create a key under Users and Access → Integrations → App Store Connect API (App Manager role).",
            cx,
        )
        .when_some(self.asc_saved.clone(), |this, (_, key_id)| {
            this.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .child(status_dot(meter::up()))
                    .child(format!("Key {key_id} saved in the keychain")),
            )
        })
        .child(Self::field("Issuer ID", Input::new(&self.asc_issuer).small(), cx))
        .child(Self::field("Key ID", Input::new(&self.asc_key_id).small(), cx))
        .child(Self::field(
            "Private key",
            h_flex()
                .gap_3()
                .items_center()
                .child(
                    Button::new("choose-p8")
                        .small()
                        .outline()
                        .icon(IconName::FolderOpen)
                        .label("Choose .p8 file…")
                        .on_click(cx.listener(|this, _, window, cx| this.choose_p8(window, cx))),
                )
                .child(div().text_sm().text_color(cx.theme().muted_foreground).child(file_label)),
            cx,
        ))
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("save-asc")
                        .small()
                        .primary()
                        .label("Save to keychain")
                        .on_click(cx.listener(|this, _, window, cx| this.save_asc(window, cx))),
                )
                .when(self.asc_saved.is_some(), |this| {
                    this.child(
                        Button::new("test-asc")
                            .small()
                            .outline()
                            .label("Test connection")
                            .loading(self.asc_busy)
                            .disabled(self.asc_busy)
                            .on_click(cx.listener(|this, _, window, cx| this.test_asc(window, cx))),
                    )
                    .child(
                        Button::new("remove-asc")
                            .small()
                            .ghost()
                            .label("Remove")
                            .on_click(cx.listener(|this, _, window, cx| this.remove_asc(window, cx))),
                    )
                }),
        )
        .when_some(Self::result_line(&self.asc_result, cx), |this, line| this.child(line))
    }

    fn ads_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let section = self.section(
            "Apple Ads",
            "Adds Apple's search popularity (0–100) to your keywords. AppDoctor makes the key pair; the private key stays in your keychain.",
            cx,
        );
        let Some(public_pem) = self.ads_public_pem.clone() else {
            return section.child(
                h_flex().child(
                    Button::new("generate-ads-key")
                        .small()
                        .primary()
                        .icon(IconName::Plus)
                        .label("Generate key pair")
                        .on_click(cx.listener(|this, _, window, cx| this.generate_ads_key(window, cx))),
                ),
            );
        };
        let complete = self.ads_key.as_ref().is_some_and(AppleAdsKey::is_complete);
        section
            .child(
                div()
                    .text_sm()
                    .child("1. Copy this public key. 2. In Apple Ads, open Account Settings → API and paste it. 3. Enter the clientId, teamId and keyId Apple shows you."),
            )
            .child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .p_3()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted.opacity(0.5))
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_color(muted)
                            .child(public_pem.clone()),
                    )
                    .child(Clipboard::new("copy-ads-public-key").value(public_pem).tooltip("Copy public key")),
            )
            .child(Self::field("clientId", Input::new(&self.ads_client_id).small(), cx))
            .child(Self::field("teamId", Input::new(&self.ads_team_id).small(), cx))
            .child(Self::field("keyId", Input::new(&self.ads_key_id).small(), cx))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("save-ads")
                            .small()
                            .primary()
                            .label("Save to keychain")
                            .on_click(cx.listener(|this, _, window, cx| this.save_ads(window, cx))),
                    )
                    .when(complete, |this| {
                        this.child(
                            Button::new("test-ads")
                                .small()
                                .outline()
                                .label("Test connection")
                                .loading(self.ads_busy)
                                .disabled(self.ads_busy)
                                .on_click(cx.listener(|this, _, window, cx| this.test_ads(window, cx))),
                        )
                    })
                    .child(
                        Button::new("remove-ads")
                            .small()
                            .ghost()
                            .label("Remove key")
                            .on_click(cx.listener(|this, _, window, cx| this.remove_ads(window, cx))),
                    ),
            )
            .when_some(Self::result_line(&self.ads_result, cx), |this, line| this.child(line))
    }

    fn about_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let data_dir = state.services.data_dir.clone();
        let (color, update) = match &state.update {
            UpdateStatus::Unknown | UpdateStatus::Checking => {
                (cx.theme().muted_foreground, "Checking for updates…".to_owned())
            }
            UpdateStatus::Current => (meter::up(), "You're up to date.".to_owned()),
            UpdateStatus::Available(info) => (cx.theme().primary, format!("AppDoctor {} is available.", info.version)),
            UpdateStatus::Failed(_) => (
                cx.theme().muted_foreground,
                "Couldn't reach the update server.".to_owned(),
            ),
        };
        self.section(
            "About",
            "Your apps, keywords and rank history are stored in one SQLite file on this Mac. AppDoctor sends no analytics.",
            cx,
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .text_sm()
                .child(pill(format!("Version {}", env!("CARGO_PKG_VERSION")), cx.theme().muted_foreground))
                .child(status_dot(color))
                .child(update)
                .child(div().flex_1())
                .child(
                    Button::new("check-update")
                        .small()
                        .outline()
                        .label("Check for updates")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.state.update(cx, |state, cx| state.check_for_update(cx))
                        })),
                ),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .text_sm()
                .child(div().text_color(cx.theme().muted_foreground).child("Data folder"))
                .child(div().flex_1().truncate().font_family(cx.theme().mono_font_family.clone()).text_xs().child(data_dir.display().to_string()))
                .child(Button::new("reveal-data").small().ghost().label("Show").on_click(move |_, _, cx| {
                    cx.reveal_path(&data_dir.join("appdoctor.db"))
                })),
        )
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-page")
            .size_full()
            .overflow_y_scroll()
            .p_6()
            .gap_5()
            .child(widgets::page_header(
                "Settings",
                "Connections and your account. Secrets are kept in the system keychain.",
                cx,
            ))
            .child(self.account_section(cx))
            .child(self.asc_section(cx))
            .child(self.ads_section(cx))
            .child(self.about_section(cx))
    }
}
