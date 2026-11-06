//! Dictate: the microphone is the screen.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::app::FloureApp;
use crate::theme;

pub fn render(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let status = if app.recording {
        ("Listening…", theme::accent())
    } else {
        ("Ready", theme::text_muted())
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(theme::GUTTER)
        .pt(theme::SPACE_8)
        .pb(theme::SPACE_6)
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .items_center()
                .justify_center()
                .gap(theme::SPACE_4)
                .child(mic_button(app, cx))
                .child(
                    div()
                        .text_size(theme::TEXT_SM)
                        .text_color(status.1)
                        .child(status.0),
                )
                .child(shortcut_pill(&app.config.hotkey))
                .child(waveform(app.recording)),
        )
        .child(transcript_panel(app))
        .child(session_strip(app))
        .into_any_element()
}

fn mic_button(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let (fill, border, glyph) = if app.recording {
        (theme::accent(), theme::accent(), theme::text())
    } else {
        (theme::raised(), theme::border(), theme::text())
    };

    div()
        .id("dictate-mic")
        .role(gpui::Role::Button)
        .aria_label(if app.recording {
            "Stop dictation"
        } else {
            "Start dictation"
        })
        .size(px(112.0))
        .rounded(px(9999.0))
        .bg(fill)
        .border_1()
        .border_color(border)
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .hover(|style| {
            style.bg(if app.recording {
                theme::accent()
            } else {
                theme::surface()
            })
        })
        .child(big_mic(glyph))
        .on_click(cx.listener(|this, _, _, cx| this.toggle_recording(cx)))
        .into_any_element()
}

fn big_mic(color: gpui::Rgba) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w(px(16.0))
                .h(px(28.0))
                .rounded(px(9.0))
                .border_2()
                .border_color(color),
        )
        .child(
            div()
                .w(px(28.0))
                .h(px(15.0))
                .mt(px(-7.0))
                .border_2()
                .border_t_0()
                .rounded_b(px(15.0))
                .border_color(color),
        )
        .child(div().w(px(2.0)).h(px(6.0)).bg(color))
        .child(div().w(px(14.0)).h(px(2.0)).rounded(px(1.0)).bg(color))
        .into_any_element()
}

fn shortcut_pill(hotkey: &str) -> AnyElement {
    div()
        .px(theme::SPACE_3)
        .py(px(5.0))
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .text_size(theme::TEXT_XS)
        .text_color(theme::text_secondary())
        .child(crate::app::display_hotkey(hotkey))
        .into_any_element()
}

fn waveform(recording: bool) -> AnyElement {
    // 24 flat, monochrome bars. No glow, no gradient. A deterministic resting
    // shape keeps the row from looking broken when the mic is idle.
    let resting = [
        3.0, 6.0, 10.0, 14.0, 11.0, 7.0, 4.0, 8.0, 13.0, 16.0, 12.0, 6.0,
    ];
    let color = if recording {
        theme::text_secondary()
    } else {
        theme::text_faint()
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .h(px(20.0));
    for i in 0..24 {
        let height = resting[i % resting.len()];
        row = row.child(div().w(px(2.0)).h(px(height)).rounded(px(1.0)).bg(color));
    }
    row.into_any_element()
}

fn transcript_panel(app: &FloureApp) -> AnyElement {
    let mut panel = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(640.0))
        .mx_auto()
        .min_h(px(140.0))
        .rounded(theme::RADIUS_PANEL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .p(theme::SPACE_6);

    if let Some(error) = &app.engine_error {
        panel = panel.child(
            div()
                .mb(theme::SPACE_3)
                .text_size(theme::TEXT_SM)
                .text_color(theme::error())
                .child(error.clone()),
        );
    }

    panel
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .mb(theme::SPACE_4)
                .child(
                    div()
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_faint())
                        .child("Transcript"),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_faint())
                        .child("Copy   Cancel   Clear"),
                ),
        )
        .child(
            div()
                .text_size(theme::TEXT_XL)
                .text_color(theme::text_muted())
                .child("Press the shortcut and speak"),
        )
        .child(
            div()
                .mt(theme::SPACE_2)
                .text_size(theme::TEXT_BASE)
                .text_color(theme::text_faint())
                .child("Your words appear here as you finish each sentence."),
        )
        .into_any_element()
}

fn session_strip(app: &FloureApp) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .max_w(px(640.0))
        .mx_auto()
        .mt(theme::SPACE_4)
        .text_size(theme::TEXT_XS)
        .text_color(theme::text_faint())
        .child("0 words   0 WPM   0:00")
        .child(format!("{} · CPU", asr_label(app)))
        .into_any_element()
}

fn asr_label(app: &FloureApp) -> &'static str {
    match app.config.asr_profile {
        floure_core::config::AsrProfile::Parakeet => "Parakeet",
        floure_core::config::AsrProfile::WhisperTurbo => "Whisper Turbo",
        floure_core::config::AsrProfile::WhisperBase => "Whisper Base",
    }
}
