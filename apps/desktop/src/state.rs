//! The model every view reads: tracked apps, the selected app and its
//! keywords, the running rank check, account and update status.
//!
//! Views observe this entity and re-render when it notifies. Operations
//! that can fail return a `Task` so the calling view can report the error
//! in its window.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context as _;
use appdoctor_appstore::parse_app_ref;
use appdoctor_entitlement::UpdateInfo;
use appdoctor_store::{AppRecord, KeywordId, KeywordRow};
use futures::StreamExt as _;
use gpui_kit::{AppContext as _, Context, Entity, Task};

use crate::entitlement::{self, Account};
use crate::icons::IconCache;
use crate::rank_check::{self, Outcome, Progress, now};
use crate::services::Services;
use crate::updater;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Apps,
    Audit,
    Keywords,
    Rankings,
    Competitors,
    Localization,
    Screenshots,
    Release,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    Unknown,
    Checking,
    Current,
    /// Newer, but this copy can't install it itself: download it by hand.
    Available(UpdateInfo),
    /// Downloading and installing in the background.
    Installing(UpdateInfo),
    /// Installed; a restart opens the new version.
    Ready(UpdateInfo),
    Failed(String),
}

/// How often a running app looks for a new release, after the check at launch.
const UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(6 * 60 * 60);

pub struct AppState {
    pub services: Arc<Services>,
    pub icons: Entity<IconCache>,
    pub section: Section,
    /// Which design the Screenshots section shows.
    pub screenshot_target: appdoctor_screenshots::Target,
    /// The language of the design open in Screenshots; `None` is the default design.
    pub screenshot_locale: Option<String>,
    pub apps: Vec<AppRecord>,
    pub selected_app: Option<u64>,
    /// The storefront the keyword table shows and new keywords go to.
    pub country: String,
    /// A request for the AI beside Localization, from another screen
    /// (the audit's Fix with AI); Localization takes it and opens the chat.
    pub ai_request: Option<String>,
    /// All keywords of the selected app, every storefront.
    pub keywords: Vec<KeywordRow>,
    keyword_counts: HashMap<u64, usize>,
    /// The storefronts each app is tracked in, the one it was added from first.
    countries: HashMap<u64, Vec<String>>,
    pub check: Option<Progress>,
    pub account: Account,
    pub update: UpdateStatus,
    _check_task: Option<Task<()>>,
}

const LAST_APP: &str = "last_app";

impl AppState {
    pub fn new(services: Arc<Services>, cx: &mut Context<Self>) -> Self {
        let icons = cx.new(|_| IconCache::new(services.data_dir.join("cache").join("icons")));
        cx.observe(&icons, |_, _, cx| cx.notify()).detach();
        Self {
            account: Account::load(&services),
            icons,
            section: Section::Keywords,
            screenshot_target: appdoctor_screenshots::Target::Iphone,
            screenshot_locale: None,
            apps: Vec::new(),
            selected_app: None,
            country: "us".into(),
            ai_request: None,
            keywords: Vec::new(),
            keyword_counts: HashMap::new(),
            countries: HashMap::new(),
            check: None,
            update: UpdateStatus::Unknown,
            services,
            _check_task: None,
        }
    }

    /// Loads data and starts background checks. Called once the window exists.
    pub fn start(&mut self, cx: &mut Context<Self>) {
        self.reload_apps();
        let last = self
            .services
            .store
            .setting(LAST_APP)
            .ok()
            .flatten()
            .and_then(|id| id.parse().ok());
        match last
            .filter(|id| self.app(*id).is_some())
            .or(self.apps.first().map(|app| app.id))
        {
            Some(id) => self.select_app(id, cx),
            None => self.section = Section::Apps,
        }
        self.watch_for_updates(cx);
        if let appdoctor_entitlement::Status::Active {
            needs_refresh: true, ..
        } = self.account.status
        {
            self.refresh_entitlement(cx).detach();
        }
        cx.notify();
    }

    pub fn app(&self, id: u64) -> Option<&AppRecord> {
        self.apps.iter().find(|app| app.id == id)
    }

    pub fn selected(&self) -> Option<&AppRecord> {
        self.selected_app.and_then(|id| self.app(id))
    }

    /// Keywords of the selected app in the current storefront.
    pub fn visible_keywords(&self) -> impl Iterator<Item = &KeywordRow> {
        self.keywords.iter().filter(|row| row.country == self.country)
    }

    pub fn keyword_count(&self, app_id: u64) -> usize {
        self.keyword_counts.get(&app_id).copied().unwrap_or(0)
    }

    /// The storefronts an app is tracked in, the one it was added from first.
    pub fn countries(&self, app_id: u64) -> Vec<String> {
        match self.countries.get(&app_id) {
            Some(countries) if !countries.is_empty() => countries.clone(),
            _ => self.app(app_id).map(|app| vec![app.country.clone()]).unwrap_or_default(),
        }
    }

    pub fn set_section(&mut self, section: Section, cx: &mut Context<Self>) {
        self.section = section;
        cx.notify();
    }

