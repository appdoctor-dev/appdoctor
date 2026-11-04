//! Languages of a design. A language can have its own copy of the design
//! (`design.de-DE.json` beside `design.json`), which starts from the default
//! design that every other language uses. It has its own words, pictures and
//! version history, and can take later layout changes from the default while
//! keeping those.

use appdoctor_screenshots::Design;
use appdoctor_store::{LOCALES, locale_name};
use gpui_kit::assets::IconName as ExtraIcon;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectItem, SelectState};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{ScreenshotsView, write_pngs};
use crate::ui::controls::{group, heading};
use crate::ui::versions;

/// A language in the picker; its value is the App Store Connect code, empty
/// for the default design.
#[derive(Clone)]
pub(super) struct LanguageItem {
    code: SharedString,
    title: SharedString,
    designed: bool,
}

impl SelectItem for LanguageItem {
    type Value = SharedString;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        h_flex()
            .w_full()
            .gap_2()
            .items_center()
            .child(div().flex_1().child(self.title.clone()))
            .when(self.designed, |this| {
                this.child(Icon::new(IconName::Check).size(px(13.)))
            })
    }

    fn value(&self) -> &SharedString {
        &self.code
    }
}

pub(super) type LanguageSelect = SelectState<SearchableVec<LanguageItem>>;

const DEFAULT: &str = "Default";

pub(super) fn language_select(
    window: &mut Window,
    cx: &mut Context<ScreenshotsView>,
) -> (Entity<LanguageSelect>, Subscription) {
    let select = cx.new(|cx| LanguageSelect::new(SearchableVec::new(Vec::new()), None, window, cx).searchable(true));
    let subscription = cx.subscribe(&select, |this, _, event: &SelectEvent<_>, cx| {
        let SelectEvent::Confirm(code) = event;
        let locale = code.as_ref().filter(|code| !code.is_empty()).map(ToString::to_string);
        this.state.update(cx, |state, cx| {
            if state.screenshot_locale != locale {
                state.set_screenshot_locale(locale, cx);
            }
        });
    });
    (select, subscription)
}

