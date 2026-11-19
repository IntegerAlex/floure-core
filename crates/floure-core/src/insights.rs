//! Aggregates for the Stats screen, computed from the local history DB.
//!
//! Every number is derived from saved transcripts. There is no synthetic or
//! seeded data: an empty database produces zeros and an empty heatmap.

use std::path::PathBuf;

use anyhow::Result;
use rusqlite::Connection;
use serde::Serialize;

const WORD_COUNT: &str = "(LENGTH(raw_text) - LENGTH(REPLACE(raw_text, ' ', '')) + 1)";

/// Monday 00:00 local, converted to UTC for comparison with stored
/// `created_at` values. `weekday 1` alone moves *forward* to Monday, so the
/// -6 days step makes it this week's Monday.
const WEEK_START: &str =
    "datetime('now', 'localtime', '-6 days', 'weekday 1', 'start of day', 'utc')";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsCategory {
    pub name: String,
    pub words: i64,
    pub max_words: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightsStreak {
    pub current: i64,
    pub longest: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeatmapDay {
    pub date: String,
    pub level: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekWords {
    pub label: String,
    pub words: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    pub wpm: i64,
    pub wpm_trend: i64,
    pub total_words: i64,
    pub words_this_week: i64,
    pub words_previous_week: i64,
    pub words_trend: i64,
    pub sessions: i64,
    pub previous_week_sessions: i64,
    pub ai_fixes: i64,
    pub categories: Vec<InsightsCategory>,
    pub streak: InsightsStreak,
    pub heatmap: Vec<HeatmapDay>,
    pub weeks: Vec<WeekWords>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceIntelligence {
    pub most_active_day: String,
    pub most_productive_hour: String,
    pub avg_dictation_length: String,
    pub most_used_language: String,
    pub most_active_day_words: i64,
    pub peak_voice_usage: String,
    pub per_utterance: String,
    pub language_percentage: i64,
}

pub struct InsightsRepo {
    db: PathBuf,
}

impl InsightsRepo {
    pub fn new(db: impl Into<PathBuf>) -> Self {
        Self { db: db.into() }
    }

    fn conn(&self) -> Result<Connection> {
        let conn = Connection::open(&self.db)?;
        crate::history::ensure_schema(&conn)?;
        Ok(conn)
    }

    pub fn summary(&self) -> Result<Insights> {
        let conn = self.conn()?;
        let week_start = WEEK_START;
        let one = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0) };

        let total_words = one(&format!(
            "SELECT COALESCE(SUM({WORD_COUNT}), 0) FROM transcripts WHERE raw_text != ''"
        ));
        let words_this_week = one(&format!(
            "SELECT COALESCE(SUM({WORD_COUNT}), 0) FROM transcripts
             WHERE raw_text != '' AND created_at >= {week_start}"
        ));
        let words_prev_week = one(&format!(
            "SELECT COALESCE(SUM({WORD_COUNT}), 0) FROM transcripts
             WHERE raw_text != '' AND created_at >= datetime({week_start}, '-7 days')
               AND created_at < {week_start}"
        ));
        let sessions = one("SELECT COUNT(*) FROM transcripts");
        let previous_week_sessions = one(&format!(
            "SELECT COUNT(*) FROM transcripts
             WHERE created_at >= datetime({week_start}, '-7 days') AND created_at < {week_start}"
        ));

        // WPM across the whole corpus: total words divided by total active
        // speech time. Averaging per-row ratios would over-weight short clips.
        let wpm = conn
            .query_row(
                &format!(
                    "SELECT CASE WHEN COALESCE(SUM(duration_sec), 0) > 0
                             THEN SUM({WORD_COUNT}) / (SUM(duration_sec) / 60.0)
                             ELSE 0 END
                     FROM transcripts WHERE raw_text != '' AND duration_sec > 0"
                ),
                [],
                |r| r.get::<_, f64>(0).map(|v| v as i64),
            )
            .unwrap_or(0);
        let wpm_prev = conn
            .query_row(
                &format!(
                    "SELECT CASE WHEN COALESCE(SUM(duration_sec), 0) > 0
                             THEN SUM({WORD_COUNT}) / (SUM(duration_sec) / 60.0)
                             ELSE 0 END
                     FROM transcripts WHERE raw_text != '' AND duration_sec > 0
                       AND created_at >= datetime('now', '-14 days')
                       AND created_at < datetime('now', '-7 days')"
                ),
                [],
                |r| r.get::<_, f64>(0).map(|v| v as i64),
            )
            .unwrap_or(0);

        let ai_fixes = one("SELECT COUNT(*) FROM transcripts
             WHERE processed_text != '' AND processed_text != raw_text AND mode != 'off'");

        let categories = categories(&conn, total_words)?;
        let streak = streaks(&conn)?;
        let heatmap = heatmap(&conn)?;
        let weeks = weekly_weeks(&conn)?;

        Ok(Insights {
            wpm,
            wpm_trend: trend(wpm, wpm_prev),
            total_words,
            words_this_week,
            words_previous_week: words_prev_week,
            words_trend: trend(words_this_week, words_prev_week),
            sessions,
            previous_week_sessions,
            ai_fixes,
            categories,
            streak,
            heatmap,
            weeks,
        })
    }

    pub fn voice_intelligence(&self) -> Result<VoiceIntelligence> {
        let conn = self.conn()?;

        let (most_active_day, most_active_day_words): (String, i64) = conn
            .query_row(
                &format!(
                    "SELECT CASE CAST(strftime('%w', created_at) AS INTEGER)
                       WHEN 0 THEN 'Sunday' WHEN 1 THEN 'Monday' WHEN 2 THEN 'Tuesday'
                       WHEN 3 THEN 'Wednesday' WHEN 4 THEN 'Thursday' WHEN 5 THEN 'Friday'
                       ELSE 'Saturday' END,
                       COALESCE(SUM({WORD_COUNT}), 0)
                     FROM transcripts WHERE raw_text != ''
                     GROUP BY strftime('%w', created_at) ORDER BY 2 DESC LIMIT 1"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("—".into(), 0));

        let (hour, peak_sessions): (i64, i64) = conn
            .query_row(
                "SELECT CAST(strftime('%H', created_at) AS INTEGER) AS h, COUNT(*) AS cnt
                 FROM transcripts GROUP BY h ORDER BY cnt DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));
        let hour_label = match hour {
            0 => "12 AM".to_string(),
            1..=11 => format!("{hour} AM"),
            12 => "12 PM".to_string(),
            13..=23 => format!("{} PM", hour - 12),
            _ => "—".to_string(),
        };

        let avg_duration: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(duration_sec), 0) FROM transcripts WHERE duration_sec > 0",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0.0);
        let avg_dictation = if avg_duration > 0.0 {
            if avg_duration < 60.0 {
                format!("{avg_duration:.0} seconds")
            } else {
                format!("{:.1} minutes", avg_duration / 60.0)
            }
        } else {
            "—".to_string()
        };

        let (most_used_language, lang_sessions): (String, i64) = conn
            .query_row(
                "SELECT language, COUNT(*) AS cnt FROM transcripts
                 WHERE language != '' GROUP BY language ORDER BY cnt DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or(("—".into(), 0));
        let total_sessions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transcripts WHERE language != ''",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let language_pct = if total_sessions > 0 {
            (lang_sessions as f64 / total_sessions as f64 * 100.0).round() as i64
        } else {
            0
        };

        Ok(VoiceIntelligence {
            most_active_day,
            most_productive_hour: hour_label,
            avg_dictation_length: avg_dictation,
            most_used_language,
            most_active_day_words,
            peak_voice_usage: format!("{peak_sessions} sessions"),
            per_utterance: if avg_duration > 0.0 {
                format!("Avg {avg_duration:.1}s")
            } else {
                "No data".to_string()
            },
            language_percentage: language_pct,
        })
    }
}

fn trend(now: i64, previous: i64) -> i64 {
    if previous > 0 {
        ((now - previous) as f64 / previous as f64 * 100.0).round() as i64
    } else {
        0
    }
}

fn categories(conn: &Connection, total_words: i64) -> Result<Vec<InsightsCategory>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT mode, COALESCE(SUM({WORD_COUNT}), 0) AS words FROM transcripts
         WHERE raw_text != '' AND mode != 'off' GROUP BY mode ORDER BY words DESC"
    ))?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let mut categories = Vec::new();
    for row in rows.flatten() {
        let (mode, words) = row;
        let name = match mode.as_str() {
            "cleanup" => "AI Prompts",
            "email" => "Emails",
            "bullet_list" => "Documents",
            "commit_message" => "Messages",
            _ => "Other",
        };
        categories.push(InsightsCategory {
            name: name.to_string(),
            words,
            max_words: total_words.max(1),
        });
    }
    let other: i64 = conn
        .query_row(
            &format!(
                "SELECT COALESCE(SUM({WORD_COUNT}), 0) FROM transcripts
                 WHERE raw_text != '' AND mode = 'off'"
            ),
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if other > 0 {
        if let Some(existing) = categories.iter_mut().find(|c| c.name == "Other") {
            existing.words += other;
        } else {
            categories.push(InsightsCategory {
                name: "Other".to_string(),
                words: other,
                max_words: total_words.max(1),
            });
        }
    }
    Ok(categories)
}

fn streaks(conn: &Connection) -> Result<InsightsStreak> {
    let dates: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT date(created_at) AS day FROM transcripts ORDER BY day DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    let days: std::collections::HashSet<&str> = dates.iter().map(String::as_str).collect();

    let mut current = 0i64;
    for offset in 0..2000i64 {
        let day: String = conn
            .query_row("SELECT date('now', '-' || ?1 || ' days')", [offset], |r| {
                r.get(0)
            })
            .unwrap_or_default();
        if days.contains(day.as_str()) {
            current += 1;
        } else {
            break;
        }
    }

    let mut longest = 0i64;
    let mut run = 1i64;
    for pair in dates.windows(2) {
        let diff: i64 = conn
            .query_row(
                "SELECT julianday(?1) - julianday(?2)",
                [&pair[0], &pair[1]],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if diff == 1 {
            run += 1;
        } else {
            longest = longest.max(run);
            run = 1;
        }
    }
    if !dates.is_empty() {
        longest = longest.max(run);
    }

    Ok(InsightsStreak { current, longest })
}

fn heatmap(conn: &Connection) -> Result<Vec<HeatmapDay>> {
    let mut activity = std::collections::HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT date(created_at) AS day, COUNT(*) AS cnt FROM transcripts
             WHERE created_at >= datetime('now', '-182 days') GROUP BY day",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;
        for row in rows.flatten() {
            activity.insert(row.0, row.1);
        }
    }
    let mut days = Vec::with_capacity(182);
    for offset in (0..182i64).rev() {
        let date: String = conn
            .query_row("SELECT date('now', '-' || ?1 || ' days')", [offset], |r| {
                r.get(0)
            })
            .unwrap_or_default();
        let count = activity.get(&date).copied().unwrap_or(0);
        days.push(HeatmapDay {
            date,
            level: heat_level(count),
        });
    }
    Ok(days)
}

/// Twelve calendar weeks, oldest first. Each bar is one Monday–Sunday week.
fn weekly_weeks(conn: &Connection) -> Result<Vec<WeekWords>> {
    let mut weeks = Vec::with_capacity(12);
    for offset in (0..12i64).rev() {
        let start: String = conn
            .query_row(
                &format!(
                    "SELECT date('now', 'localtime', '-6 days', 'weekday 1',
                             'start of day', '-{} days')",
                    offset * 7
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        let words: i64 = conn
            .query_row(
                &format!(
                    "SELECT COALESCE(SUM({WORD_COUNT}), 0) FROM transcripts
                     WHERE raw_text != ''
                       AND date(created_at, 'localtime') >= ?1
                       AND date(created_at, 'localtime') <= date(?1, '+6 days')"
                ),
                [&start],
                |r| r.get(0),
            )
            .unwrap_or(0);
        let label: String = conn
            .query_row("SELECT strftime('%m/%d', ?1)", [&start], |r| r.get(0))
            .unwrap_or_default();
        weeks.push(WeekWords { label, words });
    }
    Ok(weeks)
}

fn heat_level(count: i64) -> i64 {
    match count {
        0 => 0,
        1..=2 => 1,
        3..=5 => 2,
        6..=10 => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempDb(PathBuf);

    impl TempDb {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            Self(
                std::env::temp_dir().join(format!("floure-insights-{}-{n}.db", std::process::id())),
            )
        }

        fn insert(&self, text: &str, mode: &str, duration: f64) {
            let conn = Connection::open(&self.0).unwrap();
            crate::history::ensure_schema(&conn).unwrap();
            conn.execute(
                "INSERT INTO transcripts(raw_text, processed_text, language, mode, model, duration_sec)
                 VALUES (?1, ?1, 'en', ?2, 'parakeet', ?3)",
                params![text, mode, duration],
            )
            .unwrap();
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn empty_history_reports_zeros_and_full_heatmap() {
        let temp = TempDb::new();
        let summary = InsightsRepo::new(&temp.0).summary().unwrap();
        assert_eq!(summary.total_words, 0);
        assert_eq!(summary.wpm, 0);
        assert_eq!(summary.heatmap.len(), 182);
        assert_eq!(summary.weeks.len(), 12);
        assert_eq!(summary.sessions, 0);
        assert_eq!(summary.streak.longest, 0);
    }

    #[test]
    fn words_and_wpm_come_from_saved_rows() {
        let temp = TempDb::new();
        temp.insert("one two three four", "cleanup", 4.0);
        temp.insert("five six", "off", 0.0);
        let summary = InsightsRepo::new(&temp.0).summary().unwrap();
        assert_eq!(summary.total_words, 6);
        assert_eq!(summary.words_this_week, 6);
        // Only the row with a duration participates: 4 words / (4s / 60) = 60.
        assert_eq!(summary.wpm, 60);
        assert_eq!(summary.streak.current, 1);
        assert_eq!(summary.sessions, 2);
        assert!(summary.heatmap.last().is_some_and(|d| d.level == 1));
        assert!(
            summary.weeks.last().is_some_and(|w| w.words >= 6),
            "this week's bar must include today's rows"
        );
    }

    #[test]
    fn week_boundary_starts_monday_and_excludes_older_rows() {
        let temp = TempDb::new();
        temp.insert("this week", "cleanup", 1.0);
        // Eight days back is always before this week's Monday.
        let conn = Connection::open(&temp.0).unwrap();
        crate::history::ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO transcripts(raw_text, processed_text, language, mode, model, duration_sec, created_at)
             VALUES ('old row', 'old row', 'en', 'cleanup', 'parakeet', 1.0, datetime('now', '-8 days'))",
            [],
        )
        .unwrap();
        let summary = InsightsRepo::new(&temp.0).summary().unwrap();
        assert_eq!(summary.total_words, 4);
        assert_eq!(summary.words_this_week, 2);
        assert_eq!(summary.previous_week_sessions, 1);
    }

    #[test]
    fn heatmap_levels_are_discrete() {
        for (count, level) in [
            (0, 0),
            (1, 1),
            (2, 1),
            (3, 2),
            (5, 2),
            (6, 3),
            (10, 3),
            (11, 4),
        ] {
            assert_eq!(heat_level(count), level, "count {count}");
        }
    }

    #[test]
    fn trend_is_zero_without_a_previous_week() {
        let temp = TempDb::new();
        temp.insert("one two", "cleanup", 1.0);
        let summary = InsightsRepo::new(&temp.0).summary().unwrap();
        assert_eq!(summary.words_trend, 0);
        assert_eq!(summary.wpm_trend, 0);
    }
}
