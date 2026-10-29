//! Localization: the App Store metadata for every language, edited in
//! place and pushed to App Store Connect after a confirmation that lists
//! exactly what changes.
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
use gpui_kit::component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::runtime;
use crate::services::Services;
use crate::state::{AppState, Section};
use crate::theme::meter;
use crate::ui::widgets::{self, card, pill, status_dot};

const NAME_LIMIT: usize = 30;
const SUBTITLE_LIMIT: usize = 30;
const KEYWORDS_LIMIT: usize = 100;
const PROMO_LIMIT: usize = 170;
const DESCRIPTION_LIMIT: usize = 4000;
const WHATS_NEW_LIMIT: usize = 4000;

/// App Store Connect's metadata locales.
const LOCALES: &[(&str, &str)] = &[
    ("ar-SA", "Arabic"),
    ("ca", "Catalan"),
    ("zh-Hans", "Chinese (Simplified)"),
    ("zh-Hant", "Chinese (Traditional)"),
    ("hr", "Croatian"),
    ("cs", "Czech"),
    ("da", "Danish"),
    ("nl-NL", "Dutch"),
    ("en-AU", "English (Australia)"),
    ("en-CA", "English (Canada)"),
    ("en-GB", "English (U.K.)"),
    ("en-US", "English (U.S.)"),
    ("fi", "Finnish"),
    ("fr-FR", "French"),
    ("fr-CA", "French (Canada)"),
    ("de-DE", "German"),
    ("el", "Greek"),
    ("he", "Hebrew"),
    ("hi", "Hindi"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("it", "Italian"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ms", "Malay"),
    ("no", "Norwegian"),
    ("pl", "Polish"),
    ("pt-BR", "Portuguese (Brazil)"),
    ("pt-PT", "Portuguese (Portugal)"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("es-MX", "Spanish (Mexico)"),
    ("es-ES", "Spanish (Spain)"),
    ("sv", "Swedish"),
    ("th", "Thai"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("vi", "Vietnamese"),
];

fn locale_name(code: &str) -> String {
    LOCALES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map_or_else(|| code.to_owned(), |(_, name)| (*name).to_owned())
}

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

        let mut subscriptions = vec![
            cx.observe(&state, |this, state, cx| {
                if this.loaded.as_ref().map(|l| l.app_id) != state.read(cx).selected_app {
                    this.loaded = None;
                    this.selected = None;
                    this.status = None;
                }
                cx.notify();
            }),
            cx.subscribe_in(&add_locale, window, |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(code)) = event {
                    this.add_locale(code, window, cx);
                }
            }),
        ];
        for input in [&fields.name, &fields.subtitle, &fields.keywords, &fields.promo] {
            subscriptions.push(cx.subscribe(input, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }
        for area in [&fields.description, &fields.whats_new] {
            subscriptions.push(cx.subscribe(area, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            }));
        }

        Self {
            state,
            loaded: None,
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
                        let keep = this
                            .selected
                            .clone()
                            .filter(|code| loaded.locales.iter().any(|l| &l.code == code));
                        let first = loaded.locales.first().map(|l| l.code.clone());
                        this.loaded = Some(loaded);
                        if let Some(code) = keep.or(first) {
                            this.select(&code, window, cx);
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

    fn select(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(locale) = self.locale(code).cloned() else {
            return;
        };
        self.selected = Some(code.to_owned());
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
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .child("These fields change on App Store Connect. Apple shows them with the next release (promotional text, right away)."),
                        )
                        .children(changes.lines.iter().map(|(label, value)| {
                            h_flex()
                                .gap_3()
                                .text_sm()
                                .child(div().w(px(120.)).flex_shrink_0().text_color(theme.muted_foreground).child(label.clone()))
                                .child(div().flex_1().child(value.clone()))
                        })),
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
            this.update(cx, |this, cx| {
                this.saving = false;
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
            // Re-read so the editor's baseline is what Apple now has.
            this.update(cx, |this, cx| this.reload_after_save(cx)).ok();
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

    fn counted_field(
        &self,
        label: &str,
        count: usize,
        limit: usize,
        enabled: bool,
        field: AnyElement,
        cx: &App,
    ) -> impl IntoElement {
        let over = count > limit;
        v_flex()
            .gap_1()
            .child(
                h_flex()
                    .text_xs()
                    .child(div().flex_1().font_weight(FontWeight::MEDIUM).child(label.to_owned()))
                    .when(!enabled, |this| {
                        this.child(div().pr_2().text_color(cx.theme().muted_foreground).child("read-only"))
                    })
                    .child(
                        div()
                            .text_color(if over {
                                cx.theme().danger
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(format!("{count}/{limit}")),
                    ),
            )
            .child(field)
    }

    fn keyword_helper(&self, cx: &mut Context<Self>) -> impl IntoElement {
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
                .text_xs()
                .child(status_dot(color))
                .child(div().text_color(theme.muted_foreground).child(text))
        };
        v_flex()
            .gap_1p5()
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
            })
    }
}

/// Words Apple indexes for search: name, subtitle and the keyword field.
fn indexed_words(name: &str, subtitle: &str, keywords: &str) -> HashSet<String> {
    appdoctor_scoring::tokens(&format!("{name} {subtitle} {}", keywords.replace(',', " ")))
        .into_iter()
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let services = self.services(cx);
        let Some(app) = self.state.read(cx).selected().cloned() else {
            return widgets::empty_state(IconName::Inbox, "No app selected", "Pick an app in the sidebar.", cx)
                .into_any_element();
        };
        if !services.has_asc_key() {
            return widgets::empty_state(
                gpui_kit::assets::IconName::Languages,
                "Connect App Store Connect",
                "Add your App Store Connect API key in Settings to edit your App Store metadata in every language from here.",
                cx,
            )
            .child(Button::new("loc-settings").primary().label("Open Settings").on_click(cx.listener(|this, _, _, cx| {
                this.state.update(cx, |state, cx| state.set_section(Section::Settings, cx))
            })))
            .into_any_element();
        }
        let theme = cx.theme().clone();

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
                    .outline()
                    .icon(IconName::RefreshCw)
                    .label(if self.loaded.is_some() {
                        "Reload"
                    } else {
                        "Load from App Store Connect"
                    })
                    .loading(self.loading)
                    .disabled(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
            );

        let Some(loaded) = self.loaded.clone() else {
            return v_flex()
                .size_full()
                .child(header)
                .child(widgets::empty_state(
                    gpui_kit::assets::IconName::Languages,
                    "Load your metadata",
                    "AppDoctor reads every language's name, subtitle, keywords and description from App Store Connect. Nothing changes until you save and confirm.",
                    cx,
                ))
                .into_any_element();
        };

        let locale_list = v_flex()
            .id("locale-list")
            .w(px(220.))
            .flex_shrink_0()
            .h_full()
            .overflow_y_scroll()
            .border_r_1()
            .border_color(theme.border)
            .p_2()
            .gap_0p5()
            .children(loaded.locales.iter().map(|locale| {
                let code = locale.code.clone();
                let active = self.selected.as_deref() == Some(code.as_str());
                v_flex()
                    .id(SharedString::from(format!("locale-{code}")))
                    .px_2()
                    .py_1p5()
                    .rounded(theme.radius)
                    .cursor_pointer()
                    .map(|this| {
                        if active {
                            this.bg(theme.accent)
                        } else {
                            this.hover(|style| style.bg(theme.accent.opacity(0.6)))
                        }
                    })
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .child(locale_name(&code)),
                    )
                    .child(
                        div().text_xs().text_color(theme.muted_foreground).truncate().child(
                            locale
                                .info
                                .as_ref()
                                .and_then(|i| i.name.clone())
                                .unwrap_or_else(|| code.clone()),
                        ),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| this.select(&code, window, cx)))
            }))
            .child(
                div().pt_2().child(
                    Select::new(&self.add_locale)
                        .small()
                        .placeholder("Add language…")
                        .menu_width(px(260.)),
                ),
            );

        let count = |state: &Entity<InputState>, cx: &App| state.read(cx).value().chars().count();
        let area_count = |state: &Entity<TextareaState>, cx: &App| state.read(cx).value().chars().count();
        let info_on = loaded.info_editable;
        let version_on = loaded.version_editable;
        let editor = v_flex()
            .id("locale-editor")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .p_5()
            .gap_4()
            .when(!info_on || !version_on, |this| {
                this.child(div().text_xs().text_color(theme.muted_foreground).child(
                    "Name and subtitle need an app info that isn't live yet, and the other fields need a version being prepared. Create the next version in App Store Connect to edit everything.",
                ))
            })
            .child(
                h_flex()
                    .gap_4()
                    .child(div().flex_1().child(self.counted_field(
                        "Name",
                        count(&self.fields.name, cx),
                        NAME_LIMIT,
                        info_on,
                        Input::new(&self.fields.name).disabled(!info_on).into_any_element(),
                        cx,
                    )))
                    .child(div().flex_1().child(self.counted_field(
                        "Subtitle",
                        count(&self.fields.subtitle, cx),
                        SUBTITLE_LIMIT,
                        info_on,
                        Input::new(&self.fields.subtitle).disabled(!info_on).into_any_element(),
                        cx,
                    ))),
            )
            .child(self.counted_field(
                "Keywords",
                count(&self.fields.keywords, cx),
                KEYWORDS_LIMIT,
                version_on,
                Input::new(&self.fields.keywords).disabled(!version_on).into_any_element(),
                cx,
            ))
            .child(card(cx).p_3().gap_2().child(self.keyword_helper(cx)))
            .child(self.counted_field(
                "Promotional text",
                count(&self.fields.promo, cx),
                PROMO_LIMIT,
                version_on,
                Input::new(&self.fields.promo).disabled(!version_on).into_any_element(),
                cx,
            ))
            .child(self.counted_field(
                "Description",
                area_count(&self.fields.description, cx),
                DESCRIPTION_LIMIT,
                version_on,
                Textarea::new(&self.fields.description).disabled(!version_on).into_any_element(),
                cx,
            ))
            .child(self.counted_field(
                "What's New",
                area_count(&self.fields.whats_new, cx),
                WHATS_NEW_LIMIT,
                version_on,
                Textarea::new(&self.fields.whats_new).disabled(!version_on).into_any_element(),
                cx,
            ))
            .child(
                h_flex().child(
                    Button::new("save-localization")
                        .primary()
                        .label("Review and save…")
                        .loading(self.saving)
                        .disabled(self.saving || self.selected.is_none() || (!info_on && !version_on))
                        .on_click(cx.listener(|this, _, window, cx| this.confirm_save(window, cx))),
                ),
            );

        v_flex()
            .size_full()
            .child(header)
            .child(h_flex().flex_1().min_h_0().child(locale_list).child(editor))
            .into_any_element()
    }
}
