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
    "chart-line",
    "image",
    "key-round",
    "languages",
    "log-out",
    "smartphone",
    "sparkles",
    "stethoscope",
    "trending-up",
    "users",
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
