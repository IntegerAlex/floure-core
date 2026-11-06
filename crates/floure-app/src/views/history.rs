//! History: what was said, when.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};

use crate::app::FloureApp;
use crate::theme;

pub fn render(app: &FloureApp, _cx: &mut Context<FloureApp>) -> AnyElement {
    let mut list = div().flex().flex_col().w_full().mt(theme::SPACE_4);

    if let Some(error) = &app.history_error {
        list = list.child(
            div()
                .py(theme::SPACE_4)
                .text_size(theme::TEXT_SM)
                .text_color(theme::error())
                .child(error.clone()),
        );
    } else if app.history.is_empty() {
        list = list.child(
            div()
                .py(theme::SPACE_12)
                .text_size(theme::TEXT_BASE)
                .text_color(theme::text_muted())
                .child("No transcripts yet. Your first dictation will appear here."),
        );
    } else {
        for row in &app.history {
            let text = if row.processed_text.trim().is_empty() {
                row.raw_text.clone()
            } else {
                row.processed_text.clone()
            };
            list = list.child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap(theme::SPACE_6)
                    .py(theme::SPACE_4)
                    .border_b_1()
                    .border_color(theme::border())
                    .child(
                        div()
                            .w(px(96.0))
                            .flex_shrink_0()
                            .text_size(theme::TEXT_XS)
                            .text_color(theme::text_faint())
                            .child(row.created_at.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(theme::TEXT_BASE)
                            .text_color(theme::text())
                            .child(text),
                    )
                    .child(
                        div()
                            .text_size(theme::TEXT_XS)
                            .text_color(theme::text_faint())
                            .child(row.mode.clone()),
                    ),
            );
        }
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
                .flex_row()
                .items_center()
                .justify_between()
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
                .child(search_field()),
        )
        .child(list)
        .into_any_element()
}

fn search_field() -> AnyElement {
    div()
        .w(px(240.0))
        .h(px(32.0))
        .px(theme::SPACE_3)
        .flex()
        .items_center()
        .rounded(theme::RADIUS_CONTROL)
        .border_1()
        .border_color(theme::border())
        .bg(theme::surface())
        .text_size(theme::TEXT_SM)
        .text_color(theme::text_faint())
        .child("Search transcripts")
        .into_any_element()
}
