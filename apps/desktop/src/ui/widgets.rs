//! Small presentational pieces shared by the pages.

use std::sync::Arc;

use appdoctor_appstore::{COUNTRIES, Country};
use gpui_kit::component::select::{SearchableVec, SelectItem, SelectState};
use gpui_kit::component::{ActiveTheme as _, IndexPath, h_flex, v_flex};
use gpui_kit::*;

/// An app icon, or a rounded placeholder while it loads.
pub fn app_icon(image: Option<Arc<Image>>, size: Pixels, cx: &App) -> AnyElement {
    let radius = size * 0.225;
    match image {
        Some(image) => img(image).size(size).rounded(radius).flex_shrink_0().into_any_element(),
        None => div()
            .size(size)
            .rounded(radius)
            .flex_shrink_0()
            .bg(cx.theme().muted)
            .border_1()
            .border_color(cx.theme().border)
            .into_any_element(),
    }
}

/// The flag emoji for an ISO country code.
pub fn flag(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphabetic)
        .take(2)
        .filter_map(|c| char::from_u32(0x1F1E6 + (c.to_ascii_lowercase() as u32 - 'a' as u32)))
        .collect()
}

/// A 0–100 value with a short bar, as the keyword table shows popularity
/// and difficulty.
pub fn meter(value: f32, color: Hsla, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(
            div()
                .w(px(22.))
                .text_right()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .child(format!("{}", value.round() as i64)),
        )
        .child(
            div().w(px(52.)).h(px(5.)).rounded_full().bg(cx.theme().muted).child(
                div()
                    .h_full()
                    .rounded_full()
                    .bg(color)
                    .w(relative((value / 100.).clamp(0.04, 1.))),
            ),
        )
}

/// "just now", "5m ago", "3h ago", "2d ago".
pub fn relative_time(timestamp: i64, now: i64) -> String {
    let seconds = (now - timestamp).max(0);
    match seconds {
        0..60 => "just now".into(),
        60..3_600 => format!("{}m ago", seconds / 60),
        3_600..86_400 => format!("{}h ago", seconds / 3_600),
        _ => format!("{}d ago", seconds / 86_400),
    }
}

/// Compact counts: 950, 12.3K, 4.1M.
pub fn compact(count: u64) -> String {
    match count {
        0..1_000 => count.to_string(),
        1_000..1_000_000 => format!("{:.1}K", count as f64 / 1_000.).replace(".0K", "K"),
        _ => format!("{:.1}M", count as f64 / 1_000_000.).replace(".0M", "M"),
    }
}

/// A bordered surface for grouping a page's controls.
pub fn card(cx: &App) -> Div {
    v_flex()
        .gap_3()
        .p_4()
        .rounded(cx.theme().radius_lg)
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

/// A page heading with a one-line description.
pub fn page_header(title: &str, description: &str, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(description.to_owned()),
        )
}

/// Centered empty-state block.
pub fn empty_state(icon: impl Into<gpui_kit::component::Icon>, title: &str, body: &str, cx: &App) -> Div {
    v_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_3()
        .p_8()
        .child(
            div()
                .size(px(52.))
                .rounded(px(14.))
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(icon.into().size(px(24.))),
        )
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(body.to_owned()),
        )
}

/// A small rounded label.
pub fn pill(text: impl Into<SharedString>, color: Hsla) -> impl IntoElement {
    div()
        .px_2()
        .py(px(1.))
        .rounded_full()
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .text_color(color)
        .bg(color.opacity(0.12))
        .child(text.into())
}

/// "▲ 3" in green, "▼ 2" in red, "New", "Out", "=", "—".
pub fn change_label(change: appdoctor_store::RankChange, cx: &App) -> (String, Hsla) {
    use appdoctor_store::RankChange;
    let muted = cx.theme().muted_foreground;
    match change {
        RankChange::Up(n) => (format!("▲ {n}"), crate::theme::meter::up()),
        RankChange::Down(n) => (format!("▼ {n}"), crate::theme::meter::down()),
        RankChange::Entered => ("New".into(), crate::theme::meter::up()),
        RankChange::Dropped => ("Out".into(), crate::theme::meter::down()),
        RankChange::Same => ("=".into(), muted),
        RankChange::First | RankChange::Unchecked => ("—".into(), muted),
    }
}

pub fn status_dot(color: Hsla) -> impl IntoElement {
    div().size(px(7.)).rounded_full().bg(color).flex_shrink_0()
}

// Country picker

#[derive(Clone)]
pub struct CountryItem {
    country: &'static Country,
    title: SharedString,
}

impl SelectItem for CountryItem {
    type Value = &'static str;

    fn title(&self) -> SharedString {
        self.title.clone()
    }

    fn value(&self) -> &Self::Value {
        &self.country.code
    }

    fn matches(&self, query: &str) -> bool {
        let query = query.trim().to_lowercase();
        self.country.name.to_lowercase().contains(&query) || self.country.code == query
    }
}

pub type CountrySelect = SelectState<SearchableVec<CountryItem>>;

pub fn country_select(selected: &str, window: &mut Window, cx: &mut Context<CountrySelect>) -> CountrySelect {
    let items: Vec<CountryItem> = COUNTRIES
        .iter()
        .map(|country| CountryItem {
            country,
            title: format!("{}  {}", flag(country.code), country.name).into(),
        })
        .collect();
    let selected = items
        .iter()
        .position(|item| item.country.code == selected)
        .map(IndexPath::new);
    SelectState::new(SearchableVec::new(items), selected, window, cx).searchable(true)
}
