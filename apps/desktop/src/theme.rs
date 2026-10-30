//! AppDoctor's look: macOS's own greys and system indigo on top of
//! gpui-kit's light/dark themes, following the system appearance.

use gpui_kit::component::Theme;
use gpui_kit::{App, Hsla, Window, px, rgb};

/// Apple's systemIndigo.
fn accent(dark: bool) -> Hsla {
    hex(if dark { 0x5e5ce6 } else { 0x5856d6 })
}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

/// Follows the system appearance and applies the accent. Call again when
/// the appearance changes, since switching mode reloads the base theme.
pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    Theme::sync_system_appearance(window, cx);
    Theme::update(cx, |theme| {
        // Native Mac density: gpui-kit's 16px default reads large next to
        // AppKit apps, and every rem-based size follows this.
        theme.font_size = px(14.);
        let accent = accent(theme.mode.is_dark());
        let hover = accent.opacity(0.9);
        let active = Hsla {
            l: accent.l - 0.06,
            ..accent
        };
        theme.primary = accent;
        theme.primary_hover = hover;
        theme.primary_active = active;
        theme.button_primary = accent;
        theme.button_primary_hover = hover;
        theme.button_primary_active = active;
        theme.ring = accent;
        theme.progress_bar = accent;
        theme.sidebar_primary = accent;
        theme.link = accent;
        theme.slider_bar = accent;
        theme.switch_thumb = hex(0xffffff);
        theme.slider_thumb = hex(0xffffff);
        theme.radius = px(8.);
        theme.radius_lg = px(12.);

        // AppKit's window, sidebar and control greys rather than the
        // near-black / pure-white web defaults.
        let dark = theme.mode.is_dark();
        let (background, raised, control, control_hover, separator, secondary_label, label) = if dark {
            (0x1e1e1e, 0x262628, 0x2c2c2e, 0x3a3a3c, 0x38383a, 0x98989d, 0xf5f5f7)
        } else {
            (0xffffff, 0xf5f5f7, 0xf2f2f7, 0xe5e5ea, 0xe0e0e5, 0x6e6e73, 0x1d1d1f)
        };
        theme.background = hex(background);
        theme.foreground = hex(label);
        theme.border = hex(separator);
        theme.input = hex(control_hover);
        theme.muted = hex(control);
        theme.muted_foreground = hex(secondary_label);
        theme.secondary = hex(control);
        theme.secondary_hover = hex(control_hover);
        theme.accent = hex(control);
        theme.list_hover = hex(control);
        theme.popover = hex(if dark { 0x2c2c2e } else { 0xffffff });
        theme.sidebar = hex(raised);
        theme.sidebar_border = hex(separator);
        theme.sidebar_accent = hex(if dark { 0x3a3a3c } else { 0xe3e3e8 });
        theme.title_bar = hex(raised);
        theme.title_bar_border = hex(separator);
        theme.switch = hex(control_hover);
        // Tables and lists like Finder's: the window colour with faint
        // alternate rows, and the accent for the selection.
        let stripe = theme.foreground.opacity(if dark { 0.025 } else { 0.03 });
        theme.table = hex(background);
        theme.table_head = hex(background);
        theme.table_foot = hex(background);
        theme.table_even = stripe;
        theme.table_hover = hex(control);
        theme.table_row_border = hex(background);
        theme.table_active = accent.opacity(0.22);
        theme.table_active_border = accent.opacity(0.0);
        theme.colors.list = hex(background);
        theme.list_head = hex(background);
        theme.list_even = stripe;
        theme.list_active = accent.opacity(0.22);
        theme.list_active_border = accent.opacity(0.0);
        theme.status_bar = hex(raised);
        theme.status_bar_border = hex(separator);
        theme.tab_bar = hex(raised);
        theme.tab_bar_segmented = hex(control);
    });
}

/// Colours for the difficulty and popularity meters, shared by the table.
pub mod meter {
    use gpui_kit::{Hsla, hsla};

    pub fn difficulty(score: f32) -> Hsla {
        match score {
            s if s < 35. => hsla(142. / 360., 0.60, 0.45, 1.),
            s if s < 60. => hsla(38. / 360., 0.92, 0.50, 1.),
            s if s < 80. => hsla(20. / 360., 0.90, 0.54, 1.),
            _ => hsla(0., 0.78, 0.56, 1.),
        }
    }

    pub fn popularity(score: u8) -> Hsla {
        match score {
            s if s < 20 => hsla(0., 0.0, 0.62, 1.),
            s if s < 40 => hsla(38. / 360., 0.92, 0.50, 1.),
            s if s < 60 => hsla(84. / 360., 0.62, 0.46, 1.),
            _ => hsla(142. / 360., 0.60, 0.42, 1.),
        }
    }

    pub fn up() -> Hsla {
        hsla(142. / 360., 0.60, 0.40, 1.)
    }

    pub fn down() -> Hsla {
        hsla(0., 0.72, 0.52, 1.)
    }
}
