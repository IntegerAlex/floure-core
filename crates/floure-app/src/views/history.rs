//! History: what was said, when. Search is FTS-backed; every action operates
//! on the filtered rows the screen actually shows.

use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, prelude::*, px,
};
use gpui_component::input::Input;

use crate::app::FloureApp;
use crate::theme;
use floure_core::History;

const MODES: [(&str, &str); 5] = [
    ("cleanup", "Cleanup"),
    ("email", "Email"),
    ("bullet_list", "Bullets"),
    ("commit_message", "Commit"),
    ("off", "Raw"),
];

pub fn render(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(theme::GUTTER)
        .pt(theme::SPACE_8)
        .pb(theme::SPACE_6)
        .child(header(app, cx))
        .child(filters(app, cx))
        .child(selection_bar(app, cx))
        .child(rows(app, cx))
        .child(footer(app, cx))
        .into_any_element()
}

fn header(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(theme::SPACE_4)
        .child(
            div()
                .flex()
                .flex_row()
                .items_baseline()
                .gap(theme::SPACE_3)
                .child(
                    div()
                        .text_size(theme::TEXT_2XL)
                        .font_weight(theme::semibold())
                        .child("History"),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_SM)
                        .text_color(theme::text_faint())
                        .child(format!(
                            "{} {}",
                            app.history.len(),
                            if app.history.len() == 1 {
                                "transcript"
                            } else {
                                "transcripts"
                            }
                        )),
                ),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(theme::SPACE_2)
                .child(
                    div()
                        .h(px(32.0))
                        .w(px(260.0))
                        .px(theme::SPACE_3)
                        .flex()
                        .items_center()
                        .rounded(theme::RADIUS_CONTROL)
                        .border_1()
                        .border_color(theme::border())
                        .bg(theme::surface())
                        .child(Input::new(&app.search_input).appearance(false)),
                )
                .child(button(
                    "history-export",
                    "Export",
                    app.history.is_empty(),
                    cx.listener(|this, _, _, cx| {
                        this.export_history();
                        cx.notify();
                    }),
                )),
        )
        .into_any_element()
}

fn filters(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(theme::SPACE_2)
        .mt(theme::SPACE_4);
    row = row.child(chip(
        "history-favorites",
        "Favorites",
        app.history_favorites_only,
        cx.listener(|this, _, _, cx| {
            this.toggle_favorite_filter();
            cx.notify();
        }),
    ));
    for (mode, label) in MODES {
        row = row.child(chip(
            match mode {
                "cleanup" => "mode-cleanup",
                "email" => "mode-email",
                "bullet_list" => "mode-bullets",
                "commit_message" => "mode-commit",
                _ => "mode-raw",
            },
            label,
            app.history_mode == Some(mode),
            cx.listener(move |this, _, _, cx| {
                this.toggle_history_mode(Some(mode));
                cx.notify();
            }),
        ));
    }
    row.into_any_element()
}

fn selection_bar(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    if app.history_selected.is_empty() {
        return div().into_any_element();
    }
    let count = app.history_selected.len();
    let confirming = app.history_confirm_delete;
    let mut bar = div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .mt(theme::SPACE_3)
        .px(theme::SPACE_3)
        .py(theme::SPACE_2)
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .text_size(theme::TEXT_SM)
        .text_color(theme::text_secondary())
        .child(format!(
            "{count} selected{}",
            if confirming == Some(count) {
                " — delete permanently?"
            } else {
                ""
            }
        ));

    if confirming == Some(count) {
        bar = bar.child(
            div()
                .flex()
                .flex_row()
                .gap(theme::SPACE_3)
                .child(button(
                    "delete-cancel",
                    "Cancel",
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.history_confirm_delete = None;
                        cx.notify();
                    }),
                ))
                .child(button(
                    "delete-confirm",
                    "Delete",
                    false,
                    cx.listener(|this, _, _, cx| {
                        this.confirm_delete_selected();
                        cx.notify();
                    }),
                )),
        );
    } else {
        bar = bar.child(button(
            "delete-selected",
            &format!("Delete {count}"),
            false,
            cx.listener(move |this, _, _, cx| {
                this.history_confirm_delete = Some(count);
                cx.notify();
            }),
        ));
    }
    bar.into_any_element()
}