impl ScreenshotsView {
    /// Fills the picker for the open design: the default, then languages it
    /// has or the listing has, then the rest.
    pub(super) fn refresh_languages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut designed = self
            .folder(cx)
            .map(|folder| Design::languages(&folder, self.target))
            .unwrap_or_default();
        if let Some(locale) = self.locale.clone().filter(|locale| !designed.contains(locale)) {
            designed.push(locale);
        }
        let services = self.state.read(cx).services.clone();
        let listed: Vec<String> = self
            .app_id
            .and_then(|app| services.store.listings(app).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|listing| listing.locale)
            .collect();
        let item = |code: &str| LanguageItem {
            code: code.to_owned().into(),
            title: locale_name(code).into(),
            designed: designed.iter().any(|known| known == code),
        };
        let used = |code: &str| designed.iter().chain(&listed).any(|known| known == code);
        let mut items = vec![LanguageItem {
            code: SharedString::default(),
            title: DEFAULT.into(),
            designed: false,
        }];
        items.extend(
            LOCALES
                .iter()
                .filter(|(code, _)| used(code))
                .map(|(code, _)| item(code)),
        );
        items.extend(
            LOCALES
                .iter()
                .filter(|(code, _)| !used(code))
                .map(|(code, _)| item(code)),
        );
        let selected = SharedString::from(self.locale.clone().unwrap_or_default());
        self.languages.update(cx, |select, cx| {
            select.set_items(SearchableVec::new(items), window, cx);
            select.set_selected_value(&selected, window, cx);
        });
        designed.sort();
        self.designed_languages = designed;
    }

    /// Asks the AI panel to put the open language's words into that language.
    fn translate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(code) = self.locale.clone() else {
            return;
        };
        let name = locale_name(&code);
        self.close_history(window, cx);
        self.ai_open = true;
        let left = self.left_open;
        self.set_panels(left, true, cx);
        let prompt = format!("Translate this design into {name}.");
        self.ai.update(cx, |ai, cx| ai.ask_now(prompt, cx));
        cx.notify();
    }

    /// Takes the default design's layout, keeping this language's words and
    /// pictures. Undo takes it back.
    fn update_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        let default = Design::load_for(&folder, self.target);
        if default.slides.is_empty() {
            self.status = Some(Err("The default design is empty, so there's no layout to take.".into()));
            cx.notify();
            return;
        }
        let merged = default.with_language(&self.design);
        self.store_version("Before updating the layout", false, cx);
        self.apply(window, cx, |this| {
            this.design.slides = merged.slides;
            this.design.video = merged.video;
            this.next_id = this.next_id.max(this.design.max_id());
            this.slide = this.slide.min(this.design.slides.len().saturating_sub(1));
            this.selected = None;
        });
        self.thumbs.clear();
        self.render_thumbs(cx);
        let name = self.locale.as_deref().map(locale_name).unwrap_or_default();
        self.status = Some(Ok(format!(
            "Took the default layout and kept the {name} words and pictures. ⌘Z undoes it."
        )));
        cx.notify();
    }

    /// Back to using the default design for this language. Its history stays,
    /// so opening the language again can bring it back.
    fn delete_language(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.design_path(cx) else {
            return;
        };
        self.flush(cx);
        self.store_version("Before deleting this language", false, cx);
        self._save = None;
        if let Err(error) = std::fs::remove_file(&path) {
            self.status = Some(Err(format!("Couldn't delete it: {error}")));
            cx.notify();
            return;
        }
        self.state.update(cx, |state, cx| state.set_screenshot_locale(None, cx));
    }

    /// Writes the default design and every language into its own folder.
    fn export_languages(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder(cx) else {
            return;
        };
        self.flush(cx);
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Export Here".into()),
        });
        let (target, languages) = (self.target, self.designed_languages.clone());
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(root) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| {
                this.exporting = true;
                cx.notify();
            })
            .ok();
            let destination = root.clone();
            let result = cx
                .background_spawn(async move {
                    let (mut written, mut sets) = (0, 0);
                    let default = std::iter::once(None);
                    for locale in default.chain(languages.iter().map(|code| Some(code.as_str()))) {
                        let design = Design::load_language(&folder, target, locale);
                        if design.slides.is_empty() {
                            continue;
                        }
                        let out = destination.join(locale.unwrap_or("default"));
                        std::fs::create_dir_all(&out).map_err(|error| error.to_string())?;
                        written += write_pngs(&design.slides, target.sizes(), &folder, &out)?;
                        sets += 1;
                    }
                    Ok::<_, String>((written, sets))
                })
                .await;
            this.update(cx, |this, cx| {
                this.exporting = false;
                this.status =
                    Some(result.map(|(written, sets)| {
                        format!("Exported {written} PNGs in {sets} folders to {}", root.display())
                    }));
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The picker in the header, with a way to translate a language.
    pub(super) fn language_picker(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .flex_shrink_0()
            .gap(px(6.))
            .items_center()
            .child(
                div().w(px(168.)).child(
                    Select::new(&self.languages)
                        .small()
                        .icon(ExtraIcon::Languages)
                        .placeholder(DEFAULT)
                        .menu_width(px(260.)),
                ),
            )
            .when(self.locale.is_some(), |this| {
                this.child(
                    Button::new("translate-design")
                        .ghost()
                        .small()
                        .icon(ExtraIcon::Sparkles)
                        .label("Translate")
                        .tooltip("Have your AI translate this language's words")
                        .on_click(cx.listener(|this, _, window, cx| this.translate(window, cx))),
                )
            })
    }

    /// What can be done with languages, above the slide's own properties.
    pub(super) fn language_properties(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.locale.is_none() && self.designed_languages.is_empty() {
            return None;
        }
        let theme = cx.theme().clone();
        let name = self.locale.as_deref().map(locale_name);
        let note = match &name {
            Some(name) => format!(
                "{name} has its own copy of this design. Change its words and pictures here; the default is used for languages without their own."
            ),
            None => "This default design is used for every language without its own.".to_owned(),
        };
        let count = self.designed_languages.len();
        Some(
            v_flex()
                .child(heading(name.clone().unwrap_or_else(|| "Languages".to_owned()), cx))
                .child(
                    group(cx)
                        .p(px(10.))
                        .gap(px(8.))
                        .child(div().text_size(px(12.)).text_color(theme.muted_foreground).child(note))
                        .when(self.locale.is_some(), |this| {
                            this.child(
                                Button::new("update-layout")
                                    .small()
                                    .w_full()
                                    .label("Update Layout from Default")
                                    .tooltip("Takes the default's layout and keeps this language's words and pictures")
                                    .on_click(cx.listener(|this, _, window, cx| this.update_layout(window, cx))),
                            )
                        })
                        .when(count > 0, |this| {
                            this.child(
                                Button::new("export-languages")
                                    .small()
                                    .w_full()
                                    .label("Export All Languages")
                                    .tooltip("Writes the default design and each language into its own folder")
                                    .disabled(self.exporting)
                                    .on_click(cx.listener(|this, _, window, cx| this.export_languages(window, cx))),
                            )
                        })
                        .when_some(name, |this, name| {
                            let view = cx.entity().downgrade();
                            this.child(
                                Button::new("delete-language")
                                    .ghost()
                                    .small()
                                    .w_full()
                                    .label(format!("Delete {name} Design"))
                                    .on_click(move |_, window, cx| {
                                        let view = view.clone();
                                        versions::confirm(
                                            format!("Delete the {name} design?"),
                                            format!(
                                                "{name} goes back to using the default design. Its version history stays, so opening {name} again can bring it back."
                                            ),
                                            "Delete",
                                            window,
                                            cx,
                                            move |_, cx| {
                                                view.update(cx, |this, cx| this.delete_language(cx)).ok();
                                            },
                                        );
                                    }),
                            )
                        }),
                ),
        )
    }
}
