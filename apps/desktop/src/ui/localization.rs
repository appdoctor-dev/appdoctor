//! Localization: the App Store metadata for every language, edited in
//! place and pushed to App Store Connect after a confirmation that lists
//! exactly what changes.
//!
//! Each pull is saved to the database, so AI agents can read the listing
//! (AppDoctor's `get_listing` tool). Their suggestions (`suggest_listing`)
//! show here, per language, to review in the fields and save the usual way.
//!
//! Each language keeps its earlier versions: what was pulled and saved, edits
//! about to be replaced, and versions the user named. Restoring one fills the
//! fields; only Review and Save changes App Store Connect.
//!
//! Name and subtitle live on the app info; keywords, promotional text,
//! description and what's new live on the App Store version. Each is
//! editable only while its parent is (a version being prepared, say).

use std::collections::HashSet;
use std::sync::Arc;

use appdoctor_asc::{
    AppInfoLocalization, AppInfoLocalizationUpdate, Platform, VersionLocalization, VersionLocalizationUpdate,
};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, WindowExt as _, h_flex, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use appdoctor_store::{LOCALES, Listing, ListingDraft, ListingText, ListingVersion, locale_name};

use crate::rank_check::now;
use crate::runtime;
use crate::services::Services;
use crate::state::{AppState, Section};
use crate::theme::meter;
use crate::ui::controls;
use crate::ui::versions;
use crate::ui::widgets::{self, pill, status_dot};

const NAME_LIMIT: usize = 30;
const SUBTITLE_LIMIT: usize = 30;
const KEYWORDS_LIMIT: usize = 100;
const PROMO_LIMIT: usize = 170;
const DESCRIPTION_LIMIT: usize = 4000;
const WHATS_NEW_LIMIT: usize = 4000;
const HISTORY_PANEL: f32 = 320.;
/// Typing in the fields becomes a version once it pauses this long.
const SETTLE: std::time::Duration = std::time::Duration::from_secs(30);

#[derive(Clone)]
struct LocaleItem {
    code: &'static str,
    title: SharedString,
}

impl SelectItem for LocaleItem {
    type Value = &'static str;
    fn title(&self) -> SharedString {
        self.title.clone()
    }
    fn value(&self) -> &Self::Value {
        &self.code
    }
}

#[derive(Clone)]
struct Locale {
    code: String,
    info: Option<AppInfoLocalization>,
    version: Option<VersionLocalization>,
}

#[derive(Clone)]
struct Loaded {
    app_id: u64,
    app_info_id: Option<String>,
    info_editable: bool,
    version_id: Option<String>,
    version_label: String,
    version_editable: bool,
    locales: Vec<Locale>,
}

struct Fields {
    name: Entity<InputState>,
    subtitle: Entity<InputState>,
    keywords: Entity<InputState>,
    promo: Entity<InputState>,
    description: Entity<TextareaState>,
    whats_new: Entity<TextareaState>,
}

/// What the save would change, for the confirmation.
#[derive(Clone, Default)]
struct Changes {
    info: AppInfoLocalizationUpdate,
    version: VersionLocalizationUpdate,
    lines: Vec<(String, String)>,
}

pub struct LocalizationView {
    state: Entity<AppState>,
    loaded: Option<Loaded>,
    /// The app last loaded automatically, so it happens once.
    auto_loaded: Option<u64>,
    /// AI suggestions waiting, by language.
    drafts: Vec<ListingDraft>,
    /// The suggestion now in the fields, cleared once saved.
    reviewing: Option<String>,
    ai: Entity<crate::ui::screenshots::AiPanel>,
    ai_open: bool,
    history_open: bool,
    /// The shown language's earlier versions, newest first.
    versions: Vec<ListingVersion>,
    /// The version showing what it would change.
    version_open: Option<i64>,
    version_name: Entity<InputState>,
    /// When the fields last changed, until that's been saved as a version.
    edited_at: Option<std::time::Instant>,
    loading: bool,
    saving: bool,
    selected: Option<String>,
    fields: Fields,
    add_locale: Entity<SelectState<SearchableVec<LocaleItem>>>,
    status: Option<Result<String, String>>,
    _subscriptions: Vec<Subscription>,
}

