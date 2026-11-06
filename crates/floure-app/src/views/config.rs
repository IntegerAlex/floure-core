//! Configuration shell: one page, a section list, and the General section.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};

use crate::app::FloureApp;
use crate::theme;

const SECTIONS: [&str; 8] = [
    "General",
    "Shortcuts",
    "Models",
    "Dictionary",
    "Cleanup",
    "Output",
    "Microphone & Output",
    "Diagnostics",
];

pub fn render(app: &FloureApp, _cx: &mut Context<FloureApp>) -> AnyElement {
    let mut sections = div()
        .flex()
        .flex_col()
        .w(px(200.0))
        .flex_shrink_0()
        .gap(px(2.0));
    for (index, section) in SECTIONS.iter().enumerate() {
        let active = index == 0;
        let mut row = div()
            .h(px(36.0))
            .px(theme::SPACE_3)
            .flex()
            .items_center()
            .rounded(theme::RADIUS_CONTROL)
            .text_size(theme::TEXT_SM)
            .text_color(if active {
                theme::text()
            } else {
                theme::text_muted()
            });
        if active {
            row = row.bg(theme::raised());
        }
        sections = sections.child(row.child(*section));
    }

    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(theme::GUTTER)
        .pt(theme::SPACE_8)
        .pb(theme::SPACE_6)
        .child(
            div()
                .text_size(theme::TEXT_2XL)
                .font_weight(theme::semibold())
                .child("Configuration"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap(theme::SPACE_8)
                .mt(theme::SPACE_6)
                .child(sections)
                .child(general(app)),
        )
        .into_any_element()
}

fn general(app: &FloureApp) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .max_w(px(560.0))
        .child(section_title("General"))
        .child(row("Language", language_label(app)))
        .child(row("Speech model", asr_label(app)))
        .child(row(
            "Shortcut",
            crate::app::display_hotkey(&app.config.hotkey),
        ))
        .child(row(
            "Activation",
            match app.config.ptt_mode {
                floure_core::config::PttMode::Hold => "Hold to talk",
                floure_core::config::PttMode::Toggle => "Tap to toggle",
            },
        ))
        .child(row(
            "Typing",
            if app.config.typing_enabled {
                "On"
            } else {
                "Off"
            },
        ))
        .child(row(
            "Clipboard",
            if app.config.clipboard_enabled {
                "On"
            } else {
                "Off"
            },
        ))
        .child(row(
            "Motion",
            if app.reduce_motion { "Reduced" } else { "Full" },
        ))
        .into_any_element()
}

fn section_title(title: &str) -> AnyElement {
    div()
        .mb(theme::SPACE_2)
        .text_size(theme::TEXT_LG)
        .font_weight(theme::semibold())
        .child(title.to_string())
        .into_any_element()
}

fn row(label: &str, value: impl Into<String>) -> AnyElement {
    div()
        .h(px(44.0))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(theme::border())
        .child(
            div()
                .text_size(theme::TEXT_BASE)
                .text_color(theme::text())
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(theme::TEXT_SM)
                .text_color(theme::text_muted())
                .child(value.into()),
        )
        .into_any_element()
}

fn language_label(app: &FloureApp) -> String {
    match app.config.language.as_str() {
        "auto" => "Detect automatically".to_string(),
        "en" => "English".to_string(),
        "de" => "German".to_string(),
        "es" => "Spanish".to_string(),
        "fr" => "French".to_string(),
        other => other.to_uppercase(),
    }
}

fn asr_label(app: &FloureApp) -> String {
    match app.config.asr_profile {
        floure_core::config::AsrProfile::Parakeet => "Parakeet v2".to_string(),
        floure_core::config::AsrProfile::WhisperTurbo => "Whisper Turbo".to_string(),
        floure_core::config::AsrProfile::WhisperBase => "Whisper Base".to_string(),
    }
}
