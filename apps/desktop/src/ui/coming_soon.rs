//! Sections that are part of the plan but not built yet. Each lives inside
//! this one window, like the rest of AppDoctor.

use gpui_kit::component::{ActiveTheme as _, Icon, v_flex};
use gpui_kit::*;

use crate::state::Section;
use crate::ui::widgets::{self, pill};

pub fn render(section: Section, cx: &App) -> AnyElement {
    let (icon, title, body): (Icon, &str, &str) = match section {
        Section::Rankings => (
            Icon::new(gpui_kit::assets::IconName::ChartLine),
            "Rank history",
            "Charts of every keyword's position over time, per storefront, from the history each rank check already records.",
        ),
        Section::Competitors => (
            Icon::new(gpui_kit::assets::IconName::Users),
            "Competitors",
            "Follow the apps that rank beside you: their keywords, ratings and reviews, and where they overtake you.",
        ),
        Section::Localization => (
            Icon::new(gpui_kit::assets::IconName::Languages),
            "Localization",
            "Edit name, subtitle, keywords and promotional text for every locale and push them to App Store Connect, or let your own AI agent do it through AppDoctor's local MCP server.",
        ),
        Section::Screenshots => (
            Icon::new(gpui_kit::assets::IconName::Image),
            "Screenshots",
            "Design App Store screenshots with device frames and captions, right here in AppDoctor, then export every size Apple asks for.",
        ),
        _ => (Icon::new(gpui_kit::assets::IconName::Sparkles), "Coming soon", ""),
    };
    widgets::empty_state(icon, title, body, cx)
        .child(pill("Planned", cx.theme().primary))
        .child(v_flex())
        .into_any_element()
}
