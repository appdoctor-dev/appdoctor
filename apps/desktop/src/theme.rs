//! AppDoctor's look: gpui-kit's light/dark themes following the system,
//! with an indigo accent.

use gpui_kit::component::Theme;
use gpui_kit::{App, Hsla, Window, hsla, px};

fn accent(dark: bool) -> Hsla {
    if dark {
        hsla(243. / 360., 0.85, 0.68, 1.)
    } else {
        hsla(243. / 360., 0.75, 0.59, 1.)
    }
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
