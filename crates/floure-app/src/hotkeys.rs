//! Global shortcut for X11/Windows/macOS.
//!
//! Wayland has no reliable global grab, so the loopback server and compositor
//! bindings are the documented trigger there. This module is never started
//! when `WAYLAND_DISPLAY` is set.

use std::str::FromStr;
use std::sync::Arc;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use floure_core::Engine;
use floure_core::config::{AppConfig, PttMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyStatus {
    Registered(String),
    Failed(String),
    /// Wayland (or another platform without a usable global grab).
    Unavailable,
}

pub fn start(engine: Arc<Engine>, config: &AppConfig) -> HotkeyStatus {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return HotkeyStatus::Unavailable;
    }
    let normalized = normalize_hotkey(&config.hotkey);
    let hotkey = match HotKey::from_str(&normalized) {
        Ok(hotkey) => hotkey,
        Err(error) => return HotkeyStatus::Failed(format!("{error}")),
    };
    let manager = match GlobalHotKeyManager::new() {
        Ok(manager) => manager,
        Err(error) => return HotkeyStatus::Failed(error.to_string()),
    };
    if let Err(error) = manager.register(hotkey) {
        return HotkeyStatus::Failed(error.to_string());
    }

    let mode = config.ptt_mode;
    let label = normalized.clone();
    std::thread::spawn(move || {
        // Keep the manager alive for the life of the process; dropping it
        // unregisters the shortcut.
        let _manager = manager;
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.recv() {
            if event.id() != hotkey.id() {
                continue;
            }
            match (event.state(), mode) {
                (HotKeyState::Pressed, PttMode::Hold) => {
                    let _ = engine.start();
                }
                (HotKeyState::Released, PttMode::Hold) => engine.stop(),
                (HotKeyState::Pressed, PttMode::Toggle) => {
                    if matches!(
                        engine.state(),
                        floure_core::EngineState::Listening | floure_core::EngineState::Stopping
                    ) {
                        engine.stop();
                    } else {
                        let _ = engine.start();
                    }
                }
                _ => {}
            }
        }
    });

    HotkeyStatus::Registered(label)
}

/// Tauri's cross-platform spelling → global-hotkey's parser syntax.
fn normalize_hotkey(hotkey: &str) -> String {
    hotkey
        .replace("CommandOrControl", "Ctrl")
        .replace("CmdOrCtrl", "Ctrl")
        .split('+')
        .map(|part| {
            let part = part.trim();
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => "Ctrl".to_string(),
                "shift" => "Shift".to_string(),
                "alt" | "option" => "Alt".to_string(),
                "super" | "meta" | "cmd" | "command" => "Super".to_string(),
                _ => part.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tauri_spelling_becomes_parser_spelling() {
        assert_eq!(
            normalize_hotkey("CommandOrControl+Shift+F12"),
            "Ctrl+Shift+F12"
        );
        assert_eq!(normalize_hotkey("CmdOrCtrl+alt+space"), "Ctrl+Alt+space");
        assert_eq!(normalize_hotkey("Control+super+KeyQ"), "Ctrl+Super+KeyQ");
    }
}
