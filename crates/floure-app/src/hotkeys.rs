//! Global shortcut.
//!
//! Two backends, never both at once:
//!
//! - **evdev** (Linux, Wayland-first): reads `/dev/input/event*` directly, so it
//!   works under every compositor. Opt-in: needs the `input` group.
//! - **global-hotkey** (X11, Windows, macOS): a server-side key grab.
//!
//! Wayland without `input` access has no global grab at all. That is reported
//! as [`HotkeyStatus::NeedsInputGroup`] rather than papered over; `floure
//! --toggle` bound in the compositor stays the fallback.

use std::str::FromStr;
use std::sync::Arc;

use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use floure_core::config::{AppConfig, PttMode};
use floure_core::{Engine, EngineState};

#[cfg(target_os = "linux")]
mod evdev;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotkeyStatus {
    /// evdev listener on Linux; works under Wayland.
    RegisteredEvdev(String),
    /// global-hotkey: X11, Windows, macOS.
    RegisteredX11(String),
    /// Wayland and no read access to `/dev/input`.
    NeedsInputGroup,
    /// No display server and no input access: nothing can grab a key.
    Unavailable,
    Failed(String),
}

/// Which backend a platform gets. Kept pure so the decision is testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Evdev,
    Global,
    NeedsInputGroup,
    Unavailable,
}

/// Decide the backend from facts about the session.
///
/// evdev wins whenever it is usable: unlike an X11 grab it also sees keys
/// under Wayland, and it does not care which toolkit has focus.
pub fn decide(linux: bool, wayland: bool, x11: bool, input_access: bool) -> Backend {
    if !linux {
        return Backend::Global;
    }
    if input_access {
        Backend::Evdev
    } else if wayland {
        Backend::NeedsInputGroup
    } else if x11 {
        Backend::Global
    } else {
        Backend::Unavailable
    }
}

/// The exact command the UI tells the user to run.
pub const INPUT_GROUP_COMMAND: &str = "sudo usermod -aG input $USER";

pub fn start(engine: Arc<Engine>, config: &AppConfig) -> HotkeyStatus {
    #[cfg(target_os = "linux")]
    {
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let x11 = std::env::var_os("DISPLAY").is_some();
        match decide(true, wayland, x11, evdev::input_access()) {
            Backend::Evdev => return start_evdev(engine, config),
            Backend::NeedsInputGroup => return HotkeyStatus::NeedsInputGroup,
            Backend::Unavailable => return HotkeyStatus::Unavailable,
            Backend::Global => {}
        }
    }
    start_global(engine, config)
}

#[cfg(target_os = "linux")]
fn start_evdev(engine: Arc<Engine>, config: &AppConfig) -> HotkeyStatus {
    let binding = match parse_binding(&config.hotkey) {
        Ok(binding) => binding,
        Err(error) => return HotkeyStatus::Failed(error),
    };
    match evdev::start(engine, binding, config.ptt_mode) {
        Ok(label) => HotkeyStatus::RegisteredEvdev(label),
        Err(error) => HotkeyStatus::Failed(error),
    }
}

fn start_global(engine: Arc<Engine>, config: &AppConfig) -> HotkeyStatus {
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
                (HotKeyState::Pressed, PttMode::Toggle) => toggle(&engine),
                _ => {}
            }
        }
    });

    HotkeyStatus::RegisteredX11(label)
}

/// Tap-to-toggle: a press while listening stops, anything else starts.
fn toggle(engine: &Engine) {
    if matches!(
        engine.state(),
        EngineState::Listening | EngineState::Stopping
    ) {
        engine.stop();
    } else {
        let _ = engine.start();
    }
}

// ---------------------------------------------------------------------------
// Hotkey parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub super_key: bool,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl Modifiers {
    /// Canonical chord spelling for status text, e.g. `Ctrl+Shift+F12`.
    pub fn label(self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.super_key {
            parts.push("Super");
        }
        parts.join("+")
    }
}

/// A parsed chord: zero or more modifiers and exactly one key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub struct Binding {
    pub modifiers: Modifiers,
    /// Canonical key name: `F12`, `A`, `SPACE`, `PAGEUP`, …
    pub key: String,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
impl Binding {
    pub fn label(&self) -> String {
        let modifiers = self.modifiers.label();
        if modifiers.is_empty() {
            self.key.clone()
        } else {
            format!("{modifiers}+{}", self.key)
        }
    }
}

