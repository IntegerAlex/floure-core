//! Dictate: the microphone is the screen.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::app::{FloureApp, RunState};
use crate::hotkeys::HotkeyStatus;
use crate::theme;

pub fn render(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut center = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(theme::SPACE_4)
        .child(mic_button(app, cx))
        .child(
            div()
                .text_size(theme::TEXT_SM)
                .text_color(app.status.color())
                .child(app.status.label()),
        );

    if app.status == RunState::NoModel {
        center = center.child(
            div()
                .id("open-models")
                .role(gpui::Role::Button)
                .aria_label("Open Configuration, Models")
                .px(theme::SPACE_3)
                .py(px(5.0))
                .rounded(theme::RADIUS_CONTROL)
                .border_1()
                .border_color(theme::border())
                .bg(theme::surface())
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_secondary())
                .cursor_pointer()
                .hover(|style| style.bg(theme::raised()))
                .child("Open Configuration → Models")
                .on_click(cx.listener(|this, _, _, cx| this.go_models(cx))),
        );
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
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(220.0))
                .max_h(px(460.0))
                .items_center()
                .justify_center()
                .gap(theme::SPACE_4)
                .child(center)
                .when(!app.recording, |block| block.child(shortcut_pill(app, cx)))
                .child(waveform(app)),
        )
        .child(transcript_panel(app, cx))
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
        .child(if app.recording {
            theme::icon("square-filled", px(24.0), glyph).into_any_element()
        } else {
            theme::icon("mic", px(40.0), glyph).into_any_element()
        })
        .on_click(cx.listener(|this, _, _, cx| this.toggle_recording(cx)))
        .into_any_element()
}

pub fn shortcut_display(status: &HotkeyStatus, hotkey: &str) -> (String, Option<String>) {
    match status {
        HotkeyStatus::RegisteredEvdev(_) | HotkeyStatus::RegisteredX11(_) => {
            (crate::app::display_hotkey(hotkey), None)
        }
        HotkeyStatus::Failed(error) => (
            "Shortcut unavailable".to_string(),
            Some(format!("Use the tray or run floure --toggle ({error})")),
        ),
        HotkeyStatus::NeedsInputGroup => (
            "Global shortcut needs the input group".to_string(),
            Some(format!(
                "Run {} then log out and back in",
                crate::hotkeys::INPUT_GROUP_COMMAND
            )),
        ),
        HotkeyStatus::Unavailable => (
            "floure --toggle".to_string(),
            Some("Click to copy · bind it in your compositor".to_string()),
        ),
    }
}

/// What the shortcut chip copies: the fix for the current status, never a
/// command that does not apply.
pub fn shortcut_copy(status: &HotkeyStatus) -> &'static str {
    match status {
        HotkeyStatus::NeedsInputGroup => crate::hotkeys::INPUT_GROUP_COMMAND,
        _ => "floure --toggle",
    }
}

fn shortcut_pill(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let (text, hint) = shortcut_display(&app.hotkey_status, &app.config.hotkey);
    let registered = matches!(
        app.hotkey_status,
        HotkeyStatus::RegisteredEvdev(_) | HotkeyStatus::RegisteredX11(_)
    );
    let actionable = !registered;
    let copy = shortcut_copy(&app.hotkey_status);
    let mut chip = div()
        .id("copy-trigger")
        .role(gpui::Role::Button)
        .aria_label("Copy trigger command")
        .px(theme::SPACE_3)
        .py(px(5.0))
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .text_size(theme::TEXT_XS)
        .text_color(theme::text_secondary());
    if actionable {
        chip = chip
            .cursor_pointer()
            .hover(|style| style.bg(theme::raised()))
            .on_click(cx.listener(move |this, _, _, cx| this.copy_to_clipboard_text(copy, cx)));
    }
    let mut pill = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.0))
        .child(chip.child(text));
    if let Some(hint) = hint {
        pill = pill.child(
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_muted())
                .child(hint),
        );
    }
    pill.into_any_element()
}

fn waveform(app: &FloureApp) -> AnyElement {
    // 24 flat, monochrome bars. The mic level scales them; idle settles to a
    // low resting shape so the row never looks broken.
    let pattern = [
        0.30, 0.55, 0.85, 1.0, 0.75, 0.45, 0.25, 0.50, 0.80, 0.95, 0.70, 0.35,
    ];
    let gain = if app.recording {
        0.65 + app.mic_level * 3.0
    } else {
        0.9
    };
    let color = if app.recording {
        theme::text_secondary()
    } else {
        theme::text_muted()
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(3.0))
        .h(px(24.0));
    for i in 0..24 {
        let height = (pattern[i % pattern.len()] * gain * 20.0).clamp(2.0, 20.0);
        row = row.child(div().w(px(2.0)).h(px(height)).rounded(px(1.0)).bg(color));
    }
    row.into_any_element()
}