fn rows(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    if let Some(error) = &app.history_error {
        return message(error, theme::error());
    }
    if app.history.is_empty() {
        if app.history_query.trim().is_empty() {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(theme::SPACE_2)
                .py(theme::SPACE_12)
                .child(
                    div()
                        .text_size(theme::TEXT_BASE)
                        .text_color(theme::text_muted())
                        .child("No transcripts yet"),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_SM)
                        .text_color(theme::text_faint())
                        .child("Start from your shortcut, the tray, or `floure --toggle`."),
                )
                .into_any_element();
        }
        return div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(theme::SPACE_3)
            .py(theme::SPACE_12)
            .child(
                div()
                    .text_size(theme::TEXT_BASE)
                    .text_color(theme::text_muted())
                    .child(format!(
                        "No transcripts match “{}”",
                        app.history_query.trim()
                    )),
            )
            .child(button(
                "clear-search",
                "Clear search",
                false,
                cx.listener(|this, _, window, cx| {
                    this.search_input
                        .update(cx, |state, cx| state.set_value("", window, cx));
                    this.history_query.clear();
                    this.history_limit = 50;
                    this.refresh_history();
                    cx.notify();
                }),
            ))
            .into_any_element();
    }

    let (today, yesterday) =
        History::new(floure_core::config::history_db_path()).today_and_yesterday();
    let mut list = div()
        .flex()
        .flex_col()
        .flex_1()
        .mt(theme::SPACE_4)
        .overflow_hidden();
    let mut current_day = String::new();
    for row in &app.history {
        let day = row
            .created_at
            .get(..10)
            .unwrap_or(&row.created_at)
            .to_string();
        if day != current_day {
            current_day = day.clone();
            let label = day_label(&day, &today, &yesterday);
            list = list.child(
                div()
                    .pt(theme::SPACE_3)
                    .pb(theme::SPACE_1)
                    .text_size(theme::TEXT_XS)
                    .text_color(theme::text_faint())
                    .child(label),
            );
        }
        list = list.child(row_view(app, row, cx));
    }
    list.into_any_element()
}

fn row_view(
    app: &FloureApp,
    row: &floure_core::TranscriptRow,
    cx: &mut Context<FloureApp>,
) -> AnyElement {
    let id = row.id;
    let selected = app.history_selected.contains(&id);
    let expanded = app.history_expanded.contains(&id);
    let favorite = row.favorite != 0;
    let text = if row.processed_text.trim().is_empty() {
        row.raw_text.clone()
    } else {
        row.processed_text.clone()
    };
    let time = row.created_at.get(11..16).unwrap_or("").to_string();
    let can_expand = row.raw_text != row.processed_text;

    let mut body = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(2.0))
        .child(
            div()
                .text_size(theme::TEXT_BASE)
                .text_color(theme::text())
                .child(text.clone()),
        );
    if expanded && can_expand {
        body = body.child(
            div()
                .mt(theme::SPACE_1)
                .text_size(theme::TEXT_SM)
                .text_color(theme::text_muted())
                .child(row.raw_text.clone()),
        );
    }

    div()
        .id(SharedString::from(format!("history-row-{id}")))
        .flex()
        .flex_row()
        .items_start()
        .gap(theme::SPACE_3)
        .py(theme::SPACE_3)
        .border_b_1()
        .border_color(theme::border())
        .child(checkbox(
            selected,
            cx.listener(move |this, _, _, cx| {
                this.toggle_selected(id);
                cx.notify();
            }),
        ))
        .child(
            div()
                .w(px(48.0))
                .flex_shrink_0()
                .text_size(theme::TEXT_XS)
                .font_features(theme::tabular())
                .text_color(theme::text_faint())
                .child(time),
        )
        .child(body)
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(theme::SPACE_3)
                .flex_shrink_0()
                .child(text_action(
                    SharedString::from(format!("favorite-{id}")),
                    if favorite { "Starred" } else { "Star" },
                    if favorite {
                        theme::text()
                    } else {
                        theme::text_faint()
                    },
                    cx.listener(move |this, _, _, cx| {
                        this.set_favorite(id, !favorite);
                        cx.notify();
                    }),
                ))
                .child(text_action(
                    SharedString::from(format!("copy-{id}")),
                    "Copy",
                    theme::text_faint(),
                    cx.listener(move |this, _, _, cx| {
                        this.copy_row(&text);
                        cx.notify();
                    }),
                ))
                .when(can_expand, |this| {
                    this.child(text_action(
                        SharedString::from(format!("expand-{id}")),
                        if expanded { "Clean" } else { "Raw" },
                        theme::text_faint(),
                        cx.listener(move |this, _, _, cx| {
                            this.toggle_expanded(id);
                            cx.notify();
                        }),
                    ))
                })
                .child(text_action(
                    SharedString::from(format!("delete-{id}")),
                    "Delete",
                    theme::text_faint(),
                    cx.listener(move |this, _, _, cx| {
                        this.delete_history_row(id);
                        cx.notify();
                    }),
                )),
        )
        .into_any_element()
}

