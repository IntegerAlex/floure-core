//! First-run setup: microphone, model, then the trigger. Same shell, no modal
//! stack; Back works on every step.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};

use crate::app::FloureApp;
use crate::theme;

pub fn render(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(theme::GUTTER)
        .pt(theme::SPACE_8)
        .pb(theme::SPACE_6)
        .max_w(px(720.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(theme::SPACE_2)
                .child(
                    div()
                        .text_size(theme::TEXT_2XL)
                        .font_weight(theme::semibold())
                        .child("Set up Floure"),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_SM)
                        .text_color(theme::text_faint())
                        .child(format!("Step {} of 3", app.onboarding_step + 1)),
                ),
        )
        .child(div().flex().flex_col().flex_1().mt(theme::SPACE_6).child(
            match app.onboarding_step {
                0 => microphone(app, cx),
                1 => models(app, cx),
                _ => ready(app, cx),
            },
        ))
        .child(navigation(app, cx))
        .into_any_element()
}

fn microphone(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut list = div().flex().flex_col().child(heading(
        "Choose your microphone",
        "Speak while the meter moves. Nothing is transcribed or saved here.",
    ));
    list = list.child(
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(theme::SPACE_3)
            .py(theme::SPACE_3)
            .child(meter(app))
            .child(button(
                "onboarding-mic-test",
                if app.mic_testing {
                    "Stop test"
                } else {
                    "Test mic"
                },
                cx.listener(|this, _, _, cx| this.toggle_mic_test(cx)),
            )),
    );
    list = list.child(device_row(
        "System default",
        app.config.selected_mic_id.is_none(),
        String::new(),
        cx,
    ));
    for (name, id) in &app.mic_devices {
        list = list.child(device_row(
            name,
            app.config.selected_mic_id.as_deref() == Some(id.as_str()),
            id.clone(),
            cx,
        ));
    }
    list.into_any_element()
}

fn models(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut list = div().flex().flex_col().child(heading(
        "Download a speech model",
        "Parakeet is recommended. It runs locally; audio never leaves this machine.",
    ));
    for model in &app.model_statuses {
        let id = model.id.clone();
        let progress = app.model_progress.get(&id).copied();
        let control: AnyElement = if let Some(percent) = progress {
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_secondary())
                .child(format!("{percent}%"))
                .into_any_element()
        } else if model.downloaded {
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_muted())
                .child("Downloaded")
                .into_any_element()
        } else {
            button(
                "onboarding-download",
                "Download",
                cx.listener(move |this, _, _, cx| {
                    this.download_model(&id, cx);
                }),
            )
        };
        list = list.child(
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
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(theme::TEXT_BASE)
                                .text_color(theme::text())
                                .child(model.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(theme::TEXT_XS)
                                .text_color(theme::text_faint())
                                .child(format!(
                                    "{}{}",
                                    model.backend,
                                    if model.recommended {
                                        " · Recommended"
                                    } else {
                                        ""
                                    }
                                )),
                        ),
                )
                .child(control),
        );
    }
    if !app.models_ready() {
        list = list.child(
            div()
                .mt(theme::SPACE_3)
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_muted())
                .child("At least one speech model must finish downloading to continue."),
        );
    }
    list.into_any_element()
}

fn ready(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let on_wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
    let mut list = div().flex().flex_col().child(heading(
        "You are ready",
        "Hold the shortcut (or tap, in toggle mode) and speak. Floure types into whatever window had focus.",
    ));
    list = list.child(row(
        "Shortcut",
        if on_wayland {
            "No global shortcut on Wayland".to_string()
        } else {
            crate::app::display_hotkey(&app.config.hotkey)
        },
    ));
    list = list.child(row("Compositor trigger", "floure --toggle".to_string()));
    if on_wayland {
        list = list.child(
            div()
                .mt(theme::SPACE_3)
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_faint())
                .child("Add `bind = SUPER, D, exec, floure --toggle` to your Hyprland config, or use the tray."),
        );
    }
    list = list.child(div().mt(theme::SPACE_6).child(button(
        "start-dictating",
        "Start dictating",
        cx.listener(|this, _, _, cx| this.complete_onboarding(cx)),
    )));
    list.into_any_element()
}

fn navigation(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .mt(theme::SPACE_6);
    row = row.child(if app.onboarding_step > 0 {
        button(
            "onboarding-back",
            "Back",
            cx.listener(|this, _, _, cx| this.onboarding_back(cx)),
        )
    } else {
        div().into_any_element()
    });
    if app.onboarding_step < 2 {
        row = row.child(button(
            "onboarding-next",
            "Continue",
            cx.listener(|this, _, _, cx| this.onboarding_next(cx)),
        ));
    }
    row.into_any_element()
}

fn heading(title: &str, hint: &str) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(theme::SPACE_1)
        .child(
            div()
                .text_size(theme::TEXT_LG)
                .font_weight(theme::semibold())
                .child(title.to_string()),
        )
        .child(
            div()
                .text_size(theme::TEXT_SM)
                .text_color(theme::text_muted())
                .child(hint.to_string()),
        )
        .into_any_element()
}

fn device_row(name: &str, selected: bool, id: String, cx: &mut Context<FloureApp>) -> AnyElement {
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
                .child(name.to_string()),
        )
        .child(button(
            "onboarding-device",
            if selected { "Selected" } else { "Use" },
            cx.listener(move |this, _, _, cx| {
                this.set_mic(id.clone());
                cx.notify();
            }),
        ))
        .into_any_element()
}

fn row(label: &str, value: String) -> AnyElement {
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
                .child(value),
        )
        .into_any_element()
}

fn button(
    id: &'static str,
    label: &str,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(SharedString::from(id))
        .role(gpui::Role::Button)
        .aria_label(label.to_string())
        .px(theme::SPACE_3)
        .h(px(30.0))
        .flex()
        .items_center()
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .text_size(theme::TEXT_SM)
        .text_color(theme::text_secondary())
        .cursor_pointer()
        .hover(|style| style.bg(theme::raised()))
        .on_click(listener)
        .child(label.to_string())
        .into_any_element()
}

fn meter(app: &FloureApp) -> AnyElement {
    let level = if app.mic_testing { app.mic_level } else { 0.0 };
    div()
        .w(px(120.0))
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(theme::raised())
        .child(
            div()
                .h_full()
                .w(px(120.0 * level.clamp(0.0, 1.0)))
                .rounded(px(2.0))
                .bg(theme::text_secondary()),
        )
        .into_any_element()
}