impl LocalizationView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let line = |placeholder: &str, window: &mut Window, cx: &mut Context<Self>| {
            let placeholder = placeholder.to_owned();
            cx.new(|cx| InputState::new(window, cx).placeholder(placeholder))
        };
        let area = |rows: usize, window: &mut Window, cx: &mut Context<Self>| {
            cx.new(|cx| TextareaState::new(window, cx).auto_grow(rows, 18))
        };
        let fields = Fields {
            name: line("App name", window, cx),
            subtitle: line("Subtitle", window, cx),
            keywords: line("keyword,another keyword,more", window, cx),
            promo: line("Promotional text", window, cx),
            description: area(6, window, cx),
            whats_new: area(3, window, cx),
        };
        let items: Vec<LocaleItem> = LOCALES
            .iter()
            .map(|(code, name)| LocaleItem {
                code,
                title: format!("{name} ({code})").into(),
            })
            .collect();
        let add_locale = cx.new(|cx| SelectState::new(SearchableVec::new(items), None, window, cx).searchable(true));
        let version_name = line("Name this version", window, cx);

        let mut subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                if this.loaded.as_ref().map(|l| l.app_id) != state.read(cx).selected_app {
                    this.loaded = None;
                    this.selected = None;
                    this.status = None;
                    this.reviewing = None;
                    this.drafts.clear();
                    this.versions.clear();
                    this.version_open = None;
                    this.check_drafts(cx);
                }
                cx.notify();
            }),
            cx.subscribe_in(&add_locale, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(code)) = event {
                    this.add_locale(code, window, cx);
                }
            }),
            cx.subscribe(&version_name, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.name_version(cx);
                }
            }),
        ];
        for input in [&fields.name, &fields.subtitle, &fields.keywords, &fields.promo] {
            subscriptions.push(cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited_at = Some(std::time::Instant::now());
                    cx.notify();
                }
            }));
        }
        for area in [&fields.description, &fields.whats_new] {
            subscriptions.push(cx.subscribe(area, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.edited_at = Some(std::time::Instant::now());
                    cx.notify();
                }
            }));
        }

        let ai = {
            let state = state.clone();
            cx.new(|cx| crate::ui::screenshots::AiPanel::listing(state, window, cx))
        };
        // Suggestions arrive from an agent's process; look for them often.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_millis(1200)).await;
                let alive = this.update(cx, |this, cx| {
                    this.check_drafts(cx);
                    this.autosave_edits(cx);
                });
                if alive.is_err() {
                    return;
                }
            }
        })
        .detach();

        Self {
            state,
            loaded: None,
            auto_loaded: None,
            drafts: Vec::new(),
            reviewing: None,
            ai,
            ai_open: false,
            history_open: false,
            versions: Vec::new(),
            version_open: None,
            version_name,
            edited_at: None,
            loading: false,
            saving: false,
            selected: None,
            fields,
            add_locale,
            status: None,
            _subscriptions: subscriptions,
        }
    }

    fn services(&self, cx: &App) -> Arc<Services> {
        self.state.read(cx).services.clone()
    }

    /// Picks up suggestions an agent made (or withdrew) since last time.
    fn check_drafts(&mut self, cx: &mut Context<Self>) {
        let Some(app_id) = self.state.read(cx).selected_app else {
            return;
        };
        let drafts = self.services(cx).store.drafts(app_id).unwrap_or_default();
        if drafts != self.drafts {
            self.drafts = drafts;
            cx.notify();
        }
    }

    fn draft(&self, code: &str) -> Option<&ListingDraft> {
        self.drafts.iter().find(|d| d.locale == code)
    }

    /// Saves what App Store Connect has, for agents to read and as a version
    /// of each language it's new for.
    fn remember(&self, loaded: &Loaded, label: &str, cx: &App) {
        let pulled_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        let listings: Vec<Listing> = loaded
            .locales
            .iter()
            .map(|locale| Listing {
                locale: locale.code.clone(),
                text: ListingText {
                    name: locale.info.as_ref().and_then(|i| i.name.clone()),
                    subtitle: locale.info.as_ref().and_then(|i| i.subtitle.clone()),
                    keywords: locale.version.as_ref().and_then(|v| v.keywords.clone()),
                    promotional_text: locale.version.as_ref().and_then(|v| v.promotional_text.clone()),
                    description: locale.version.as_ref().and_then(|v| v.description.clone()),
                    whats_new: locale.version.as_ref().and_then(|v| v.whats_new.clone()),
                },
                version: loaded.version_label.clone(),
                info_editable: loaded.info_editable,
                version_editable: loaded.version_editable,
                pulled_at,
            })
            .collect();
        let services = self.services(cx);
        if let Err(error) = services.store.replace_listings(loaded.app_id, &listings) {
            tracing::error!(%error, "saving the pulled listing");
        }
        for listing in listings {
            let text = without_empties(listing.text);
            if let Err(error) = services
                .store
                .save_listing_version(loaded.app_id, &listing.locale, &text, label, false, pulled_at)
            {
                tracing::error!(%error, "saving a version of the listing");
            }
        }
    }

    /// What the fields hold now.
    fn current_text(&self, cx: &App) -> ListingText {
        without_empties(ListingText {
            name: Some(Self::value(&self.fields.name, cx)),
            subtitle: Some(Self::value(&self.fields.subtitle, cx)),
            keywords: Some(Self::value(&self.fields.keywords, cx)),
            promotional_text: Some(Self::value(&self.fields.promo, cx)),
            description: Some(Self::area_value(&self.fields.description, cx)),
            whats_new: Some(Self::area_value(&self.fields.whats_new, cx)),
        })
    }

    fn refresh_versions(&mut self, cx: &mut Context<Self>) {
        let app_id = self.state.read(cx).selected_app;
        self.versions = match (app_id, &self.selected) {
            (Some(app_id), Some(code)) => self.services(cx).store.listing_versions(app_id, code).unwrap_or_default(),
            _ => Vec::new(),
        };
        cx.notify();
    }

    /// Keeps what's in the fields as a version, if it isn't saved anywhere,
    /// before something replaces it.
    fn keep_unsaved(&mut self, cx: &mut Context<Self>) {
        let (Some(app_id), Some(code), Some(changes)) =
            (self.state.read(cx).selected_app, self.selected.clone(), self.changes(cx))
        else {
            return;
        };
        if changes.lines.is_empty() {
            return;
        }
        let saved = self
            .services(cx)
            .store
            .save_listing_version(app_id, &code, &self.current_text(cx), "Unsaved edits", false, now());
        if let Err(error) = saved {
            tracing::error!(%error, "saving a version of the listing");
        }
        self.refresh_versions(cx);
    }

    /// Run often: typing that's paused becomes a version, so it survives
    /// quitting before it's saved to App Store Connect.
    fn autosave_edits(&mut self, cx: &mut Context<Self>) {
        if self.edited_at.is_some_and(|at| at.elapsed() >= SETTLE) {
            self.edited_at = None;
            self.keep_unsaved(cx);
        }
    }

    /// Keeps what's in the fields under the name typed for it.
    fn name_version(&mut self, cx: &mut Context<Self>) {
        let name = self.version_name.read(cx).value().trim().to_owned();
        let (Some(app_id), Some(code)) = (self.state.read(cx).selected_app, self.selected.clone()) else {
            return;
        };
        if name.is_empty() {
            self.status = Some(Err("Type a name for the version first.".into()));
            cx.notify();
            return;
        }
        let saved = self
            .services(cx)
            .store
            .save_listing_version(app_id, &code, &self.current_text(cx), &name, true, now());
        self.status = Some(match saved {
            Ok(_) => Ok(format!("Saved “{name}”. App Store Connect hasn't changed.")),
            Err(error) => Err(format!("Couldn't save the version: {error}")),
        });
        self.refresh_versions(cx);
    }

    /// Puts a version's text in the fields that can change now, to look over
    /// and save the usual way. What was there stays in the history.
    fn restore_version(&mut self, id: i64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(version) = self.versions.iter().find(|v| v.id == id).cloned() else {
            return;
        };
        let Some((info_on, version_on)) = self.loaded.as_ref().map(|l| (l.info_editable, l.version_editable)) else {
            return;
        };
        self.keep_unsaved(cx);
        let t = version.text;
        let set = |state: &Entity<InputState>, value: Option<String>, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |input, cx| input.set_value(value.unwrap_or_default(), window, cx));
        };
        if info_on {
            set(&self.fields.name, t.name, window, cx);
            set(&self.fields.subtitle, t.subtitle, window, cx);
        }
        if version_on {
            set(&self.fields.keywords, t.keywords, window, cx);
            set(&self.fields.promo, t.promotional_text, window, cx);
            let (description, whats_new) = (t.description.unwrap_or_default(), t.whats_new.unwrap_or_default());
            self.fields.description.update(cx, |area, cx| area.set_value(description, window, cx));
            self.fields.whats_new.update(cx, |area, cx| area.set_value(whats_new, window, cx));
        }
        self.reviewing = None;
        self.version_open = None;
        self.status = Some(Ok(match (info_on, version_on) {
            (true, true) => "Restored in the fields: edit anything, then Review and Save.".into(),
            (true, false) => "Restored the name and subtitle; the rest is read-only until the next version. Review and Save to keep them.".into(),
            _ => "Restored all but the name and subtitle, which are read-only now. Review and Save to keep it.".to_owned(),
        }));
        cx.notify();
    }

    fn delete_version(&mut self, id: i64, cx: &mut Context<Self>) {
        if let Err(error) = self.services(cx).store.delete_listing_version(id) {
            self.status = Some(Err(format!("Couldn't delete the version: {error}")));
        }
        self.refresh_versions(cx);
    }

    fn history_panel(&self, code: &str, can_edit: bool, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme().clone();
        let current = self.current_text(cx);
        let top = {
            let close = cx.listener(|this, _, _, cx| {
                this.history_open = false;
                cx.notify();
            });
            let save = cx.listener(|this, _, _, cx| this.name_version(cx));
            versions::panel_top(format!("{}, on this computer", locale_name(code)), &self.version_name, close, save, cx)
        };
        let rows: Vec<_> = self
            .versions
            .iter()
            .map(|version| {
                let id = version.id;
                let selected = self.version_open == Some(id);
                let differences = differences(&version.text, &current);
                let title = versions::title(&version.label);
                let detail = format!(
                    "{} · {}",
                    versions::when(version.created_at),
                    match differences.len() {
                        0 => "same as the fields".to_owned(),
                        1 => format!("{} differs", differences[0].0),
                        count => format!("{count} fields differ"),
                    }
                );
                let restorable = can_edit && !differences.is_empty();
                versions::row(("listing-version", id as u64), selected, cx)
                    .child(versions::heading(title.clone(), version.named, detail, cx))
                    .when(selected, |this| {
                        let restore =
                            restorable.then(|| cx.listener(move |this, _, window, cx| this.restore_version(id, window, cx)));
                        let view = cx.entity().downgrade();
                        this.when(!differences.is_empty(), |this| this.child(controls::summary(differences, cx)))
                            .child(versions::actions(restore, move |_, window, cx| {
                                let view = view.clone();
                                versions::confirm_delete(title.clone(), window, cx, move |_, cx| {
                                    view.update(cx, |this, cx| this.delete_version(id, cx)).ok();
                                });
                            }))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.version_open = if this.version_open == Some(id) { None } else { Some(id) };
                        cx.notify();
                    }))
            })
            .collect();
        let empty = rows.is_empty();
        v_flex()
            .w(px(HISTORY_PANEL))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_l_1()
            .border_color(theme.border)
            .child(top)
            .child(
                v_flex()
                    .id("listing-versions")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p(px(8.))
                    .gap(px(2.))
                    .children(rows)
                    .when(empty, |this| {
                        this.child(
                            div()
                                .px(px(9.))
                                .py(px(10.))
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child("Earlier versions of this language appear here."),
                        )
                    }),
            )
    }

    /// Puts a suggestion's fields in the editor, to look over and save.
    fn review(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(draft) = self.draft(code).cloned() else {
            return;
        };
        self.keep_unsaved(cx);
        if self.selected.as_deref() != Some(code) {
            self.select(code, window, cx);
        }
        let t = draft.text;
        let set = |state: &Entity<InputState>, value: Option<String>, window: &mut Window, cx: &mut Context<Self>| {
            if let Some(value) = value {
                state.update(cx, |input, cx| input.set_value(value, window, cx));
            }
        };
        set(&self.fields.name, t.name, window, cx);
        set(&self.fields.subtitle, t.subtitle, window, cx);
        set(&self.fields.keywords, t.keywords, window, cx);
        set(&self.fields.promo, t.promotional_text, window, cx);
        if let Some(value) = t.description {
            self.fields.description.update(cx, |area, cx| area.set_value(value, window, cx));
        }
        if let Some(value) = t.whats_new {
            self.fields.whats_new.update(cx, |area, cx| area.set_value(value, window, cx));
        }
        self.reviewing = Some(code.to_owned());
        self.status = Some(Ok("Suggestion in the fields: edit anything, then Review and Save.".into()));
        cx.notify();
    }

    fn dismiss_draft(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(app_id) = self.state.read(cx).selected_app {
            let _ = self.services(cx).store.delete_draft(app_id, code);
        }
        self.drafts.retain(|d| d.locale != code);
        if self.reviewing.as_deref() == Some(code) {
            self.reviewing = None;
            // Back to what App Store Connect has.
            self.select(code, window, cx);
        }
        cx.notify();
    }

    /// The suggestion for the shown language, or for a language the
    /// listing doesn't have yet.
    fn suggestion_banner(&self, cx: &mut Context<Self>) -> Option<Div> {
        let theme = cx.theme().clone();
        let loaded = self.loaded.as_ref()?;
        let selected = self.selected.clone();
        let (draft, missing) = match selected.as_deref().and_then(|code| self.draft(code)) {
            Some(draft) => (draft.clone(), false),
            None => (
                self.drafts
                    .iter()
                    .find(|d| !loaded.locales.iter().any(|l| l.code == d.locale))?
                    .clone(),
                true,
            ),
        };
        let code = draft.locale.clone();
        let changed: Vec<&str> = draft.text.fields().iter().filter(|f| f.3.is_some()).map(|f| f.1).collect();
        let reviewing = self.reviewing.as_deref() == Some(code.as_str());
        let title = if missing {
            format!("Your AI wrote a {} listing", locale_name(&code))
        } else if reviewing {
            "Your AI's suggestion is in the fields".to_owned()
        } else {
            format!("Your AI suggests a new {}", changed.join(", ").to_lowercase())
        };
        let (review_code, dismiss_code, add_code) = (code.clone(), code.clone(), code.clone());
        Some(
            h_flex()
                .gap(px(10.))
                .items_start()
                .px(px(14.))
                .py(px(11.))
                .rounded(px(10.))
                .bg(theme.primary.opacity(0.09))
                .border_1()
                .border_color(theme.primary.opacity(0.25))
                .child(
                    Icon::new(gpui_kit::assets::IconName::Sparkles)
                        .size(px(15.))
                        .mt(px(1.))
                        .text_color(theme.primary),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(px(3.))
                        .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(title))
                        .when(!draft.note.is_empty(), |this| {
                            this.child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(draft.note.clone()))
                        })
                        .when(missing, |this| {
                            this.child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(format!(
                                "Add {} to App Store Connect first, then review it here.",
                                locale_name(&code)
                            )))
                        }),
                )
                .child(
                    h_flex()
                        .gap(px(6.))
                        .flex_shrink_0()
                        .child(
                            Button::new("draft-dismiss")
                                .ghost()
                                .small()
                                .label("Dismiss")
                                .on_click(cx.listener(move |this, _, window, cx| this.dismiss_draft(&dismiss_code, window, cx))),
                        )
                        .when(missing, |this| {
                            this.child(
                                Button::new("draft-add-language")
                                    .primary()
                                    .small()
                                    .label(format!("Add {}", locale_name(&add_code)))
                                    .loading(self.loading)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if let Some((code, _)) = LOCALES.iter().find(|(c, _)| *c == add_code) {
                                            this.add_locale(code, window, cx);
                                        }
                                    })),
                            )
                        })
                        .when(!missing && !reviewing, |this| {
                            this.child(
                                Button::new("draft-review")
                                    .primary()
                                    .small()
                                    .label("Review")
                                    .on_click(cx.listener(move |this, _, window, cx| this.review(&review_code, window, cx))),
                            )
                        }),
                ),
        )
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(app_id) = self.state.read(cx).selected_app else {
            return;
        };
        let services = self.services(cx);
        self.loading = true;
        self.status = None;
        cx.notify();
        let fetch = runtime::spawn(async move {
            let client = services.asc_client()?;
            let id = app_id.to_string();
            let infos = client.app_infos(&id).await?;
            let info = infos.iter().find(|i| i.is_editable()).or(infos.first()).cloned();
            let versions: Vec<_> = client
                .app_store_versions(&id)
                .await?
                .into_iter()
                .filter(|v| v.platform == Some(Platform::Ios))
                .collect();
            let version = versions.iter().find(|v| v.is_editable()).or(versions.first()).cloned();

            let info_locs = match &info {
                Some(info) => client.app_info_localizations(&info.id).await?,
                None => Vec::new(),
            };
            let version_locs = match &version {
                Some(version) => client.version_localizations(&version.id).await?,
                None => Vec::new(),
            };
            let mut codes: Vec<String> = info_locs.iter().map(|l| l.locale.clone()).collect();
            for loc in &version_locs {
                if !codes.contains(&loc.locale) {
                    codes.push(loc.locale.clone());
                }
            }
            codes.sort_by_key(|code| locale_name(code));
            let locales = codes
                .into_iter()
                .map(|code| Locale {
                    info: info_locs.iter().find(|l| l.locale == code).cloned(),
                    version: version_locs.iter().find(|l| l.locale == code).cloned(),
                    code,
                })
                .collect();
            Ok(Loaded {
                app_id,
                info_editable: info.as_ref().is_some_and(|i| i.is_editable()),
                app_info_id: info.map(|i| i.id),
                version_editable: version.as_ref().is_some_and(|v| v.is_editable()),
                version_label: version
                    .as_ref()
                    .map(|v| {
                        format!(
                            "{} ({})",
                            v.version.clone().unwrap_or_default(),
                            v.state.clone().unwrap_or_default()
                        )
                    })
                    .unwrap_or_default(),
                version_id: version.map(|v| v.id),
                locales,
            })
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = fetch.await;
            this.update_in(cx, |this, window, cx| {
                this.loading = false;
                match result {
                    Ok(loaded) => {
                        // Edits are judged against what was loaded before this.
                        this.keep_unsaved(cx);
                        this.remember(&loaded, "From App Store Connect", cx);
                        let keep = this
                            .selected
                            .clone()
                            .filter(|code| loaded.locales.iter().any(|l| &l.code == code));
                        let first = loaded.locales.first().map(|l| l.code.clone());
                        this.loaded = Some(loaded);
                        if let Some(code) = keep.or(first) {
                            this.show_locale(&code, window, cx);
                        }
                    }
                    Err(error) => this.status = Some(Err(format!("{error:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn locale(&self, code: &str) -> Option<&Locale> {
        self.loaded.as_ref()?.locales.iter().find(|l| l.code == code)
    }

    /// Switches language, keeping any unsaved edits to the last one.
    fn select(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.keep_unsaved(cx);
        self.show_locale(code, window, cx);
    }

    /// Fills the fields with a language as App Store Connect has it.
    fn show_locale(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(locale) = self.locale(code).cloned() else {
            return;
        };
        self.selected = Some(code.to_owned());
        self.reviewing = None;
        self.version_open = None;
        self.refresh_versions(cx);
        let info = locale.info.unwrap_or(AppInfoLocalization {
            id: String::new(),
            locale: code.to_owned(),
            name: None,
            subtitle: None,
        });
        let version = locale.version;
        let text = |value: Option<String>| value.unwrap_or_default();
        let set = |state: &Entity<InputState>, value: String, window: &mut Window, cx: &mut Context<Self>| {
            state.update(cx, |input, cx| input.set_value(value, window, cx));
        };
        set(&self.fields.name, text(info.name), window, cx);
        set(&self.fields.subtitle, text(info.subtitle), window, cx);
        set(
            &self.fields.keywords,
            text(version.as_ref().and_then(|v| v.keywords.clone())),
            window,
            cx,
        );
        set(
            &self.fields.promo,
            text(version.as_ref().and_then(|v| v.promotional_text.clone())),
            window,
            cx,
        );
        let description = text(version.as_ref().and_then(|v| v.description.clone()));
        let whats_new = text(version.as_ref().and_then(|v| v.whats_new.clone()));
        self.fields
            .description
            .update(cx, |area, cx| area.set_value(description, window, cx));
        self.fields
            .whats_new
            .update(cx, |area, cx| area.set_value(whats_new, window, cx));
        self.status = None;
        cx.notify();
    }

    fn value(state: &Entity<InputState>, cx: &App) -> String {
        state.read(cx).value().to_string()
    }

    fn area_value(state: &Entity<TextareaState>, cx: &App) -> String {
        state.read(cx).value().to_string()
    }

    fn changes(&self, cx: &App) -> Option<Changes> {
        let locale = self.locale(self.selected.as_deref()?)?;
        let mut changes = Changes::default();
        let diff = |label: &str, before: Option<&String>, after: String, lines: &mut Vec<(String, String)>| {
            let before = before.cloned().unwrap_or_default();
            (before != after).then(|| {
                lines.push((label.to_owned(), preview(&after)));
                after
            })
        };
        let info = locale.info.as_ref();
        let version = locale.version.as_ref();
        changes.info.name = diff(
            "Name",
            info.and_then(|i| i.name.as_ref()),
            Self::value(&self.fields.name, cx),
            &mut changes.lines,
        );
        changes.info.subtitle = diff(
            "Subtitle",
            info.and_then(|i| i.subtitle.as_ref()),
            Self::value(&self.fields.subtitle, cx),
            &mut changes.lines,
        );
        changes.version.keywords = diff(
            "Keywords",
            version.and_then(|v| v.keywords.as_ref()),
            Self::value(&self.fields.keywords, cx),
            &mut changes.lines,
        );
        changes.version.promotional_text = diff(
            "Promotional text",
            version.and_then(|v| v.promotional_text.as_ref()),
            Self::value(&self.fields.promo, cx),
            &mut changes.lines,
        );
        changes.version.description = diff(
            "Description",
            version.and_then(|v| v.description.as_ref()),
            Self::area_value(&self.fields.description, cx),
            &mut changes.lines,
        );
        changes.version.whats_new = diff(
            "What's New",
            version.and_then(|v| v.whats_new.as_ref()),
            Self::area_value(&self.fields.whats_new, cx),
            &mut changes.lines,
        );
        Some(changes)
    }

    fn confirm_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(changes) = self.changes(cx) else {
            return;
        };
        if changes.lines.is_empty() {
            self.status = Some(Ok("Nothing to save.".into()));
            cx.notify();
            return;
        }
        let locale = self.selected.clone().unwrap_or_default();
        let view = cx.entity().downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let theme = cx.theme();
            let (view, save) = (view.clone(), changes.clone());
            dialog
                .title(format!("Save {} to App Store Connect?", locale_name(&locale)))
                .w(px(540.))
                .child(
                    v_flex()
                        .gap(px(12.))
                        .child(div().text_size(px(13.)).text_color(theme.muted_foreground).child(
                            "These fields change on App Store Connect. Apple shows them with the next release (promotional text, right away).",
                        ))
                        .child(controls::summary(changes.lines.clone(), cx)),
                )
                .footer(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .gap_2()
                        .child(Button::new("cancel-save").ghost().label("Cancel").on_click(|_, window, cx| window.close_dialog(cx)))
                        .child(Button::new("confirm-save").primary().label("Save to App Store Connect").on_click(
                            move |_, window, cx| {
                                window.close_dialog(cx);
                                let changes = save.clone();
                                view.update(cx, |this, cx| this.save(changes, cx)).ok();
                            },
                        )),
                )
        });
    }

    fn save(&mut self, changes: Changes, cx: &mut Context<Self>) {
        let Some(locale) = self.selected.as_deref().and_then(|code| self.locale(code)).cloned() else {
            return;
        };
        let services = self.services(cx);
        self.saving = true;
        cx.notify();
        let has_info = changes.info.name.is_some() || changes.info.subtitle.is_some();
        let has_version = changes.version.keywords.is_some()
            || changes.version.promotional_text.is_some()
            || changes.version.description.is_some()
            || changes.version.whats_new.is_some();
        let save = runtime::spawn(async move {
            let client = services.asc_client()?;
            if has_info {
                let info = locale
                    .info
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("this language has no app info to save name and subtitle to"))?;
                client.update_app_info_localization(&info.id, &changes.info).await?;
            }
            if has_version {
                let version = locale
                    .version
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("this language has no version metadata to save to"))?;
                client
                    .update_version_localization(&version.id, &changes.version)
                    .await?;
            }
            Ok(changes.lines.len())
        });
        cx.spawn(async move |this, cx| {
            let result = save.await;
            let saved = result.is_ok();
            this.update(cx, |this, cx| {
                this.saving = false;
                if result.is_ok()
                    && let (Some(code), Some(app_id)) = (this.reviewing.take(), this.state.read(cx).selected_app)
                {
                    let _ = this.services(cx).store.delete_draft(app_id, &code);
                    this.drafts.retain(|d| d.locale != code);
                }
                this.status = Some(match result {
                    Ok(count) => Ok(format!(
                        "Saved {count} field{} to App Store Connect.",
                        if count == 1 { "" } else { "s" }
                    )),
                    Err(error) => Err(format!("{error:#}")),
                });
                cx.notify();
            })
            .ok();
            // The editor's baseline is what Apple now has.
            if saved {
                this.update(cx, |this, cx| this.reload_after_save(cx)).ok();
            }
        })
        .detach();
    }

    fn reload_after_save(&mut self, cx: &mut Context<Self>) {
        self.loaded = self.loaded.take().map(|mut loaded| {
            if let Some(code) = &self.selected
                && let Some(locale) = loaded.locales.iter_mut().find(|l| &l.code == code)
            {
                let value = |state: &Entity<InputState>| Some(state.read(cx).value().to_string());
                if let Some(info) = &mut locale.info {
                    info.name = value(&self.fields.name);
                    info.subtitle = value(&self.fields.subtitle);
                }
                if let Some(version) = &mut locale.version {
                    version.keywords = value(&self.fields.keywords);
                    version.promotional_text = value(&self.fields.promo);
                    version.description = Some(self.fields.description.read(cx).value().to_string());
                    version.whats_new = Some(self.fields.whats_new.read(cx).value().to_string());
                }
            }
            loaded
        });
        if let Some(loaded) = &self.loaded {
            self.remember(loaded, "Saved to App Store Connect", cx);
        }
        self.refresh_versions(cx);
        cx.notify();
    }

    fn add_locale(&mut self, code: &'static str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(loaded) = self.loaded.clone() else {
            return;
        };
        if loaded.locales.iter().any(|l| l.code.eq_ignore_ascii_case(code)) {
            self.select(code, window, cx);
            return;
        }
        let name = Self::value(&self.fields.name, cx);
        let services = self.services(cx);
        self.saving = true;
        cx.notify();
        let create = runtime::spawn(async move {
            let client = services.asc_client()?;
            let info = match (&loaded.app_info_id, loaded.info_editable) {
                (Some(id), true) => Some(
                    client
                        .create_app_info_localization(
                            id,
                            code,
                            &AppInfoLocalizationUpdate {
                                name: Some(name),
                                subtitle: None,
                            },
                        )
                        .await?,
                ),
                _ => None,
            };
            let version = match (&loaded.version_id, loaded.version_editable) {
                (Some(id), true) => Some(
                    client
                        .create_version_localization(id, code, &Default::default())
                        .await?,
                ),
                _ => None,
            };
            if info.is_none() && version.is_none() {
                anyhow::bail!("there's no editable app info or version to add a language to");
            }
            Ok(Locale {
                code: code.to_owned(),
                info,
                version,
            })
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = create.await;
            this.update_in(cx, |this, window, cx| {
                this.saving = false;
                match result {
                    Ok(locale) => {
                        let code = locale.code.clone();
                        if let Some(loaded) = &mut this.loaded {
                            loaded.locales.push(locale);
                            loaded.locales.sort_by_key(|l| locale_name(&l.code));
                        }
                        this.select(&code, window, cx);
                        this.status = Some(Ok(format!("Added {}.", locale_name(&code))));
                    }
                    Err(error) => this.status = Some(Err(format!("{error:#}"))),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Appends the words of `term` the metadata doesn't contain yet.
    fn add_missing_words(&mut self, term: &str, window: &mut Window, cx: &mut Context<Self>) {
        let present = indexed_words(
            &Self::value(&self.fields.name, cx),
            &Self::value(&self.fields.subtitle, cx),
            &Self::value(&self.fields.keywords, cx),
        );
        let missing: Vec<String> = appdoctor_scoring::tokens(term)
            .into_iter()
            .filter(|word| !present.contains(word))
            .collect();
        if missing.is_empty() {
            return;
        }
        let mut keywords = Self::value(&self.fields.keywords, cx)
            .trim()
            .trim_end_matches(',')
            .to_owned();
        for word in missing {
            if !keywords.is_empty() {
                keywords.push(',');
            }
            keywords.push_str(&word);
        }
        self.fields
            .keywords
            .update(cx, |input, cx| input.set_value(keywords, window, cx));
        cx.notify();
    }

    /// A stacked group row: the label and its character count above the
    /// field, the count red once it's over Apple's limit.
    fn counted_field(label: &str, count: usize, limit: usize, enabled: bool, field: AnyElement, cx: &App) -> Div {
        let theme = cx.theme();
        let over = count > limit;
        v_flex()
            .px(px(10.))
            .pt(px(9.))
            .pb(px(10.))
            .gap(px(7.))
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .text_size(px(13.))
                            .text_color(theme.foreground.opacity(0.85))
                            .child(label.to_owned()),
                    )
                    .when(!enabled, |this| {
                        this.child(
                            div()
                                .text_size(px(11.))
                                .text_color(theme.muted_foreground)
                                .child("Read-only"),
                        )
                    })
                    .child(
                        div()
                            .text_size(px(12.))
                            .when(over, |this| this.font_weight(FontWeight::SEMIBOLD))
                            .text_color(if over { theme.danger } else { theme.muted_foreground })
                            .child(format!("{count}/{limit}")),
                    ),
            )
            .child(field)
    }

    /// Notes on the keyword field, or nothing when there's nothing to say.
    fn keyword_helper(&self, cx: &mut Context<Self>) -> Option<Div> {
        let name = Self::value(&self.fields.name, cx);
        let subtitle = Self::value(&self.fields.subtitle, cx);
        let keywords = Self::value(&self.fields.keywords, cx);
        let title_words: HashSet<String> = appdoctor_scoring::tokens(&format!("{name} {subtitle}"))
            .into_iter()
            .collect();
        let mut seen = HashSet::new();
        let (mut wasted, mut repeated) = (Vec::new(), Vec::new());
        for word in keywords.split(',').flat_map(appdoctor_scoring::tokens) {
            if title_words.contains(&word) {
                wasted.push(word);
            } else if !seen.insert(word.clone()) {
                repeated.push(word);
            }
        }
        wasted.dedup();
        repeated.dedup();
        let spaces = str::matches(keywords.as_str(), ", ").count();
        let present = indexed_words(&name, &subtitle, &keywords);
        let tracked: Vec<String> = self
            .state
            .read(cx)
            .visible_keywords()
            .map(|row| row.term.clone())
            .collect();
        let (covered, uncovered): (Vec<_>, Vec<_>) = tracked
            .into_iter()
            .partition(|term| appdoctor_scoring::tokens(term).iter().all(|w| present.contains(w)));
        let theme = cx.theme().clone();
        let note = |color: Hsla, text: String| {
            h_flex()
                .gap_2()
                .items_center()
                .text_size(px(12.))
                .child(status_dot(color))
                .child(div().text_color(theme.muted_foreground).child(text))
        };
        let anything = !wasted.is_empty()
            || !repeated.is_empty()
            || spaces > 0
            || !covered.is_empty()
            || !uncovered.is_empty();
        if !anything {
            return None;
        }
        let notes = v_flex()
            .gap(px(6.))
            .px(px(10.))
            .py(px(10.))
            .when(!wasted.is_empty(), |this| {
                this.child(note(
                    theme.warning,
                    format!(
                        "Already in the name or subtitle, so these waste characters: {}",
                        wasted.join(", ")
                    ),
                ))
            })
            .when(!repeated.is_empty(), |this| {
                this.child(note(theme.warning, format!("Repeated: {}", repeated.join(", "))))
            })
            .when(spaces > 0, |this| {
                this.child(note(
                    theme.warning,
                    format!(
                        "{spaces} space{} after commas; Apple doesn't need them",
                        if spaces == 1 { "" } else { "s" }
                    ),
                ))
            })
            .when(!covered.is_empty() || !uncovered.is_empty(), |this| {
                this.child(note(
                    meter::up(),
                    format!(
                        "Covers {} of {} tracked keywords. Click one to add its missing words:",
                        covered.len(),
                        covered.len() + uncovered.len()
                    ),
                ))
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap_1()
                        .children(uncovered.into_iter().take(12).map(|term| {
                            let label = term.clone();
                            div()
                                .id(SharedString::from(format!("cover-{term}")))
                                .cursor_pointer()
                                .child(pill(label, theme.primary))
                                .on_click(
                                    cx.listener(move |this, _, window, cx| this.add_missing_words(&term, window, cx)),
                                )
                        })),
                )
            });
        Some(notes)
    }
}

/// Words Apple indexes for search: name, subtitle and the keyword field.
fn indexed_words(name: &str, subtitle: &str, keywords: &str) -> HashSet<String> {
    appdoctor_scoring::tokens(&format!("{name} {subtitle} {}", keywords.replace(',', " ")))
        .into_iter()
        .collect()
}

/// The same text with empty fields as unset, however each got that way.
fn without_empties(text: ListingText) -> ListingText {
    let some = |value: Option<String>| value.filter(|value| !value.is_empty());
    ListingText {
        name: some(text.name),
        subtitle: some(text.subtitle),
        keywords: some(text.keywords),
        promotional_text: some(text.promotional_text),
        description: some(text.description),
        whats_new: some(text.whats_new),
    }
}

/// Where a version differs from `current`: each field's name and the version's text.
fn differences(version: &ListingText, current: &ListingText) -> Vec<(String, String)> {
    version
        .fields()
        .iter()
        .zip(current.fields())
        .filter(|(ours, theirs)| ours.3 != theirs.3)
        .map(|((_, label, _, value), _)| (label.to_string(), preview(value.map_or("", String::as_str))))
        .collect()
}

fn preview(text: &str) -> String {
    let flat = text.replace('\n', " ");
    if flat.chars().count() > 120 {
        format!("{}…", flat.chars().take(120).collect::<String>())
    } else if flat.is_empty() {
        "(empty)".into()
    } else {
        flat
    }
}

impl Render for LocalizationView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let services = self.services(cx);
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        // A request from the audit: open the chat with it.
        if let Some(prompt) = self.state.update(cx, |state, _| state.ai_request.take()) {
            self.ai_open = true;
            self.ai.update(cx, |ai, cx| ai.ask_now(prompt, cx));
        }
        if !services.has_asc_key() {
            // The AI can still suggest; saving needs the key.
            let connect = widgets::empty_state(
                gpui_kit::assets::IconName::Languages,
                "Connect App Store Connect",
                "Add your App Store Connect API key in Settings to edit your App Store metadata in every language from here, and to save what your AI suggests.",
                cx,
            )
            .child(Button::new("loc-settings").primary().label("Open Settings").on_click(cx.listener(|this, _, _, cx| {
                this.state.update(cx, |state, cx| state.set_section(Section::Settings, cx))
            })));
            return h_flex()
                .size_full()
                .child(div().flex_1().min_w_0().h_full().child(connect))
                .when(self.ai_open, |this| this.child(self.ai.clone()))
                .into_any_element();
        }
        // Reading the listing changes nothing, so it loads on arrival.
        if self.loaded.is_none() && !self.loading && self.auto_loaded != Some(app.id) {
            self.auto_loaded = Some(app.id);
            self.load(window, cx);
        }
        let theme = cx.theme().clone();
        let ai_open = self.ai_open;
        let ai_button = Button::new("localization-ai")
            .small()
            .icon(gpui_kit::assets::IconName::Sparkles)
            .label("AI")
            .tooltip("Improve the listing with your AI")
            .map(|button| if ai_open { button.primary() } else { button.ghost() })
            .on_click(cx.listener(|this, _, _, cx| {
                this.ai_open = !this.ai_open;
                this.history_open = false;
                cx.notify();
            }));
        let history_button = Button::new("localization-history")
            .small()
            .icon(gpui_kit::assets::IconName::Clock)
            .label("History")
            .tooltip("Earlier versions of this language")
            .disabled(self.loaded.is_none())
            .map(|button| if self.history_open { button.primary() } else { button.ghost() })
            .on_click(cx.listener(|this, _, _, cx| {
                this.history_open = !this.history_open;
                this.ai_open = false;
                this.refresh_versions(cx);
            }));
        let with_ai = move |body: AnyElement, ai: Entity<crate::ui::screenshots::AiPanel>| {
            h_flex()
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().h_full().child(body))
                .when(ai_open, |this| this.child(ai))
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
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Localization"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(match &self.loaded {
                                Some(loaded) if !loaded.version_label.is_empty() => {
                                    format!("{} · version {}", app.name, loaded.version_label)
                                }
                                _ => app.name.clone(),
                            }),
                    ),
            )
            .child(div().flex_1())
            .when_some(self.status.clone(), |this, status| {
                let (color, text) = match status {
                    Ok(text) => (meter::up(), text),
                    Err(text) => (theme.danger, text),
                };
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .text_xs()
                        .max_w(px(420.))
                        .child(status_dot(color))
                        .child(div().truncate().text_color(color).child(text)),
                )
            })
            .child(
                Button::new("load-localizations")
                    .small()
                    .icon(IconName::RefreshCw)
                    .label(if self.loaded.is_some() {
                        "Reload"
                    } else {
                        "Load from App Store Connect"
                    })
                    .loading(self.loading)
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
            )
            .child(history_button)
            .child(ai_button);

        let Some(loaded) = self.loaded.clone() else {
            let empty = widgets::empty_state(
                gpui_kit::assets::IconName::Languages,
                if self.loading { "Loading your metadata…" } else { "Load your metadata" },
                "AppDoctor reads every language's name, subtitle, keywords and description from App Store Connect. Nothing changes until you save and confirm.",
                cx,
            );
            return v_flex()
                .size_full()
                .child(header)
                .child(with_ai(empty.into_any_element(), self.ai.clone()))
                .into_any_element();
        };

        let rows = loaded.locales.iter().map(|locale| {
            let code = locale.code.clone();
            let active = self.selected.as_deref() == Some(code.as_str());
            let (text, subtext) = if active {
                (theme.primary_foreground, theme.primary_foreground.opacity(0.75))
            } else {
                (theme.foreground, theme.muted_foreground)
            };
            let suggested = self.draft(&code).is_some();
            v_flex()
                .id(SharedString::from(format!("locale-{code}")))
                .px(px(8.))
                .py(px(5.))
                .rounded(px(7.))
                .cursor_pointer()
                .map(|this| {
                    if active {
                        this.bg(theme.primary)
                    } else {
                        this.hover(|style| style.bg(theme.foreground.opacity(0.05)))
                    }
                })
                .child(
                    h_flex()
                        .gap(px(5.))
                        .items_center()
                        .child(div().text_size(px(13.)).text_color(text).child(locale_name(&code)))
                        .when(suggested, |this| {
                            this.child(
                                Icon::new(gpui_kit::assets::IconName::Sparkles)
                                    .size(px(11.))
                                    .text_color(if active { text } else { theme.primary }),
                            )
                        }),
                )
                .child(
                    div().text_size(px(11.)).text_color(subtext).truncate().child(
                        locale
                            .info
                            .as_ref()
                            .and_then(|i| i.name.clone())
                            .unwrap_or_else(|| code.clone()),
                    ),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.select(&code, window, cx)))
        });
        let locale_list = v_flex()
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                h_flex()
                    .h(px(28.))
                    .mt(px(10.))
                    .px(px(14.))
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Languages"),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(theme.muted_foreground)
                            .child(loaded.locales.len().to_string()),
                    ),
            )
            .child(
                v_flex()
                    .id("locale-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(8.))
                    .py(px(4.))
                    .gap(px(1.))
                    .children(rows),
            )
            .child(
                div().p(px(8.)).border_t_1().border_color(theme.sidebar_border).child(
                    Select::new(&self.add_locale)
                        .small()
                        .placeholder("Add language…")
                        .menu_width(px(260.)),
                ),
            );

        let count = |state: &Entity<InputState>, cx: &App| state.read(cx).value().chars().count();
        let area_count = |state: &Entity<TextareaState>, cx: &App| state.read(cx).value().chars().count();
        let line = |state: &Entity<InputState>, on: bool, cx: &App| {
            controls::well(cx)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Input::new(state).appearance(false).small().disabled(!on)),
                )
                .into_any_element()
        };
        let area = |state: &Entity<TextareaState>, on: bool, cx: &App| {
            controls::well(cx)
                .items_start()
                .py(px(4.))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .child(Textarea::new(state).appearance(false).disabled(!on)),
                )
                .into_any_element()
        };
        let info_on = loaded.info_editable;
        let version_on = loaded.version_editable;
        let fields = &self.fields;

        let app_info = v_flex()
            .child(controls::heading("App info", cx))
            .child(controls::rows(
                [
                    Self::counted_field(
                        "Name",
                        count(&fields.name, cx),
                        NAME_LIMIT,
                        info_on,
                        line(&fields.name, info_on, cx),
                        cx,
                    )
                    .into_any_element(),
                    Self::counted_field(
                        "Subtitle",
                        count(&fields.subtitle, cx),
                        SUBTITLE_LIMIT,
                        info_on,
                        line(&fields.subtitle, info_on, cx),
                        cx,
                    )
                    .into_any_element(),
                ],
                cx,
            ))
            .when(!info_on, |this| {
                this.child(controls::footnote(
                    "Name and subtitle can change only while the app's information isn't live. Create the next version in App Store Connect to edit them.",
                    cx,
                ))
            });

        let mut search_rows = vec![
            Self::counted_field(
                "Keywords",
                count(&fields.keywords, cx),
                KEYWORDS_LIMIT,
                version_on,
                line(&fields.keywords, version_on, cx),
                cx,
            )
            .into_any_element(),
        ];
        if let Some(helper) = self.keyword_helper(cx) {
            search_rows.push(helper.into_any_element());
        }
        let search = v_flex()
            .child(controls::heading("Search", cx))
            .child(controls::rows(search_rows, cx))
            .child(controls::footnote(
                "Separate keywords with commas, no spaces. Words already in the name or subtitle don't need repeating.",
                cx,
            ));

        let version = v_flex()
            .child(controls::heading("Version", cx))
            .child(controls::rows(
                [
                    Self::counted_field(
                        "Promotional text",
                        count(&fields.promo, cx),
                        PROMO_LIMIT,
                        version_on,
                        line(&fields.promo, version_on, cx),
                        cx,
                    )
                    .into_any_element(),
                    Self::counted_field(
                        "Description",
                        area_count(&fields.description, cx),
                        DESCRIPTION_LIMIT,
                        version_on,
                        area(&fields.description, version_on, cx),
                        cx,
                    )
                    .into_any_element(),
                    Self::counted_field(
                        "What's New",
                        area_count(&fields.whats_new, cx),
                        WHATS_NEW_LIMIT,
                        version_on,
                        area(&fields.whats_new, version_on, cx),
                        cx,
                    )
                    .into_any_element(),
                ],
                cx,
            ))
            .child(controls::footnote(
                if version_on {
                    "Keywords, description and What's New go live with the version; promotional text changes right away."
                } else {
                    "Keywords and these fields change only while a version is being prepared. Create the next version in App Store Connect to edit them."
                },
                cx,
            ));

        let editor = div().id("locale-editor").flex_1().min_w_0().h_full().overflow_y_scroll().child(
            h_flex().w_full().justify_center().child(
                v_flex()
                    .w_full()
                    .max_w(px(720.))
                    .px_6()
                    .pt(px(22.))
                    .pb(px(40.))
                    .gap(px(24.))
                    .children(self.suggestion_banner(cx))
                    .child(app_info)
                    .child(search)
                    .child(version)
                    .child(
                        h_flex().justify_end().child(
                            Button::new("save-localization")
                                .primary()
                                .label("Review and Save…")
                                .loading(self.saving)
                                .disabled(self.saving || self.selected.is_none() || (!info_on && !version_on))
                                .on_click(cx.listener(|this, _, window, cx| this.confirm_save(window, cx))),
                        ),
                    ),
            ),
        );

        let history = self
            .selected
            .clone()
            .filter(|_| self.history_open)
            .map(|code| self.history_panel(&code, info_on || version_on, cx));
        let body = h_flex().size_full().child(locale_list).child(editor).children(history);
        v_flex()
            .size_full()
            .child(header)
            .child(with_ai(body.into_any_element(), self.ai.clone()))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use appdoctor_store::ListingText;

    use super::{differences, without_empties};

    #[test]
    fn empty_and_unset_fields_are_the_same_text() {
        let pulled = ListingText {
            name: Some("Moonwake".into()),
            subtitle: Some(String::new()),
            ..ListingText::default()
        };
        let saved = ListingText {
            name: Some("Moonwake".into()),
            ..ListingText::default()
        };
        assert_eq!(without_empties(pulled), saved);
    }

    #[test]
    fn differences_name_each_field_with_the_versions_text() {
        let version = ListingText {
            name: Some("Moonwake".into()),
            subtitle: Some("Sleep sounds".into()),
            keywords: None,
            ..ListingText::default()
        };
        let current = ListingText {
            name: Some("Moonwake".into()),
            subtitle: Some("Gentle alarms".into()),
            keywords: Some("sleep,rain".into()),
            ..ListingText::default()
        };
        assert_eq!(
            differences(&version, &current),
            vec![
                ("Subtitle".to_owned(), "Sleep sounds".to_owned()),
                ("Keywords".to_owned(), "(empty)".to_owned()),
            ]
        );
        assert!(differences(&version, &version).is_empty());
    }
}