/// Parse the stored hotkey (`CommandOrControl+Shift+F12`) into modifiers and a
/// canonical key name. Unknown keys are an error, never a silent no-op.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_binding(hotkey: &str) -> Result<Binding, String> {
    let mut modifiers = Modifiers::default();
    let mut key = None;
    for part in hotkey.split('+') {
        let part = part.trim();
        if part.is_empty() {
            return Err(format!("empty part in hotkey {hotkey:?}"));
        }
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "commandorcontrol" | "cmdorctrl" | "commandorctrl" => {
                modifiers.ctrl = true
            }
            "shift" => modifiers.shift = true,
            "alt" | "option" | "opt" => modifiers.alt = true,
            "super" | "meta" | "cmd" | "command" | "win" | "windows" => modifiers.super_key = true,
            _ => {
                if key.is_some() {
                    return Err(format!("more than one key in hotkey {hotkey:?}"));
                }
                key = Some(canonical_key(part)?);
            }
        }
    }
    let key = key.ok_or_else(|| format!("no key in hotkey {hotkey:?}"))?;
    Ok(Binding { modifiers, key })
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn canonical_key(part: &str) -> Result<String, String> {
    // Tauri/KeyboardEvent spelling: `KeyQ`, `Digit1`.
    let stripped = part
        .strip_prefix("Key")
        .or_else(|| part.strip_prefix("Digit"))
        .unwrap_or(part);
    let upper = stripped.to_ascii_uppercase();
    let named = match upper.as_str() {
        "SPACE" | "SPACEBAR" => "SPACE",
        "ENTER" | "RETURN" => "ENTER",
        "ESC" | "ESCAPE" => "ESC",
        "TAB" => "TAB",
        "BACKSPACE" => "BACKSPACE",
        "DELETE" | "DEL" => "DELETE",
        "INSERT" | "INS" => "INSERT",
        "HOME" => "HOME",
        "END" => "END",
        "PAGEUP" | "PGUP" => "PAGEUP",
        "PAGEDOWN" | "PGDN" => "PAGEDOWN",
        "UP" | "ARROWUP" => "UP",
        "DOWN" | "ARROWDOWN" => "DOWN",
        "LEFT" | "ARROWLEFT" => "LEFT",
        "RIGHT" | "ARROWRIGHT" => "RIGHT",
        "PAUSE" => "PAUSE",
        "SCROLLLOCK" => "SCROLLLOCK",
        "PRINTSCREEN" | "PRTSC" => "PRINTSCREEN",
        "MENU" | "CONTEXTMENU" => "MENU",
        "MINUS" => "MINUS",
        "EQUAL" => "EQUAL",
        "COMMA" => "COMMA",
        "PERIOD" => "PERIOD",
        "SLASH" => "SLASH",
        "BACKSLASH" => "BACKSLASH",
        "SEMICOLON" => "SEMICOLON",
        "QUOTE" => "QUOTE",
        "BACKQUOTE" | "GRAVE" => "BACKQUOTE",
        "BRACKETLEFT" => "BRACKETLEFT",
        "BRACKETRIGHT" => "BRACKETRIGHT",
        _ => {
            let mut chars = upper.chars();
            let letter = chars.next().unwrap_or_default();
            let single = chars.next().is_none() && letter.is_ascii_alphanumeric();
            let is_fkey = upper
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
            if single {
                return Ok(upper);
            }
            if is_fkey {
                return Ok(upper);
            }
            return Err(format!("unsupported key {part:?}"));
        }
    };
    Ok(named.to_string())
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

    #[test]
    fn stored_hotkey_parses_into_modifiers_and_key() {
        let binding = parse_binding("CommandOrControl+Shift+F12").unwrap();
        assert_eq!(
            binding.modifiers,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            }
        );
        assert_eq!(binding.key, "F12");
        assert_eq!(binding.label(), "Ctrl+Shift+F12");

        let binding = parse_binding("CmdOrCtrl+alt+space").unwrap();
        assert!(binding.modifiers.ctrl && binding.modifiers.alt);
        assert_eq!(binding.key, "SPACE");

        let binding = parse_binding("Super+KeyQ").unwrap();
        assert!(binding.modifiers.super_key);
        assert_eq!(binding.key, "Q");

        let binding = parse_binding("Control+Digit1").unwrap();
        assert_eq!(binding.key, "1");
    }

    #[test]
    fn malformed_hotkeys_fail_instead_of_guessing() {
        assert!(parse_binding("CommandOrControl+Shift").is_err(), "no key");
        assert!(parse_binding("A+B").is_err(), "two keys");
        assert!(parse_binding("Ctrl+").is_err(), "empty part");
        assert!(parse_binding("Ctrl+NotAKey").is_err(), "unknown key");
    }

    #[test]
    fn backend_choice_matches_the_documented_fallbacks() {
        // Linux with input access always uses evdev, Wayland or not.
        assert_eq!(decide(true, true, true, true), Backend::Evdev);
        assert_eq!(decide(true, false, true, true), Backend::Evdev);
        // Wayland without input access has no global grab: tell the user why.
        assert_eq!(decide(true, true, false, false), Backend::NeedsInputGroup);
        // X11 without input access keeps global-hotkey.
        assert_eq!(decide(true, false, true, false), Backend::Global);
        // No display server and no input access: nothing can grab a key.
        assert_eq!(decide(true, false, false, false), Backend::Unavailable);
        // Other platforms keep global-hotkey.
        assert_eq!(decide(false, false, false, false), Backend::Global);
    }
}
