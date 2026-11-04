//! gpui-kit's default icons plus the few extra Lucide icons AppDoctor uses
//! (see assets/icons/LICENSE-LUCIDE), embedded so the binary needs no files.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

macro_rules! extra_icons {
    ($($name:literal),* $(,)?) => {
        &[$((concat!("icons/", $name, ".svg"), include_bytes!(concat!("../assets/icons/", $name, ".svg")))),*]
    };
}

static EXTRA: &[(&str, &[u8])] = extra_icons![
    "bookmark",
    "chart-line",
    "clapperboard",
    "clock",
    "gauge",
    "image",
    "key-round",
    "languages",
    "lightbulb",
    "lock",
    "lock-open",
    "log-out",
    "message-square-plus",
    "panel-right-close",
    "panel-top",
    "pause",
    "play",
    "rotate-ccw",
    "smartphone",
    "scan-search",
    "sparkles",
    "stethoscope",
    "tablet",
    "text-align-center",
    "text-align-end",
    "text-align-start",
    "trending-up",
    "type",
    "users",
    "volume-2",
    "wand-sparkles",
    "zoom-in",
];

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = EXTRA.iter().find(|(extra, _)| *extra == path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut all = gpui_kit::assets::Assets.list(path)?;
        all.extend(
            EXTRA
                .iter()
                .filter(|(extra, _)| extra.starts_with(path))
                .map(|(extra, _)| SharedString::from(*extra)),
        );
        Ok(all)
    }
}
