//! Stats: how much this is actually used. Every number comes from the DB.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px, rgba};

use crate::app::FloureApp;
use crate::theme;
use floure_core::Insights;

pub fn render(app: &FloureApp, _cx: &mut Context<FloureApp>) -> AnyElement {
    let Some(insights) = &app.insights else {
        return div()
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .text_color(theme::text_muted())
            .child("Stats are unavailable right now.")
            .into_any_element();
    };

    div()
        .flex()
        .flex_col()
        .flex_1()
        .px(theme::GUTTER)
        .pt(theme::SPACE_8)
        .pb(theme::SPACE_6)
        .gap(theme::SPACE_8)
        .child(
            div()
                .text_size(theme::TEXT_2XL)
                .font_weight(theme::semibold())
                .child("Stats"),
        )
        .child(figures(insights))
        .child(streak_sentence(insights))
        .child(heatmap(insights))
        .child(weekly_bars(insights))
        .into_any_element()
}

fn figures(insights: &Insights) -> AnyElement {
    let items = [
        ("WORDS THIS WEEK", insights.words_this_week.to_string()),
        ("TOTAL WORDS", insights.total_words.to_string()),
        ("CURRENT STREAK", format!("{} d", insights.streak.current)),
        ("AVERAGE WPM", insights.wpm.to_string()),
    ];
    let mut row = div().flex().flex_row().gap(theme::SPACE_8).w_full();
    for (label, value) in items {
        row = row.child(
            div()
                .flex()
                .flex_col()
                .gap(theme::SPACE_1)
                .min_w(px(120.0))
                .child(
                    div()
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_faint())
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_2XL)
                        .font_weight(theme::semibold())
                        .child(value),
                ),
        );
    }
    row.into_any_element()
}

fn streak_sentence(insights: &Insights) -> AnyElement {
    let sentence = if insights.total_words == 0 {
        "Your first session will appear here.".to_string()
    } else if insights.words_this_week == 0 {
        "Nothing dictated this week yet.".to_string()
    } else {
        let sessions = insights.heatmap.iter().filter(|d| d.level > 0).count();
        format!(
            "{} {} this week across {} {}.",
            insights.words_this_week,
            if insights.words_this_week == 1 {
                "word"
            } else {
                "words"
            },
            sessions,
            if sessions == 1 { "session" } else { "sessions" }
        )
    };
    div()
        .text_size(theme::TEXT_BASE)
        .text_color(theme::text_secondary())
        .child(sentence)
        .into_any_element()
}

fn heatmap(insights: &Insights) -> AnyElement {
    // 12 weeks, newest last. Seven rows per column, Monday-first is not
    // guaranteed by the data source; the grid is chronological.
    let days: Vec<_> = insights.heatmap.iter().rev().take(84).rev().collect();
    let mut grid = div().flex().flex_row().gap(px(3.0));
    for week in days.chunks(7) {
        let mut column = div().flex().flex_col().gap(px(3.0));
        for day in week {
            let color = match day.level {
                0 => theme::raised(),
                1 => rgba(0xFFFFFF2E),
                2 => rgba(0xFFFFFF5C),
                3 => rgba(0xFFFFFF99),
                _ => rgba(0xFFFFFFE6),
            };
            let mut cell = div().size(px(10.0)).rounded(px(2.0)).bg(color);
            if days.last().is_some_and(|last| day.date == last.date) {
                cell = cell.border_1().border_color(theme::text());
            }
            column = column.child(cell);
        }
        grid = grid.child(column);
    }
    grid.into_any_element()
}

fn weekly_bars(insights: &Insights) -> AnyElement {
    let max = insights
        .weekly_words
        .iter()
        .map(|d| d.words)
        .max()
        .unwrap_or(0)
        .max(1);
    let mut row = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(theme::SPACE_3)
        .h(px(96.0));
    for day in &insights.weekly_words {
        let height = ((day.words as f32 / max as f32) * 88.0).max(2.0);
        row = row.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(theme::SPACE_1)
                .child(
                    div()
                        .w(px(18.0))
                        .h(px(height))
                        .rounded(px(2.0))
                        .bg(theme::text_secondary()),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_faint())
                        .child(day.label.clone()),
                ),
        );
    }
    row.into_any_element()
}
