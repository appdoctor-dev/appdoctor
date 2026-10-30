//! Settings: the AppDoctor account, App Store Connect and Apple Ads keys,
//! updates and where data lives. Every secret goes to the OS keychain.

use std::path::PathBuf;

use appdoctor_entitlement::Status;
use appdoctor_secrets::{AppleAdsKey, AscKey, Secret};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::input::InputState;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::entitlement;
use crate::runtime;
use crate::state::{AppState, UpdateStatus};
use crate::theme::meter;
use crate::ui::controls;
use crate::ui::widgets::{self, status_dot};

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
                    "57246542-96fe-1a63-e053-0824d011072a",
                    asc.as_ref().map_or("", |key| &key.issuer_id),
                    window,
                    cx,
                )
            }),
            asc_key_id: cx.new(|cx| {
                input(
                    "2X9R4HXF34",
                    asc.as_ref().map_or("", |key| &key.key_id),
                    window,
                    cx,
                )
            }),
            asc_file: None,
            asc_saved: asc.map(|key| (key.issuer_id, key.key_id)),
            asc_busy: false,
            asc_result: None,
            ads_client_id: cx.new(|cx| input("SEARCHADS.…", &ads_ids.client_id, window, cx)),
            ads_team_id: cx.new(|cx| input("SEARCHADS.…", &ads_ids.team_id, window, cx)),
            ads_key_id: cx.new(|cx| input("Key ID", &ads_ids.key_id, window, cx)),
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

    /// A section as System Settings draws one: a heading, the group, then
    /// whatever follows it (buttons, an outcome, a footnote).
    fn section(title: &str, body: impl IntoElement, cx: &App) -> Div {
        v_flex().child(controls::heading(title.to_owned(), cx)).child(body)
    }

    /// Push buttons under a group, aligned right like a macOS form's.
    fn actions() -> Div {
        h_flex().pt(px(10.)).gap_2().justify_end()
    }

    fn account_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let account = self.state.read(cx).account.clone();
        let services = self.services(cx);
        let signed_in = entitlement::Account::has_session(&services);
        let theme = cx.theme().clone();
        let (color, title, detail) = match &account.status {
            Status::DevBuild => (
                meter::up(),
                "Developer build".to_owned(),
                "Every feature is unlocked; no account needed.".to_owned(),
            ),
            Status::Active { plan, expires_at, .. } => (
                meter::up(),
                format!("{} plan", capitalize(plan)),
                format!(
                    "Active · works offline for {} more days",
                    ((*expires_at - crate::rank_check::now()) / 86_400).max(0)
                ),
            ),
            Status::Expired { plan } => (
                theme.warning,
                format!("{} plan", capitalize(plan)),
                "Needs a refresh. Connect to the internet and refresh the licence.".to_owned(),
            ),
            Status::NotActivated => (
                theme.muted_foreground,
                "Not signed in".to_owned(),
                "Sign in to activate AppDoctor on this computer.".to_owned(),
            ),
            Status::Invalid(reason) => (theme.danger, "Licence problem".to_owned(), reason.clone()),
        };

        let buttons = h_flex()
            .gap_2()
            .flex_shrink_0()
            .when(!matches!(account.status, Status::DevBuild) && !signed_in, |this| {
                this.child(
                    Button::new("sign-in")
                        .small()
                        .primary()
                        .label("Sign In…")
                        .loading(account.busy)
                        .disabled(account.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.sign_in(window, cx))),
                )
            })
            .when(!matches!(account.status, Status::DevBuild) && signed_in, |this| {
                this.child(
                    Button::new("sign-out")
                        .small()
                        .ghost()
                        .label("Sign Out")
                        .on_click(cx.listener(|this, _, window, cx| this.sign_out(window, cx))),
                )
                .child(
                    Button::new("refresh-licence")
                        .small()
                        .label("Refresh")
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
            });

        let status = h_flex()
            .px(px(10.))
            .py(px(10.))
            .gap(px(10.))
            .items_center()
            .child(
                div()
                    .size(px(32.))
                    .flex_shrink_0()
                    .rounded(px(8.))
                    .bg(theme.primary)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(gpui_kit::assets::IconName::Stethoscope)
                            .size(px(17.))
                            .text_color(theme.primary_foreground),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(title),
                    )
                    .child(
                        h_flex()
                            .gap(px(6.))
                            .items_center()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(status_dot(color))
                            .child(div().min_w_0().truncate().child(detail)),
                    ),
            )
            .child(buttons);

        let mut items = vec![status.into_any_element()];
        if let Some(code) = account.pending.clone() {
            let url = code.browser_url().to_owned();
            items.push(
                h_flex()
                    .px(px(10.))
                    .py(px(12.))
                    .gap(px(14.))
                    .items_center()
                    .child(
                        div()
                            .text_size(px(24.))
                            .font_family(theme.mono_font_family.clone())
                            .font_weight(FontWeight::BOLD)
                            .child(code.user_code.clone()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .text_size(px(12.))
                            .child("Confirm this code in your browser to finish signing in.")
                            .child(
                                div()
                                    .truncate()
                                    .text_color(theme.muted_foreground)
                                    .child(code.verification_uri.clone()),
                            ),
                    )
                    .child(
                        Button::new("reopen-browser")
                            .small()
                            .label("Open Browser")
                            .on_click(move |_, _, cx| cx.open_url(&url)),
                    )
                    .into_any_element(),
            );
        }
        items.push(
            controls::row(
                "API server",
                h_flex()
                    .gap_2()
                    .child(controls::text_field(&self.api_base, cx).w(px(FIELD - 64.)))
                    .child(
                        Button::new("save-api")
                            .small()
                            .label("Save")
                            .on_click(cx.listener(|this, _, window, cx| this.save_api_base(window, cx))),
                    ),
                cx,
            )
            .into_any_element(),
        );

        Self::section("Account", controls::rows(items, cx), cx)
            .when_some(account.error.clone(), |this, error| {
                this.child(div().pt(px(8.)).child(controls::banner(&Err(error), cx)))
            })
            .child(controls::footnote(
                "Signing in activates signed builds and updates. Only your device ID, device name and app version are sent; keywords and ranks never leave this computer.",
                cx,
            ))
    }

    fn asc_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let file_label = match (&self.asc_file, &self.asc_saved) {
            (Some((name, _)), _) => name.clone(),
            (None, Some(_)) => "In the keychain".to_owned(),
            (None, None) => "None chosen".to_owned(),
        };
        let mut items = Vec::new();
        if let Some((_, key_id)) = &self.asc_saved {
            items.push(
                controls::row(
                    "Status",
                    h_flex()
                        .gap(px(6.))
                        .items_center()
                        .text_size(px(13.))
                        .text_color(muted)
                        .child(status_dot(meter::up()))
                        .child(format!("Key {key_id} saved")),
                    cx,
                )
                .into_any_element(),
            );
        }
        items.push(
            controls::row(
                "Issuer ID",
                controls::text_field(&self.asc_issuer, cx).w(px(FIELD)),
                cx,
            )
            .into_any_element(),
        );
        items.push(controls::row("Key ID", controls::text_field(&self.asc_key_id, cx).w(px(FIELD)), cx).into_any_element());
        items.push(
            controls::row(
                "Private key",
                h_flex()
                    .gap_2()
                    .items_center()
                    .min_w_0()
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(muted)
                            .child(file_label),
                    )
                    .child(
                        Button::new("choose-p8")
                            .small()
                            .label("Choose…")
                            .on_click(cx.listener(|this, _, window, cx| this.choose_p8(window, cx))),
                    ),
                cx,
            )
            .into_any_element(),
        );

        Self::section("App Store Connect", controls::rows(items, cx), cx)
            .child(
                Self::actions()
                    .when(self.asc_saved.is_some(), |this| {
                        this.child(
                            Button::new("remove-asc")
                                .small()
                                .ghost()
                                .label("Remove")
                                .on_click(cx.listener(|this, _, window, cx| this.remove_asc(window, cx))),
                        )
                        .child(
                            Button::new("test-asc")
                                .small()
                                .label("Test Connection")
                                .loading(self.asc_busy)
                                .disabled(self.asc_busy)
                                .on_click(cx.listener(|this, _, window, cx| this.test_asc(window, cx))),
                        )
                    })
                    .child(
                        Button::new("save-asc")
                            .small()
                            .primary()
                            .label("Save to Keychain")
                            .on_click(cx.listener(|this, _, window, cx| this.save_asc(window, cx))),
                    ),
            )
            .when_some(self.asc_result.as_ref(), |this, result| {
                this.child(div().pt(px(8.)).child(controls::banner(result, cx)))
            })
            .child(controls::footnote(
                "Lets AppDoctor read and write your app's name, subtitle, keywords and description, and submit versions to App Review after you confirm a summary. Create a key in App Store Connect under Users and Access → Integrations → App Store Connect API, with the App Manager role.",
                cx,
            ))
    }

    fn ads_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let Some(public_pem) = self.ads_public_pem.clone() else {
            return Self::section(
                "Apple Ads",
                controls::rows(
                    [controls::row(
                        "Key pair",
                        Button::new("generate-ads-key")
                            .small()
                            .label("Generate…")
                            .on_click(cx.listener(|this, _, window, cx| this.generate_ads_key(window, cx))),
                        cx,
                    )
                    .into_any_element()],
                    cx,
                ),
                cx,
            )
            .child(controls::footnote(
                "Adds Apple's search popularity (0–100) to your keywords. AppDoctor makes a key pair for you; the private key stays in your keychain.",
                cx,
            ));
        };
        let complete = self.ads_key.as_ref().is_some_and(AppleAdsKey::is_complete);
        let id_row = |name: &'static str, input: &Entity<InputState>, cx: &App| {
            controls::row(name, controls::text_field(input, cx).w(px(FIELD)), cx).into_any_element()
        };
        let items = vec![
            controls::stacked(
                "Public key",
                None,
                h_flex()
                    .gap_2()
                    .items_start()
                    .p(px(8.))
                    .rounded(px(7.))
                    .bg(theme.foreground.opacity(0.06))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(11.))
                            .line_height(px(15.))
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.muted_foreground)
                            .child(public_pem.clone()),
                    )
                    .child(Clipboard::new("copy-ads-public-key").value(public_pem).tooltip("Copy public key")),
                cx,
            )
            .into_any_element(),
            id_row("clientId", &self.ads_client_id, cx),
            id_row("teamId", &self.ads_team_id, cx),
            id_row("keyId", &self.ads_key_id, cx),
        ];

        Self::section("Apple Ads", controls::rows(items, cx), cx)
            .child(
                Self::actions()
                    .child(
                        Button::new("remove-ads")
                            .small()
                            .ghost()
                            .label("Remove Key")
                            .on_click(cx.listener(|this, _, window, cx| this.remove_ads(window, cx))),
                    )
                    .when(complete, |this| {
                        this.child(
                            Button::new("test-ads")
                                .small()
                                .label("Test Connection")
                                .loading(self.ads_busy)
                                .disabled(self.ads_busy)
                                .on_click(cx.listener(|this, _, window, cx| this.test_ads(window, cx))),
                        )
                    })
                    .child(
                        Button::new("save-ads")
                            .small()
                            .primary()
                            .label("Save to Keychain")
                            .on_click(cx.listener(|this, _, window, cx| this.save_ads(window, cx))),
                    ),
            )
            .when_some(self.ads_result.as_ref(), |this, result| {
                this.child(div().pt(px(8.)).child(controls::banner(result, cx)))
            })
            .child(controls::footnote(
                "Adds Apple's search popularity (0–100) to your keywords. Copy the public key into Apple Ads under Account Settings → API, then enter the clientId, teamId and keyId it shows you. The private key stays in your keychain.",
                cx,
            ))
    }

    fn about_section(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let state = self.state.read(cx);
        let data_dir = state.services.data_dir.clone();
        let (color, update) = match &state.update {
            UpdateStatus::Unknown | UpdateStatus::Checking => (theme.muted_foreground, "Checking…".to_owned()),
            UpdateStatus::Current => (meter::up(), "Up to date".to_owned()),
            UpdateStatus::Available(info) => (theme.primary, format!("{} is available", info.version)),
            UpdateStatus::Failed(_) => (theme.muted_foreground, "Couldn't reach the update server".to_owned()),
        };
        let secondary = |text: String| {
            div()
                .min_w_0()
                .truncate()
                .text_size(px(13.))
                .text_color(theme.muted_foreground)
                .child(text)
        };
        let items = vec![
            controls::row("Version", secondary(env!("CARGO_PKG_VERSION").to_owned()), cx).into_any_element(),
            controls::row(
                "Updates",
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(status_dot(color))
                    .child(secondary(update))
                    .child(
                        Button::new("check-update")
                            .small()
                            .label("Check Now")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.state.update(cx, |state, cx| state.check_for_update(cx))
                            })),
                    ),
                cx,
            )
            .into_any_element(),
            controls::row(
                "Data folder",
                h_flex()
                    .gap_2()
                    .items_center()
                    .min_w_0()
                    .max_w(px(FIELD + 40.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(12.))
                            .font_family(theme.mono_font_family.clone())
                            .text_color(theme.muted_foreground)
                            .child(data_dir.display().to_string()),
                    )
                    .child(
                        Button::new("reveal-data")
                            .small()
                            .label("Show")
                            .on_click(move |_, _, cx| cx.reveal_path(&data_dir.join("appdoctor.db"))),
                    ),
                cx,
            )
            .into_any_element(),
        ];
        Self::section("About", controls::rows(items, cx), cx).child(controls::footnote(
            "Your apps, keywords and rank history are stored in one SQLite file on this computer. AppDoctor sends no analytics.",
            cx,
        ))
    }
}

/// The widest a settings page grows, and the width of its text fields.
const COLUMN: f32 = 640.;
const FIELD: f32 = 300.;

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().id("settings-page").size_full().overflow_y_scroll().child(
            h_flex().w_full().justify_center().child(
                v_flex()
                    .w_full()
                    .max_w(px(COLUMN))
                    .px_6()
                    .pt(px(28.))
                    .pb(px(40.))
                    .gap(px(28.))
                    .child(widgets::page_header(
                        "Settings",
                        "Connections and your account. Secrets are kept in the system keychain.",
                        cx,
                    ))
                    .child(self.account_section(cx))
                    .child(self.asc_section(cx))
                    .child(self.ads_section(cx))
                    .child(self.about_section(cx)),
            ),
        )
    }
}
