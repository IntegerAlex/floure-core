//! Local, never-transmitted environment record for diagnosing
//! environment-dependent bugs.
//!
//! The evidence is blunt: ~17% of bug reports are non-reproducible and
//! environmental differences are 24% of classified root causes (MSR 2014);
//! configuration errors are 27% of customer cases and 31% of high-severity
//! ones (SOSP 2011). Almost every audio bug found in this codebase was one — a
//! 48 kHz mic, an I32 device, a stale device index. None of them are
//! diagnosable from "it doesn't work".
//!
//! So this records what the app actually ran against: OS and version, display
//! server, audio server, the negotiated mic rate/channels/format, and the
//! selected engines. It is written to the data directory and **never sent
//! anywhere**. Sharing is a deliberate user action ("Copy diagnostics"), so
//! nothing leaves the machine on its own. Setting `DO_NOT_TRACK` disables even
//! the local write.

use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

/// The negotiated audio format, recorded when a capture stream starts.
///
/// This is the single most valuable field in the whole record: the 48 kHz
/// flush bug, the I32 refusal, and the sample-rate assumptions were all only
/// visible from here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEnv {
    pub sample_rate: u32,
    pub channels: u16,
    pub format: String,
}

static AUDIO_ENV: OnceLock<Mutex<Option<AudioEnv>>> = OnceLock::new();

fn audio_env() -> &'static Mutex<Option<AudioEnv>> {
    AUDIO_ENV.get_or_init(|| Mutex::new(None))
}

/// Remember the format a capture stream negotiated. Called on every start, so
/// the record always reflects the most recent device.
pub fn record_audio(env: AudioEnv) {
    if let Ok(mut slot) = audio_env().lock() {
        *slot = Some(env);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostics {
    pub app_version: String,
    pub os: String,
    pub os_version: String,
    pub arch: String,
    /// "wayland" / "x11" / "native" — decides whether a global hotkey can work
    /// at all, which is the difference between the hotkey path and the loopback.
    pub display_server: String,
    /// "pipewire" / "pulseaudio" / "alsa" / "unknown".
    pub audio_server: String,
    pub asr_profile: String,
    pub llm_provider: String,
    pub language: String,
    pub selected_mic_id: Option<String>,
    pub mic_sample_rate: Option<u32>,
    pub mic_channels: Option<u16>,
    pub mic_format: Option<String>,
    pub updated_unix: u64,
}

/// `DO_NOT_TRACK=1` means do not collect, not merely do not upload. Nothing
/// here is uploaded regardless, but the flag is honoured as a full opt-out.
fn opted_out() -> bool {
    std::env::var("DO_NOT_TRACK")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Human-readable OS name/version, best effort and without new dependencies.
///
/// Linux reads `PRETTY_NAME` from `/etc/os-release` (e.g. "Ubuntu 24.04.1
/// LTS"); elsewhere the compile-time target is all we can state honestly.
fn os_version() -> String {
    if cfg!(target_os = "linux") {
        if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
            for line in text.lines() {
                if let Some(v) = line.strip_prefix("PRETTY_NAME=") {
                    return v.trim().trim_matches('"').to_string();
                }
            }
        }
    }
    String::new()
}

/// Which sound server the app is actually talking to.
///
/// Checked by socket presence rather than by running `pactl`/`pw-cli`: those
/// may not be installed, and a diagnostic that fails on a minimal system is
/// the opposite of useful.
fn audio_server() -> String {
    if !cfg!(target_os = "linux") {
        return "native".to_string();
    }
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if runtime.is_empty() {
        return "unknown".to_string();
    }
    let dir = std::path::Path::new(&runtime);
    if dir.join("pipewire-0").exists() {
        "pipewire".to_string()
    } else if dir.join("pulse").join("native").exists() {
        "pulseaudio".to_string()
    } else {
        "alsa".to_string()
    }
}

/// Build the current record. Pure with respect to the filesystem except for
/// reading `/etc/os-release` and probing sockets.
pub fn snapshot() -> Diagnostics {
    let config = crate::config::AppConfig::load();
    let (_, display_server) = crate::output::detect_platform();
    let audio = audio_env().lock().ok().and_then(|s| s.clone());

    Diagnostics {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        os_version: os_version(),
        arch: std::env::consts::ARCH.to_string(),
        display_server: display_server.to_string(),
        audio_server: audio_server(),
        asr_profile: format!("{:?}", config.asr_profile),
        llm_provider: format!("{:?}", config.llm_provider),
        language: config.language.clone(),
        selected_mic_id: config.selected_mic_id.clone(),
        mic_sample_rate: audio.as_ref().map(|a| a.sample_rate),
        mic_channels: audio.as_ref().map(|a| a.channels),
        mic_format: audio.as_ref().map(|a| a.format.clone()),
        updated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    }
}

fn path() -> std::path::PathBuf {
    crate::config::data_dir().join("diagnostics.json")
}

/// Write the record to the data directory. Best-effort: a failure to write
/// must never affect the app, so the result is only logged.
pub fn write() {
    if opted_out() {
        return;
    }
    let path = path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(&snapshot()) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("[diagnostics] write {} failed: {e}", path.display());
            }
        }
        Err(e) => eprintln!("[diagnostics] serialize failed: {e}"),
    }
}

/// The record as pretty JSON, for the UI to display and copy.
///
/// Returned as a string rather than a struct so the frontend never has to be
/// rebuilt when a field is added.
#[tauri::command]
pub fn get_diagnostics() -> Result<String, String> {
    serde_json::to_string_pretty(&snapshot()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_always_carries_the_fields_needed_to_triage() {
        let d = snapshot();
        // Without these, an environment bug report is unactionable.
        assert!(!d.app_version.is_empty());
        assert!(!d.os.is_empty());
        assert!(!d.arch.is_empty());
        assert!(!d.display_server.is_empty());
        assert!(!d.audio_server.is_empty());
        assert!(d.updated_unix > 0);
    }

    #[test]
    fn recording_audio_env_shows_up_in_the_snapshot() {
        record_audio(AudioEnv {
            sample_rate: 48_000,
            channels: 2,
            format: "I32".to_string(),
        });
        let d = snapshot();
        // This is exactly the case that used to be undiagnosable: a 48 kHz
        // stereo I32 device.
        assert_eq!(d.mic_sample_rate, Some(48_000));
        assert_eq!(d.mic_channels, Some(2));
        assert_eq!(d.mic_format.as_deref(), Some("I32"));
    }
}