fn footer(app: &FloureApp, cx: &mut Context<FloureApp>) -> AnyElement {
    let mut footer = div()
        .flex()
        .flex_col()
        .gap(theme::SPACE_2)
        .mt(theme::SPACE_3);
    if let Some(notice) = &app.history_notice {
        footer = footer.child(
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_muted())
                .child(notice.clone()),
        );
    }
    if app.history.len() >= app.history_limit {
        footer = footer.child(button(
            "history-load-more",
            "Load more",
            false,
            cx.listener(|this, _, _, cx| {
                this.load_more_history();
                cx.notify();
            }),
        ));
    }
    footer.into_any_element()
}

fn message(text: &str, color: gpui::Rgba) -> AnyElement {
    div()
        .py(theme::SPACE_12)
        .text_size(theme::TEXT_BASE)
        .text_color(color)
        .child(text.to_string())
        .into_any_element()
}

fn chip(
    id: &'static str,
    label: &'static str,
    active: bool,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let mut chip = div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(label)
        .px(theme::SPACE_3)
        .h(px(28.0))
        .flex()
        .items_center()
        .rounded(px(9999.0))
        .border_1()
        .border_color(if active {
            theme::text_faint()
        } else {
            theme::border()
        })
        .text_size(theme::TEXT_XS)
        .cursor_pointer()
        .text_color(if active {
            theme::text()
        } else {
            theme::text_muted()
        })
        .on_click(listener);
    if active {
        chip = chip.bg(theme::raised());
    }
    chip.child(label).into_any_element()
}

fn checkbox(
    checked: bool,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id("row-checkbox")
        .role(gpui::Role::CheckBox)
        .aria_label("Select transcript")
        .mt(px(2.0))
        .size(px(16.0))
        .flex_shrink_0()
        .rounded(px(4.0))
        .border_1()
        .border_color(if checked {
            theme::text()
        } else {
            theme::border()
        })
        .bg(if checked {
            theme::text()
        } else {
            theme::ground()
        })
        .cursor_pointer()
        .on_click(listener)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::ground())
                .child(if checked { "✓" } else { "" }),
        )
        .into_any_element()
}

fn text_action(
    id: SharedString,
    label: &'static str,
    color: gpui::Rgba,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(label)
        .px(px(6.0))
        .py(px(2.0))
        .rounded(px(5.0))
        .text_size(theme::TEXT_XS)
        .text_color(color)
        .cursor_pointer()
        .hover(|style| style.bg(theme::raised()).text_color(theme::text()))
        .on_click(listener)
        .child(label)
        .into_any_element()
}

fn button(
    id: &'static str,
    label: &str,
    disabled: bool,
    listener: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .role(gpui::Role::Button)
        .aria_label(label.to_string())
        .px(theme::SPACE_3)
        .h(px(28.0))
        .flex()
        .items_center()
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .text_size(theme::TEXT_XS)
        .text_color(if disabled {
            theme::text_faint()
        } else {
            theme::text_secondary()
        })
        .cursor_pointer()
        .when(!disabled, |this| this.on_click(listener))
        .child(label.to_string())
        .into_any_element()
}

/// Date heading for a UTC timestamp's date part, with local Today/Yesterday.
pub fn day_label(day: &str, today: &str, yesterday: &str) -> String {
    if day == today {
        "Today".to_string()
    } else if day == yesterday {
        "Yesterday".to_string()
    } else {
        day.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_headings_use_local_today_and_yesterday() {
        assert_eq!(day_label("2026-11-06", "2026-11-06", "2026-11-05"), "Today");
        assert_eq!(
            day_label("2026-11-05", "2026-11-06", "2026-11-05"),
            "Yesterday"
        );
        assert_eq!(
            day_label("2026-11-03", "2026-11-06", "2026-11-05"),
            "2026-11-03"
        );
    }
}
