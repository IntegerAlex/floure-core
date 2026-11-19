//! Idempotent data migrations run once per launch.
//!
//! Two things move forward:
//! 1. `history.db` gets the external-content FTS index and backfill.
//! 2. Values that only ever lived in the Tauri webview's localStorage are
//!    folded into `config.json` from a one-shot `ui-state.json` written by the
//!    final Tauri release.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::config::{AppConfig, PttMode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// The FTS index exists and was verified after the backfill.
    pub fts_ready: bool,
    /// A `ui-state.json` was found and its values were merged.
    pub ui_state_applied: bool,
    /// The bridge file was renamed so the next launch skips it.
    pub ui_state_retired: bool,
}

#[derive(Debug, Default, Deserialize)]
struct UiState {
    #[serde(default)]
    hotkey: Option<String>,
    #[serde(default)]
    ptt_mode: Option<PttMode>,
    #[serde(default)]
    sound_enabled: Option<bool>,
    #[serde(default)]
    sound_volume: Option<f32>,
    #[serde(default)]
    onboarding_completed: Option<bool>,
}

/// Run every migration. Safe to call on every launch: each step is keyed on
/// data that disappears or becomes a no-op once applied.
pub fn migrate(db_path: &Path, config_path: &Path) -> Result<MigrationReport> {
    let conn = crate::history::open(db_path)?;
    // `open` created the FTS table and backfilled it; a successful query
    // proves the index is usable, not just declared.
    let fts_ready = conn
        .query_row("SELECT count(*) FROM transcripts_fts", [], |r| {
            r.get::<_, i64>(0)
        })
        .is_ok();
    drop(conn);

    let (ui_state_applied, ui_state_retired) = migrate_ui_state(config_path)?;

    Ok(MigrationReport {
        fts_ready,
        ui_state_applied,
        ui_state_retired,
    })
}

fn ui_state_path(config_path: &Path) -> PathBuf {
    config_path.with_file_name("ui-state.json")
}

fn migrate_ui_state(config_path: &Path) -> Result<(bool, bool)> {
    let bridge = ui_state_path(config_path);
    if !bridge.exists() {
        return Ok((false, false));
    }
    let raw = std::fs::read_to_string(&bridge)?;
    let state: UiState = match serde_json::from_str(&raw) {
        Ok(state) => state,
        Err(error) => {
            // A corrupt bridge must not block startup, and must not be
            // silently deleted either: keep it for a bug report.
            eprintln!(
                "[migrate] ignoring unreadable {}: {error}",
                bridge.display()
            );
            return Ok((false, false));
        }
    };

    let mut config = AppConfig::load_at(config_path);
    if let Some(hotkey) = state.hotkey.filter(|h| !h.trim().is_empty()) {
        config.hotkey = hotkey.trim().to_string();
    }
    if let Some(mode) = state.ptt_mode {
        config.ptt_mode = mode;
    }
    if let Some(enabled) = state.sound_enabled {
        config.sound_enabled = enabled;
    }
    if let Some(volume) = state.sound_volume.filter(|v| v.is_finite()) {
        config.sound_volume = volume.clamp(0.0, 1.0);
    }
    if let Some(done) = state.onboarding_completed {
        config.onboarding_completed = done;
    }
    config.save_at(config_path)?;

    let retired = bridge.with_extension("json.migrated");
    if retired.exists() {
        let _ = std::fs::remove_file(&retired);
    }
    std::fs::rename(&bridge, &retired)?;
    Ok((true, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = NEXT.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("floure-migrate-{}-{n}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn config(&self) -> PathBuf {
            self.0.join("config.json")
        }

        fn db(&self) -> PathBuf {
            self.0.join("history.db")
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn bridge(dir: &TempDir, json: &str) {
        std::fs::write(dir.0.join("ui-state.json"), json).unwrap();
    }

    #[test]
    fn ui_state_is_merged_then_retired() {
        let dir = TempDir::new();
        bridge(
            &dir,
            r#"{
                "hotkey": "Alt+Space",
                "ptt_mode": "toggle",
                "sound_enabled": false,
                "sound_volume": 0.4,
                "onboarding_completed": true
            }"#,
        );

        let report = migrate(&dir.db(), &dir.config()).unwrap();
        assert!(report.fts_ready);
        assert!(report.ui_state_applied);
        assert!(report.ui_state_retired);

        let config = AppConfig::load_at(&dir.config());
        assert_eq!(config.hotkey, "Alt+Space");
        assert_eq!(config.ptt_mode, PttMode::Toggle);
        assert!(!config.sound_enabled);
        assert!((config.sound_volume - 0.4).abs() < 1e-6);
        assert!(config.onboarding_completed);

        assert!(!dir.0.join("ui-state.json").exists());
        assert!(dir.0.join("ui-state.json.migrated").exists());
    }

    #[test]
    fn second_run_is_a_no_op() {
        let dir = TempDir::new();
        bridge(&dir, r#"{"hotkey": "Alt+Space"}"#);
        migrate(&dir.db(), &dir.config()).unwrap();

        let report = migrate(&dir.db(), &dir.config()).unwrap();
        assert!(report.fts_ready, "schema work is still idempotent");
        assert!(!report.ui_state_applied);
        assert!(!report.ui_state_retired);
        assert_eq!(AppConfig::load_at(&dir.config()).hotkey, "Alt+Space");
    }

    #[test]
    fn malformed_bridge_is_kept_for_diagnosis() {
        let dir = TempDir::new();
        bridge(&dir, "{ not json");
        let report = migrate(&dir.db(), &dir.config()).unwrap();
        assert!(!report.ui_state_applied);
        assert!(dir.0.join("ui-state.json").exists());
    }

    #[test]
    fn blank_hotkey_and_out_of_range_volume_are_sanitised() {
        let dir = TempDir::new();
        bridge(&dir, r#"{"hotkey": "  ", "sound_volume": 4.5}"#);
        migrate(&dir.db(), &dir.config()).unwrap();
        let config = AppConfig::load_at(&dir.config());
        assert_eq!(config.hotkey, "CommandOrControl+Shift+F12");
        assert!((config.sound_volume - 1.0).abs() < 1e-6);
    }
}