    /// Opens the Screenshots section on one of its designs.
    pub fn open_screenshots(&mut self, target: appdoctor_screenshots::Target, cx: &mut Context<Self>) {
        self.screenshot_target = target;
        self.section = Section::Screenshots;
        cx.notify();
    }

    pub fn set_screenshot_locale(&mut self, locale: Option<String>, cx: &mut Context<Self>) {
        self.screenshot_locale = locale;
        cx.notify();
    }

    pub fn set_country(&mut self, country: &str, cx: &mut Context<Self>) {
        self.country = country.to_ascii_lowercase();
        cx.notify();
    }

    fn reload_apps(&mut self) {
        match self.services.store.apps() {
            Ok(apps) => self.apps = apps,
            Err(error) => tracing::error!(%error, "loading apps"),
        }
        self.countries = self
            .apps
            .iter()
            .map(|app| (app.id, self.services.store.app_countries(app.id).unwrap_or_default()))
            .collect();
        self.keyword_counts = self
            .apps
            .iter()
            .map(|app| {
                (
                    app.id,
                    self.services.store.keywords(app.id).map(|rows| rows.len()).unwrap_or(0),
                )
            })
            .collect();
    }

    pub fn reload_keywords(&mut self, cx: &mut Context<Self>) {
        self.keywords = match self.selected_app {
            Some(id) => self.services.store.keywords(id).unwrap_or_else(|error| {
                tracing::error!(%error, "loading keywords");
                Vec::new()
            }),
            None => Vec::new(),
        };
        if let Some(id) = self.selected_app {
            self.keyword_counts.insert(id, self.keywords.len());
            if let Ok(countries) = self.services.store.app_countries(id) {
                self.countries.insert(id, countries);
            }
        }
        cx.notify();
    }

    pub fn select_app(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.selected_app != Some(id) {
            self.selected_app = Some(id);
            self.screenshot_locale = None;
            if let Some(app) = self.app(id) {
                self.country = app.country.clone();
            }
            let _ = self.services.store.set_setting(LAST_APP, &id.to_string());
            self.reload_keywords(cx);
        }
        if matches!(self.section, Section::Apps | Section::Settings) {
            self.section = Section::Keywords;
        }
        cx.notify();
    }

    /// Opens an app's keywords in one of its storefronts.
    pub fn open_app_in(&mut self, id: u64, country: &str, cx: &mut Context<Self>) {
        self.select_app(id, cx);
        self.set_country(country, cx);
        self.section = Section::Keywords;
        cx.notify();
    }

    /// Stops tracking an app in one storefront (not the one it was added
    /// from), with its keywords there.
    pub fn remove_country(&mut self, id: u64, country: &str, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.services.store.remove_app_country(id, country)?;
        if self.selected_app == Some(id) && self.country == country {
            self.country = self.app(id).map(|app| app.country.clone()).unwrap_or_else(|| "us".into());
        }
        self.reload_apps();
        self.reload_keywords(cx);
        Ok(())
    }