fn transcript_panel(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut panel = div()
        .flex()
        .flex_col()
        .w_full()
        .max_w(px(640.0))
        .mx_auto()
        .min_h(px(140.0))
        .max_h(px(280.0))
        .rounded(theme::RADIUS_PANEL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .p(theme::SPACE_6);

    if let Some(error) = &app.engine_error {
        panel = panel.child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .mb(theme::SPACE_3)
                .child(
                    div()
                        .text_size(theme::TEXT_SM)
                        .text_color(theme::error())
                        .child(error.clone()),
                )
                .child(
                    div()
                        .id("dismiss-error")
                        .role(gpui::Role::Button)
                        .aria_label("Dismiss error")
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_muted())
                        .cursor_pointer()
                        .child("Dismiss")
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))),
                ),
        );
    }

    let has_lines = !app.utterances.is_empty();
    let active = has_lines
        || app.recording
        || matches!(
            app.status,
            RunState::Transcribing | RunState::Rewriting | RunState::Stopping
        );
    let mut head = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .mb(theme::SPACE_4)
        .child(
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_muted())
                .child("Transcript"),
        );
    if active {
        let mut actions = div()
            .flex()
            .flex_row()
            .gap(theme::SPACE_3)
            .text_size(theme::TEXT_XS);
        if has_lines {
            actions = actions
                .child(action(
                    "copy-transcript",
                    if app.transcript_copied {
                        "Copied"
                    } else {
                        "Copy all"
                    },
                    true,
                    cx.listener(|this, _, _, _| this.copy_transcript()),
                ))
                .child(action(
                    "clear-transcript",
                    "Clear",
                    true,
                    cx.listener(|this, _, _, cx| this.clear(cx)),
                ));
        }
        if app.recording {
            actions = actions.child(action(
                "cancel-transcript",
                "Cancel",
                true,
                cx.listener(|this, _, _, cx| this.cancel(cx)),
            ));
        }
        head = head.child(actions);
    }
    panel = panel.child(head);

    if has_lines {
        let start = app.utterances.len().saturating_sub(4);
        let mut lines = div()
            .flex()
            .flex_col()
            .gap(theme::SPACE_2)
            .overflow_hidden();
        for (index, line) in app.utterances[start..].iter().enumerate() {
            let latest = start + index + 1 == app.utterances.len();
            let text = if line.streaming && line.cleaned.is_empty() {
                format!("{} …", line.raw)
            } else {
                line.text().to_string()
            };
            lines = lines.child(
                div()
                    .text_size(if latest {
                        theme::TEXT_XL
                    } else {
                        theme::TEXT_BASE
                    })
                    .text_color(if latest {
                        theme::text()
                    } else {
                        theme::text_muted()
                    })
                    .child(text),
            );
        }
        panel = panel.child(lines);
    } else {
        let title = if app.recording {
            "Listening — speak now"
        } else if matches!(app.status, RunState::Transcribing | RunState::Rewriting) {
            "Working on it…"
        } else {
            match app.hotkey_status {
                HotkeyStatus::RegisteredEvdev(_) | HotkeyStatus::RegisteredX11(_) => {
                    "Press the shortcut and speak"
                }
                _ => "Trigger dictation and speak",
            }
        };
        panel = panel
            .child(
                div()
                    .text_size(theme::TEXT_XL)
                    .text_color(theme::text_muted())
                    .child(title),
            )
            .child(
                div()
                    .mt(theme::SPACE_2)
                    .text_size(theme::TEXT_BASE)
                    .text_color(theme::text_muted())
                    .child("Your words appear here as you finish each sentence."),
            );
    }

    if active {
        panel = panel.child(session_strip(app));
    }
    panel.into_any_element()
}

fn action(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let color = if enabled {
        theme::text_secondary()
    } else {
        theme::text_faint()
    };
    div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(label)
        .text_color(color)
        .cursor_pointer()
        .child(label)
        .when(enabled, |this| this.on_click(listener))
        .into_any_element()
}

fn session_strip(app: &FloureApp) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .w_full()
        .mt(theme::SPACE_4)
        .pt(theme::SPACE_3)
        .border_t_1()
        .border_color(theme::border())
        .text_size(theme::TEXT_XS)
        .font_features(theme::tabular())
        .text_color(theme::text_muted())
        .child(format!(
            "{} words   {} WPM   {}",
            app.session.words,
            app.session.wpm(),
            app.session.elapsed()
        ))
        .child(format!("{} · CPU", asr_label(app)))
        .into_any_element()
}

fn asr_label(app: &FloureApp) -> &'static str {
    match app.config.asr_profile {
        floure_core::config::AsrProfile::Parakeet => "Parakeet",
        floure_core::config::AsrProfile::ParakeetUltra => "Parakeet Ultra",
        floure_core::config::AsrProfile::WhisperTurbo => "Whisper Turbo",
        floure_core::config::AsrProfile::WhisperBase => "Whisper Base",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wayland_fallback_is_visible_not_silent() {
        let (text, hint) =
            shortcut_display(&HotkeyStatus::Unavailable, "CommandOrControl+Shift+F12");
        assert_eq!(text, "floure --toggle");
        assert!(hint.is_some_and(|hint| hint.contains("Click to copy")));

        let (text, hint) = shortcut_display(&HotkeyStatus::Failed("in use".into()), "Alt+Space");
        assert_eq!(text, "Shortcut unavailable");
        assert!(hint.is_some_and(|hint| hint.contains("floure --toggle")));

        let (text, hint) = shortcut_display(
            &HotkeyStatus::RegisteredEvdev("Ctrl+Shift+F12".into()),
            "CommandOrControl+Shift+F12",
        );
        assert_eq!(text, "Ctrl + Shift + F12");
        assert!(hint.is_none());
        assert_eq!(
            shortcut_copy(&HotkeyStatus::RegisteredEvdev("Ctrl+Shift+F12".into())),
            "floure --toggle"
        );
    }

    /// The Wayland-without-input-group state must name the exact fix, not just
    /// say "unavailable".
    #[test]
    fn missing_input_group_names_the_command_and_the_relogin() {
        let (text, hint) =
            shortcut_display(&HotkeyStatus::NeedsInputGroup, "CommandOrControl+Shift+F12");
        assert!(text.contains("input group"));
        let hint = hint.expect("hint with the fix");
        assert!(hint.contains("sudo usermod -aG input $USER"));
        assert!(hint.contains("log out and back in"));
        assert_eq!(
            shortcut_copy(&HotkeyStatus::NeedsInputGroup),
            "sudo usermod -aG input $USER"
        );
    }
}
