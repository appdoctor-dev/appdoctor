//! The main window's frame: title bar, the sidebar (tracked apps, then the
//! sections of the selected app), and the current page.

use appdoctor_entitlement::Status;
use appdoctor_screenshots::Target;
use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, TitleBar, h_flex, v_flex};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::state::{AppState, Section, UpdateStatus};
use crate::theme::{self, meter};
use crate::ui::apps::AppsView;
use crate::ui::competitors::CompetitorsView;
use crate::ui::keywords::KeywordsView;
use crate::ui::localization::LocalizationView;
use crate::ui::rankings::RankingsView;
use crate::ui::release::ReleaseView;
use crate::ui::screenshots::ScreenshotsView;
use crate::ui::settings::SettingsView;
use crate::ui::widgets::{app_icon, flag, status_dot};

const SIDEBAR_WIDTH: f32 = 236.;

pub struct Shell {
    state: Entity<AppState>,
    keywords: Entity<KeywordsView>,
    apps: Entity<AppsView>,
    release: Entity<ReleaseView>,
    rankings: Entity<RankingsView>,
    competitors: Entity<CompetitorsView>,
    localization: Entity<LocalizationView>,
    screenshots: Entity<ScreenshotsView>,
    settings: Entity<SettingsView>,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let keywords = cx.new(|cx| KeywordsView::new(state.clone(), window, cx));
        let apps = cx.new(|cx| AppsView::new(state.clone(), window, cx));
        let release = cx.new(|cx| ReleaseView::new(state.clone(), cx));
        let rankings = cx.new(|cx| RankingsView::new(state.clone(), cx));
        let competitors = cx.new(|cx| CompetitorsView::new(state.clone(), window, cx));
        let localization = cx.new(|cx| LocalizationView::new(state.clone(), window, cx));
        let screenshots = cx.new(|cx| ScreenshotsView::new(state.clone(), window, cx));
        let settings = cx.new(|cx| SettingsView::new(state.clone(), window, cx));
        let subscriptions = vec![
            cx.observe(&state, |_, _, cx| cx.notify()),
            cx.observe_window_appearance(window, |_, window, cx| theme::apply(Some(window), cx)),
        ];
        Self {
            state,
            keywords,
            apps,
            release,
            rankings,
            competitors,
            localization,
            screenshots,
            settings,
            _subscriptions: subscriptions,
        }
    }

    fn go(&self, section: Section, cx: &mut App) {
        self.state.update(cx, |state, cx| state.set_section(section, cx));
    }

    fn nav_item(
        &self,
        id: &'static str,
        icon: impl Into<Icon>,
        label: &'static str,
        section: Section,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let active = self.state.read(cx).section == section;
        let theme = cx.theme();
        h_flex()
            .id(id)
            .h(px(30.))
            .px_2()
            .gap_2()
            .items_center()
            .rounded(theme.radius)
            .text_sm()
            .cursor_pointer()
            .map(|this| {
                if active {
                    this.bg(theme.sidebar_accent)
                        .text_color(theme.sidebar_accent_foreground)
                        .font_weight(FontWeight::MEDIUM)
                } else {
                    this.text_color(theme.sidebar_foreground)
                        .hover(|style| style.bg(theme.sidebar_accent.opacity(0.6)))
                }
            })
            .child(
                icon.into()
                    .small()
                    .text_color(if active { theme.primary } else { theme.muted_foreground }),
            )
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| this.go(section, cx)))
    }

    /// A design under Screenshots: iPhone, iPad, header or search art.
    fn screenshot_item(&self, target: Target, cx: &mut Context<Self>) -> Stateful<Div> {
        let state = self.state.read(cx);
        let active = state.section == Section::Screenshots && state.screenshot_target == target;
        let theme = cx.theme();
        let (icon, label) = match target {
            Target::Iphone => (gpui_kit::assets::IconName::Smartphone, "iPhone"),
            Target::Ipad => (gpui_kit::assets::IconName::Tablet, "iPad"),
            Target::Header => (gpui_kit::assets::IconName::PanelTop, "Page header"),
            Target::Search => (gpui_kit::assets::IconName::ScanSearch, "Search result"),
        };
        h_flex()
            .id(("nav-shots", target as usize))
            .h(px(26.))
            .ml(px(22.))
            .px_2()
            .gap_2()
            .items_center()
            .rounded(theme.radius)
            .text_size(px(12.5))
            .cursor_pointer()
            .map(|this| {
                if active {
                    this.bg(theme.sidebar_accent)
                        .text_color(theme.sidebar_accent_foreground)
                        .font_weight(FontWeight::MEDIUM)
                } else {
                    this.text_color(theme.muted_foreground)
                        .hover(|style| style.bg(theme.sidebar_accent.opacity(0.6)))
                }
            })
            .child(
                Icon::new(icon)
                    .size(px(13.))
                    .text_color(if active { theme.primary } else { theme.muted_foreground }),
            )
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.state.update(cx, |state, cx| state.open_screenshots(target, cx))
            }))
    }

    fn group_label(label: &str, cx: &App) -> Div {
        h_flex()
            .px_2()
            .pt_3()
            .pb_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(label.to_owned())
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);
        let apps = state.apps.clone();
        let selected = state.selected_app;
        let on_keywords_page = !matches!(state.section, Section::Apps | Section::Settings);
        let icons = state.icons.clone();
        let account = state.account.status.clone();
        let update = state.update.clone();
        let theme = cx.theme().clone();
        let screenshot_items: Vec<_> = Target::ALL
            .into_iter()
            .map(|target| self.screenshot_item(target, cx))
            .collect();

        let app_rows: Vec<_> = apps
            .into_iter()
            .map(|app| {
                let id = app.id;
                let active = selected == Some(id) && on_keywords_page;
                let icon = app
                    .icon_url
                    .clone()
                    .and_then(|url| icons.update(cx, |icons, cx| icons.get(&url, cx)));
                let count = self.state.read(cx).keyword_count(id);
                h_flex()
                    .id(("sidebar-app", id))
                    .px_2()
                    .py_1p5()
                    .gap_2p5()
                    .items_center()
                    .rounded(theme.radius)
                    .cursor_pointer()
                    .map(|this| {
                        if active {
                            this.bg(theme.sidebar_accent)
                        } else {
                            this.hover(|style| style.bg(theme.sidebar_accent.opacity(0.6)))
                        }
                    })
                    .child(app_icon(icon, px(28.), cx))
                    .child(
                        v_flex()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.sidebar_foreground)
                                    .truncate()
                                    .child(app.name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{} iPhone · {count} keywords", flag(&app.country))),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.state.update(cx, |state, cx| {
                            state.select_app(id, cx);
                            if !matches!(
                                state.section,
                                Section::Rankings
                                    | Section::Competitors
                                    | Section::Localization
                                    | Section::Screenshots
                                    | Section::Release
                            ) {
                                state.set_section(Section::Keywords, cx);
                            }
                        })
                    }))
                    .context_menu({
                        let state = self.state.clone();
                        move |menu, _, _| {
                            let state = state.clone();
                            menu.item(PopupMenuItem::new("Remove app…").on_click(move |_, window, cx| {
                                crate::ui::apps::confirm_remove_app(state.clone(), id, window, cx)
                            }))
                        }
                    })
            })
            .collect();

        let (account_color, account_label) = match &account {
            Status::DevBuild => (meter::up(), "Developer build".to_owned()),
            Status::Active { plan, .. } => (meter::up(), format!("{} plan", capitalize(plan))),
            Status::Expired { .. } => (theme.warning, "Licence expired".to_owned()),
            Status::NotActivated => (theme.muted_foreground, "Not activated".to_owned()),
            Status::Invalid(_) => (theme.danger, "Licence problem".to_owned()),
        };

        v_flex()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .bg(theme.sidebar)
            .border_r_1()
            .border_color(theme.sidebar_border)
            .child(
                v_flex()
                    .id("sidebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px_2()
                    .pb_2()
                    .child(
                        Self::group_label("Apps", cx).child(div().flex_1()).child(
                            div()
                                .id("sidebar-add-app")
                                .p_0p5()
                                .rounded(theme.radius)
                                .cursor_pointer()
                                .hover(|style| style.bg(theme.sidebar_accent))
                                .child(Icon::new(IconName::Plus).small().text_color(theme.muted_foreground))
                                .on_click(cx.listener(|this, _, _, cx| this.go(Section::Apps, cx))),
                        ),
                    )
                    .children(app_rows)
                    .child(
                        h_flex()
                            .id("sidebar-add-app-row")
                            .px_2()
                            .py_1p5()
                            .gap_2p5()
                            .items_center()
                            .rounded(theme.radius)
                            .cursor_pointer()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .when(self.state.read(cx).section == Section::Apps, |this| {
                                this.bg(theme.sidebar_accent)
                            })
                            .hover(|style| style.bg(theme.sidebar_accent.opacity(0.6)))
                            .child(
                                div()
                                    .size(px(28.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_dashed()
                                    .border_color(theme.border)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::Plus).small()),
                            )
                            .child("Add app")
                            .on_click(cx.listener(|this, _, _, cx| this.go(Section::Apps, cx))),
                    )
                    .child(Self::group_label("Optimize", cx))
                    .child(self.nav_item("nav-keywords", IconName::Search, "Keywords", Section::Keywords, cx))
                    .child(self.nav_item(
                        "nav-rankings",
                        gpui_kit::assets::IconName::ChartLine,
                        "Rankings",
                        Section::Rankings,
                        cx,
                    ))
                    .child(self.nav_item(
                        "nav-competitors",
                        gpui_kit::assets::IconName::Users,
                        "Competitors",
                        Section::Competitors,
                        cx,
                    ))
                    .child(Self::group_label("Create", cx))
                    .child(self.nav_item(
                        "nav-localization",
                        gpui_kit::assets::IconName::Languages,
                        "Localization",
                        Section::Localization,
                        cx,
                    ))
                    .child(self.nav_item(
                        "nav-screenshots",
                        gpui_kit::assets::IconName::Image,
                        "Screenshots",
                        Section::Screenshots,
                        cx,
                    ))
                    .children(screenshot_items)
                    .child(Self::group_label("Ship", cx))
                    .child(self.nav_item("nav-release", IconName::CircleCheck, "App Review", Section::Release, cx)),
            )
            .child(
                v_flex()
                    .p_2()
                    .gap_1()
                    .border_t_1()
                    .border_color(theme.sidebar_border)
                    .when_some(
                        match update {
                            UpdateStatus::Available(info) => Some(info.version),
                            _ => None,
                        },
                        |this, version| {
                            this.child(
                                h_flex()
                                    .id("sidebar-update")
                                    .px_2()
                                    .py_1()
                                    .gap_2()
                                    .rounded(theme.radius)
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(theme.primary)
                                    .bg(theme.primary.opacity(0.1))
                                    .cursor_pointer()
                                    .child(Icon::new(IconName::ArrowUp).xsmall())
                                    .child(format!("Update to {version}"))
                                    .on_click(cx.listener(|this, _, _, cx| this.go(Section::Settings, cx))),
                            )
                        },
                    )
                    .child(self.nav_item("nav-settings", IconName::Settings, "Settings", Section::Settings, cx))
                    .child(
                        h_flex()
                            .px_2()
                            .py_1()
                            .gap_2()
                            .items_center()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(status_dot(account_color))
                            .child(account_label),
                    ),
            )
    }

    fn title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        TitleBar::new()
            .bg(theme.sidebar)
            .border_color(theme.sidebar_border)
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .text_sm()
                    .child(
                        div()
                            .size(px(20.))
                            .rounded(px(5.))
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::new(gpui_kit::assets::IconName::Stethoscope)
                                    .xsmall()
                                    .text_color(theme.primary_foreground),
                            ),
                    )
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("AppDoctor")),
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

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.state.read(cx).section;
        let page = match section {
            Section::Keywords => self.keywords.clone().into_any_element(),
            Section::Apps => self.apps.clone().into_any_element(),
            Section::Release => self.release.clone().into_any_element(),
            Section::Rankings => self.rankings.clone().into_any_element(),
            Section::Competitors => self.competitors.clone().into_any_element(),
            Section::Localization => self.localization.clone().into_any_element(),
            Section::Screenshots => self.screenshots.clone().into_any_element(),
            Section::Settings => self.settings.clone().into_any_element(),
        };
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(div().flex_1().min_w_0().h_full().child(page)),
            )
    }
}
