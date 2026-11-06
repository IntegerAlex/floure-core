//! Embedded assets: one icon set and one typeface, no runtime file lookup.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

pub struct Assets;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn load_icon(name: &str) -> Option<Cow<'static, [u8]>> {
            match name {
                $( $name => Some(Cow::Borrowed(include_bytes!(concat!("../assets/icons/", $name)).as_slice())), )*
                _ => None,
            }
        }
    };
}

icons!(
    "mic.svg",
    "clock.svg",
    "chart-no-axes-column.svg",
    "sliders-horizontal.svg",
    "minus.svg",
    "square.svg",
    "square-filled.svg",
    "x.svg",
    "copy.svg",
    "star.svg",
    "star-filled.svg",
    "trash-2.svg",
    "chevron-down.svg",
    "chevron-up.svg",
    "search.svg",
    "check.svg",
    "play.svg",
);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let name = path.strip_prefix("icons/").unwrap_or(path);
        Ok(load_icon(name))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

pub fn font() -> Cow<'static, [u8]> {
    Cow::Borrowed(include_bytes!("../assets/fonts/Inter.ttf").as_slice())
}
