//! Design tokens for the pinned monochrome dark direction.
//!
//! One ground, one accent. Nothing here should be changed per-screen: if a
//! screen needs a new colour, it is the wrong colour.

use gpui::{FontWeight, Pixels, px, rgb, rgba};

// Ground and surfaces.
pub const BG: u32 = 0x000000;
pub const SURFACE: u32 = 0x0A0A0A;
pub const RAISED: u32 = 0x141414;
/// 10% white. Borders are never a solid grey: they must sit on any surface.
pub const BORDER: u32 = 0xFFFFFF1A;

// Text.
pub const TEXT: u32 = 0xF5F5F7;
pub const TEXT_SECONDARY: u32 = 0xA1A1A6;
/// ≥4.5:1 on black. The only muted tone body text may use.
pub const TEXT_MUTED: u32 = 0x8E8E93;
/// Decorative/large text only (17 px and up).
pub const TEXT_FAINT: u32 = 0x6E6E73;

// Semantic. Accent is reserved for recording and live data.
pub const ACCENT: u32 = 0xFF453A;
pub const ERROR: u32 = 0xFF6961;

// Type scale.
pub const TEXT_XS: Pixels = px(12.0);
pub const TEXT_SM: Pixels = px(13.0);
pub const TEXT_BASE: Pixels = px(15.0);
pub const TEXT_LG: Pixels = px(17.0);
pub const TEXT_XL: Pixels = px(22.0);
pub const TEXT_2XL: Pixels = px(28.0);

// Radii.
pub const RADIUS_CONTROL: Pixels = px(8.0);
pub const RADIUS_PANEL: Pixels = px(12.0);

// Spacing.
pub const SPACE_1: Pixels = px(4.0);
pub const SPACE_2: Pixels = px(8.0);
pub const SPACE_3: Pixels = px(12.0);
pub const SPACE_4: Pixels = px(16.0);
pub const SPACE_6: Pixels = px(24.0);
pub const SPACE_8: Pixels = px(32.0);
pub const SPACE_12: Pixels = px(48.0);

// Shell dimensions.
pub const TITLEBAR_HEIGHT: Pixels = px(44.0);
pub const RAIL_WIDTH: Pixels = px(64.0);
pub const GUTTER: Pixels = px(32.0);

pub const FONT_FAMILY: &str = "Inter";

pub fn border() -> gpui::Rgba {
    rgba(BORDER)
}

pub fn ground() -> gpui::Rgba {
    rgb(BG)
}

pub fn surface() -> gpui::Rgba {
    rgb(SURFACE)
}

pub fn raised() -> gpui::Rgba {
    rgb(RAISED)
}

pub fn text() -> gpui::Rgba {
    rgb(TEXT)
}

pub fn text_secondary() -> gpui::Rgba {
    rgb(TEXT_SECONDARY)
}

pub fn text_muted() -> gpui::Rgba {
    rgb(TEXT_MUTED)
}

pub fn text_faint() -> gpui::Rgba {
    rgb(TEXT_FAINT)
}

pub fn accent() -> gpui::Rgba {
    rgb(ACCENT)
}

pub fn error() -> gpui::Rgba {
    rgb(ERROR)
}

pub fn semibold() -> FontWeight {
    FontWeight::SEMIBOLD
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance for an sRGB hex colour.
    fn luminance(hex: u32) -> f32 {
        let channel = |c: u32| {
            let c = c as f32 / 255.0;
            if c <= 0.039_28 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let r = channel((hex >> 16) & 0xFF);
        let g = channel((hex >> 8) & 0xFF);
        let b = channel(hex & 0xFF);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    fn contrast(a: u32, b: u32) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn muted_body_text_meets_aa_on_black() {
        assert!(
            contrast(TEXT_MUTED, BG) >= 4.5,
            "muted text is {:.2}:1",
            contrast(TEXT_MUTED, BG)
        );
    }

    #[test]
    fn primary_and_secondary_text_are_high_contrast() {
        assert!(contrast(TEXT, BG) >= 7.0);
        assert!(contrast(TEXT_SECONDARY, BG) >= 7.0);
    }

    #[test]
    fn accent_is_readable_for_status_text() {
        assert!(contrast(ACCENT, BG) >= 4.5);
        assert!(contrast(ERROR, BG) >= 4.5);
    }
}
