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
        ("SESSIONS", insights.sessions.to_string()),
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
                        .font_features(theme::tabular())
                        .child(value),
                ),
        );
    }
    row.into_any_element()
}

fn streak_sentence(insights: &Insights) -> AnyElement {
    div()
        .text_size(theme::TEXT_BASE)
        .text_color(theme::text_secondary())
        .child(finding_sentence(insights))
        .into_any_element()
}

pub fn finding_sentence(insights: &Insights) -> String {
    finding_text(
        insights.total_words,
        insights.words_this_week,
        insights.words_previous_week,
        insights.previous_week_sessions,
    )
}

/// A comparison needs at least three prior-week sessions; below that the
/// percentage is noise and the sentence stays neutral.
fn finding_text(
    total_words: i64,
    words_this_week: i64,
    words_previous_week: i64,
    previous_week_sessions: i64,
) -> String {
    if total_words == 0 {
        return "Your first session will appear here.".to_string();
    }
    if words_this_week == 0 {
        return "Nothing dictated this week yet.".to_string();
    }
    let words = format!(
        "{} {}",
        words_this_week,
        if words_this_week == 1 {
            "word"
        } else {
            "words"
        }
    );
    if previous_week_sessions >= 3 && words_previous_week > 0 {
        let delta =
            (words_this_week - words_previous_week) as f64 / words_previous_week as f64 * 100.0;
        let direction = if delta >= 0.0 { "more" } else { "less" };
        return format!(
            "You dictated {words} this week, {:.0}% {direction} than last week.",
            delta.abs()
        );
    }
    format!("You dictated {words} this week.")
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
    let raw_max = insights.weeks.iter().map(|d| d.words).max().unwrap_or(0);
    let max = raw_max.max(1);
    let mut row = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(theme::SPACE_2)
        .h(px(104.0));
    for week in &insights.weeks {
        let height = ((week.words as f32 / max as f32) * 88.0).max(2.0);
        row = row.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(theme::SPACE_1)
                .w(px(28.0))
                .flex_shrink_0()
                .child(
                    div()
                        .w_full()
                        .h(px(height))
                        .rounded(px(2.0))
                        .bg(theme::text_secondary()),
                )
                .child(
                    div()
                        .text_size(theme::TEXT_XS)
                        .text_color(theme::text_faint())
                        .child(week.label.clone()),
                ),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(theme::SPACE_2)
        .child(
            div()
                .text_size(theme::TEXT_XS)
                .text_color(theme::text_faint())
                .child(format!("12 weeks · max {raw_max} words")),
        )
        .child(row)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_requires_three_prior_sessions() {
        assert_eq!(
            finding_text(200, 120, 100, 2),
            "You dictated 120 words this week."
        );
        assert!(finding_text(200, 120, 100, 3).contains("20% more"));
        assert!(finding_text(200, 80, 100, 3).contains("20% less"));
    }

    #[test]
    fn finding_sentence_handles_empty_and_zero_weeks() {
        assert_eq!(
            finding_text(0, 0, 0, 0),
            "Your first session will appear here."
        );
        assert_eq!(
            finding_text(50, 0, 10, 3),
            "Nothing dictated this week yet."
        );
        assert_eq!(finding_text(50, 1, 0, 0), "You dictated 1 word this week.");
    }
}
