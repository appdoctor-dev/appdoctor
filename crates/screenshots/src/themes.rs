//! Curated looks for a whole design: background, type, accent colour,
//! device finish and glow, applied to every slide at once so a set reads
//! as one piece.

use crate::{Background, Design, Frame, LayerKind, has_font};

pub struct Theme {
    pub name: &'static str,
    pub description: &'static str,
    /// The background gradient, and how many degrees a second it turns in
    /// videos.
    pub from: &'static str,
    pub to: &'static str,
    pub angle: f32,
    pub spin: f32,
    /// Headline, subtitle and accent (for `*marked*` words) colours.
    pub text: &'static str,
    pub subtitle: &'static str,
    pub accent: &'static str,
    /// Used when installed; otherwise the default font.
    pub font: &'static str,
    pub weight: u16,
    pub frame: Frame,
    /// The colour of soft glow shapes.
    pub glow: &'static str,
}

pub const THEMES: &[Theme] = &[
    Theme {
        name: "aurora",
        description: "Deep night blue with violet glows and a mint accent; calm, premium.",
        from: "#0b1026",
        to: "#1e1b4b",
        angle: 160.,
        spin: 6.,
        text: "#ffffff",
        subtitle: "#c7d2fe",
        accent: "#5eead4",
        font: "Avenir Next",
        weight: 800,
        frame: Frame::Black,
        glow: "#8b5cf6",
    },
    Theme {
        name: "sunset",
        description: "Orange into pink with a yellow accent; warm and energetic.",
        from: "#f97316",
        to: "#db2777",
        angle: 160.,
        spin: 0.,
        text: "#ffffff",
        subtitle: "#ffe4e6",
        accent: "#fde047",
        font: "Futura",
        weight: 800,
        frame: Frame::Silver,
        glow: "#fbbf24",
    },
    Theme {
        name: "ocean",
        description: "Cyan into deep blue; clean, trustworthy, productivity.",
        from: "#06b6d4",
        to: "#1d4ed8",
        angle: 160.,
        spin: 0.,
        text: "#ffffff",
        subtitle: "#cffafe",
        accent: "#fef08a",
        font: "Helvetica Neue",
        weight: 800,
        frame: Frame::Blue,
        glow: "#22d3ee",
    },
    Theme {
        name: "mint",
        description: "Fresh greens; health, money, calm.",
        from: "#34d399",
        to: "#047857",
        angle: 160.,
        spin: 0.,
        text: "#ffffff",
        subtitle: "#d1fae5",
        accent: "#fef3c7",
        font: "Avenir Next",
        weight: 700,
        frame: Frame::Silver,
        glow: "#6ee7b7",
    },
    Theme {
        name: "paper",
        description: "Warm off-white with dark serif type and an orange accent; editorial.",
        from: "#fafaf9",
        to: "#e7e5e4",
        angle: 180.,
        spin: 0.,
        text: "#1c1917",
        subtitle: "#57534e",
        accent: "#ea580c",
        font: "Georgia",
        weight: 700,
        frame: Frame::Black,
        glow: "#fdba74",
    },
    Theme {
        name: "neon",
        description: "Near-black with a lime accent and glow; games, tech, nightlife.",
        from: "#09090b",
        to: "#18181b",
        angle: 160.,
        spin: 0.,
        text: "#fafafa",
        subtitle: "#a1a1aa",
        accent: "#a3e635",
        font: "Futura",
        weight: 800,
        frame: Frame::Black,
        glow: "#a3e635",
    },
    Theme {
        name: "candy",
        description: "Pink into lilac with rounded type; playful, kids, social.",
        from: "#fbcfe8",
        to: "#c4b5fd",
        angle: 150.,
        spin: 4.,
        text: "#3b0764",
        subtitle: "#6b21a8",
        accent: "#db2777",
        font: "Arial Rounded MT Bold",
        weight: 700,
        frame: Frame::Pink,
        glow: "#f0abfc",
    },
    Theme {
        name: "mono",
        description: "Black and graphite with a yellow accent; bold, minimal.",
        from: "#111111",
        to: "#2a2a2a",
        angle: 180.,
        spin: 0.,
        text: "#ffffff",
        subtitle: "#a3a3a3",
        accent: "#facc15",
        font: "Helvetica Neue",
        weight: 800,
        frame: Frame::Titanium,
        glow: "#facc15",
    },
];

/// The name of a soft shape that shades words over a picture.
pub const SHADE: &str = "Shade";

pub fn theme(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name.eq_ignore_ascii_case(name))
}

impl Design {
    /// Restyles every slide: backgrounds, each slide's largest text as the
    /// headline and the rest as subtitles, device finishes and glows.
    pub fn apply_theme(&mut self, theme: &Theme) {
        let font = has_font(theme.font).then(|| theme.font.to_owned());
        for slide in &mut self.slides {
            slide.background = Background::Gradient {
                from: theme.from.into(),
                to: theme.to.into(),
                angle: theme.angle,
            };
            slide.spin = theme.spin;
            let largest = slide
                .layers
                .iter()
                .filter_map(|layer| match &layer.kind {
                    LayerKind::Text { size, .. } => Some(*size),
                    _ => None,
                })
                .fold(0., f32::max);
            for layer in &mut slide.layers {
                match &mut layer.kind {
                    LayerKind::Text {
                        size,
                        weight,
                        color,
                        font: layer_font,
                        accent,
                        ..
                    } => {
                        let headline = *size >= largest;
                        *color = if headline { theme.text } else { theme.subtitle }.into();
                        *weight = if headline { theme.weight } else { 500 };
                        *accent = Some(theme.accent.into());
                        *layer_font = font.clone();
                    }
                    LayerKind::Device { frame, .. } if *frame != Frame::None => *frame = theme.frame,
                    // A shade behind words takes the background's colour, so
                    // the theme's text reads on it; other soft shapes glow.
                    LayerKind::Shape { fill, soft, .. } if *soft > 0. => {
                        *fill = if layer.name == SHADE { theme.from } else { theme.glow }.into()
                    }
                    _ => {}
                }
                layer.fit_text();
            }
        }
    }
}