    /// Looks the app up on the App Store and starts tracking it, or, for an
    /// app already tracked, starts tracking it in another storefront too.
    pub fn add_app(
        &mut self,
        input: String,
        country: String,
        cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<AppRecord>> {
        let services = self.services.clone();
        let lookup = crate::runtime::spawn(async move {
            let app_ref = parse_app_ref(&input)?;
            let country = app_ref.country.unwrap_or(country);
            let details = services
                .itunes
                .lookup(app_ref.id, &country)
                .await
                .with_context(|| format!("looking up app {}", app_ref.id))?;
            let now = now();
            let record = AppRecord {
                id: details.id.0,
                name: details.name,
                bundle_id: details.bundle_id,
                developer: details.developer,
                icon_url: details.icon_url,
                country,
                rating_count: details.rating_count,
                average_rating: details.average_rating,
                added_at: now,
                updated_at: now,
            };
            services.store.upsert_app(&record)?;
            Ok(record)
        });
        cx.spawn(async move |this, cx| {
            let record = lookup.await?;
            this.update(cx, |this, cx| {
                this.reload_apps();
                // Open it in the storefront just added, not the first one.
                this.open_app_in(record.id, &record.country, cx);
            })?;
            Ok(record)
        })
    }

    pub fn remove_app(&mut self, id: u64, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.services.store.delete_app(id)?;
        self.reload_apps();
        if self.selected_app == Some(id) {
            self.selected_app = None;
            if let Some(next) = self.apps.first().map(|app| app.id) {
                self.select_app(next, cx);
            } else {
                self.keywords.clear();
            }
        }
        cx.notify();
        Ok(())
    }

    /// Adds comma- or newline-separated keywords to the selected app in the
    /// current storefront. Returns how many were new.
    pub fn add_keywords(&mut self, text: &str, cx: &mut Context<Self>) -> anyhow::Result<usize> {
        let app_id = self.selected_app.context("add an app first")?;
        let before = self.keywords.len();
        let now = now();
        for term in text
            .split([',', '\n', ';'])
            .map(str::trim)
            .filter(|term| !term.is_empty())
        {
            self.services.store.add_keyword(app_id, term, &self.country, now)?;
        }
        self.reload_keywords(cx);
        Ok(self.keywords.len() - before)
    }

    pub fn delete_keyword(&mut self, id: KeywordId, cx: &mut Context<Self>) -> anyhow::Result<()> {
        self.services.store.delete_keyword(id)?;
        self.reload_keywords(cx);
        Ok(())
    }

    /// Checks every keyword of the selected app, in every storefront.
    pub fn check_ranks(&mut self, cx: &mut Context<Self>) -> Task<anyhow::Result<Outcome>> {
        if self.check.is_some() {
            return Task::ready(Err(anyhow::anyhow!("a rank check is already running")));
        }
        if !self.account.is_entitled() {
            return Task::ready(Err(anyhow::anyhow!(
                "Checking ranks needs an active AppDoctor subscription. Sign in under Settings."
            )));
        }
        let Some(app_id) = self.selected_app else {
            return Task::ready(Err(anyhow::anyhow!("add an app first")));
        };
        let keywords = self.keywords.clone();
        if keywords.is_empty() {
            return Task::ready(Err(anyhow::anyhow!("add some keywords first")));
        }

        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        let job = crate::runtime::spawn(rank_check::run(self.services.clone(), app_id, keywords, sender));
        self.check = Some(Progress {
            done: 0,
            total: self.keywords.len(),
            failed: 0,
        });
        // Progress arrives on its own task so the table refreshes as
        // results land, not only at the end.
        self._check_task = Some(cx.spawn(async move |this, cx| {
            while let Some(progress) = receiver.next().await {
                let reload = progress.done % 5 == 0;
                if this
                    .update(cx, |this, cx| {
                        this.check = Some(progress);
                        if reload {
                            this.reload_keywords(cx);
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        cx.spawn(async move |this, cx| {
            let outcome = job.await;
            this.update(cx, |this, cx| {
                this.check = None;
                this._check_task = None;
                this.reload_keywords(cx);
            })?;
            outcome
        })
    }

    pub fn refresh_entitlement(&mut self, cx: &mut Context<Self>) -> Task<anyhow::Result<()>> {
        let services = self.services.clone();
        let refresh = crate::runtime::spawn(entitlement::refresh(services));
        cx.spawn(async move |this, cx| {
            let result = refresh.await;
            this.update(cx, |this, cx| {
                match &result {
                    Ok(status) => {
                        this.account.status = status.clone();
                        this.account.error = None;
                    }
                    Err(error) => this.account.error = Some(error.to_string()),
                }
                cx.notify();
            })?;
            result.map(|_| ())
        })
    }

    /// Looks for a new release now, then every few hours while the app runs.
    fn watch_for_updates(&mut self, cx: &mut Context<Self>) {
        self.check_for_update(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(UPDATE_INTERVAL).await;
                if this.update(cx, |this, cx| this.check_for_update(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    /// Checks for a new release. An official Mac build installs it straight
    /// away; anything else announces it.
    pub fn check_for_update(&mut self, cx: &mut Context<Self>) {
        if matches!(self.update, UpdateStatus::Installing(_) | UpdateStatus::Ready(_)) {
            return;
        }
        self.update = UpdateStatus::Checking;
        let check = crate::runtime::spawn(updater::check());
        cx.spawn(async move |this, cx| {
            let status = match check.await {
                Ok(Some(info)) => UpdateStatus::Available(info),
                Ok(None) => UpdateStatus::Current,
                Err(error) => UpdateStatus::Failed(error.to_string()),
            };
            this.update(cx, |this, cx| {
                this.update = status;
                if let UpdateStatus::Available(info) = &this.update
                    && let Some(bundle) = updater::installable_bundle()
                {
                    this.install_update(info.clone(), bundle, cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn install_update(&mut self, info: UpdateInfo, bundle: std::path::PathBuf, cx: &mut Context<Self>) {
        self.update = UpdateStatus::Installing(info.clone());
        let install = crate::runtime::spawn(updater::install(info.clone(), bundle));
        cx.spawn(async move |this, cx| {
            let result = install.await;
            this.update(cx, |this, cx| {
                this.update = match result {
                    Ok(()) => UpdateStatus::Ready(info),
                    Err(error) => {
                        // Fall back to a manual download rather than retrying
                        // something that may keep failing (a read-only folder).
                        tracing::warn!("couldn't install update {}: {error:#}", info.version);
                        UpdateStatus::Available(info)
                    }
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Quits and opens the installed update.
    pub fn restart_to_update(cx: &mut gpui_kit::App) {
        let Some(bundle) = updater::installable_bundle() else { return };
        match updater::relaunch(&bundle) {
            Ok(()) => cx.quit(),
            Err(error) => tracing::warn!("couldn't relaunch: {error:#}"),
        }
    }
}
