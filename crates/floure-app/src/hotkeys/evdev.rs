//! evdev global shortcut (Linux).
//!
//! Reads `/dev/input/event*` directly, so it works under Wayland, where no
//! client can grab a global key. Requires the `input` group.
//!
//! The device is **not** grabbed: `EVIOCGRAB` would swallow every keystroke
//! typed at the machine. The chord therefore still reaches the focused app,
//! exactly like an X11 global grab.
//!
//! Devices are re-enumerated every couple of seconds, which is the hotplug and
//! reconnect story: no inotify, no udev monitor. Device nodes that disappear,
//! error, or hang up are dropped from the set instead of being polled forever.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use evdev::{AttributeSetRef, Device, EventType, KeyCode};

use floure_core::Engine;
use floure_core::config::PttMode;

use super::{Binding, Modifiers, toggle};

/// How often `/dev/input` is re-read, i.e. how quickly a re-plugged keyboard
/// starts working again.
const RESCAN: Duration = Duration::from_secs(2);
/// Poll cadence while devices are open. Non-blocking reads, so this is the
/// latency floor and the idle CPU ceiling.
const POLL: Duration = Duration::from_millis(5);
/// Poll cadence with no devices: do not spin while the keyboard is away.
const IDLE_POLL: Duration = Duration::from_millis(500);

/// Can this process read input devices at all? True when any `/dev/input`
/// event node opens; the permission is what matters, not the device type.
pub fn input_access() -> bool {
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return false;
    };
    entries
        .flatten()
        .filter(|entry| is_event_node(&entry.path()))
        .any(|entry| Device::open(entry.path()).is_ok())
}

fn is_event_node(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("event"))
}

/// Keyboards injected by typing tools must be ignored: Floure types its own
/// transcript through one of these, and listening to it would be a feedback
/// loop.
pub(super) fn is_injection_device(name: Option<&str>) -> bool {
    let Some(name) = name else {
        return false;
    };
    let name = name.to_ascii_lowercase();
    ["ydotool", "wtype", "dotool", "xdotool"]
        .iter()
        .any(|tool| name.contains(tool))
}

/// A keyboard carries the full letter row. A mouse with a numeric keypad is
/// not a keyboard and must not be opened for every chord.
fn has_letter_keys(keys: &AttributeSetRef<KeyCode>) -> bool {
    (KeyCode::KEY_A.0..=KeyCode::KEY_Z.0).all(|code| keys.contains(KeyCode(code)))
}

fn is_keyboard(device: &Device) -> bool {
    device.supported_keys().is_some_and(has_letter_keys)
}

/// Open the chord's key. Unknown names are reported, not ignored.
pub(super) fn keycode(name: &str) -> Option<KeyCode> {
    const FUNCTION_KEYS: [KeyCode; 24] = [
        KeyCode::KEY_F1,
        KeyCode::KEY_F2,
        KeyCode::KEY_F3,
        KeyCode::KEY_F4,
        KeyCode::KEY_F5,
        KeyCode::KEY_F6,
        KeyCode::KEY_F7,
        KeyCode::KEY_F8,
        KeyCode::KEY_F9,
        KeyCode::KEY_F10,
        KeyCode::KEY_F11,
        KeyCode::KEY_F12,
        KeyCode::KEY_F13,
        KeyCode::KEY_F14,
        KeyCode::KEY_F15,
        KeyCode::KEY_F16,
        KeyCode::KEY_F17,
        KeyCode::KEY_F18,
        KeyCode::KEY_F19,
        KeyCode::KEY_F20,
        KeyCode::KEY_F21,
        KeyCode::KEY_F22,
        KeyCode::KEY_F23,
        KeyCode::KEY_F24,
    ];

    if let Some(index) = name.strip_prefix('F').and_then(|n| n.parse::<usize>().ok())
        && let Some(key) = index.checked_sub(1).and_then(|i| FUNCTION_KEYS.get(i))
    {
        return Some(*key);
    }

    let named = match name {
        "SPACE" => KeyCode::KEY_SPACE,
        "ENTER" => KeyCode::KEY_ENTER,
        "ESC" => KeyCode::KEY_ESC,
        "TAB" => KeyCode::KEY_TAB,
        "BACKSPACE" => KeyCode::KEY_BACKSPACE,
        "DELETE" => KeyCode::KEY_DELETE,
        "INSERT" => KeyCode::KEY_INSERT,
        "HOME" => KeyCode::KEY_HOME,
        "END" => KeyCode::KEY_END,
        "PAGEUP" => KeyCode::KEY_PAGEUP,
        "PAGEDOWN" => KeyCode::KEY_PAGEDOWN,
        "UP" => KeyCode::KEY_UP,
        "DOWN" => KeyCode::KEY_DOWN,
        "LEFT" => KeyCode::KEY_LEFT,
        "RIGHT" => KeyCode::KEY_RIGHT,
        "PAUSE" => KeyCode::KEY_PAUSE,
        "SCROLLLOCK" => KeyCode::KEY_SCROLLLOCK,
        "PRINTSCREEN" => KeyCode::KEY_SYSRQ,
        "MENU" => KeyCode::KEY_MENU,
        "MINUS" => KeyCode::KEY_MINUS,
        "EQUAL" => KeyCode::KEY_EQUAL,
        "COMMA" => KeyCode::KEY_COMMA,
        "PERIOD" => KeyCode::KEY_DOT,
        "SLASH" => KeyCode::KEY_SLASH,
        "BACKSLASH" => KeyCode::KEY_BACKSLASH,
        "SEMICOLON" => KeyCode::KEY_SEMICOLON,
        "QUOTE" => KeyCode::KEY_APOSTROPHE,
        "BACKQUOTE" => KeyCode::KEY_GRAVE,
        "BRACKETLEFT" => KeyCode::KEY_LEFTBRACE,
        "BRACKETRIGHT" => KeyCode::KEY_RIGHTBRACE,
        _ => {
            let mut chars = name.chars();
            let character = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            match character {
                'A'..='Z' => KeyCode(KeyCode::KEY_A.0 + (character as u16 - 'A' as u16)),
                '1'..='9' => KeyCode(KeyCode::KEY_1.0 + (character as u16 - '1' as u16)),
                '0' => KeyCode::KEY_0,
                _ => return None,
            }
        }
    };
    Some(named)
}

/// One required modifier may be either side of the keyboard.
fn modifier_groups(modifiers: Modifiers) -> Vec<Vec<KeyCode>> {
    let mut groups = Vec::new();
    if modifiers.ctrl {
        groups.push(vec![KeyCode::KEY_LEFTCTRL, KeyCode::KEY_RIGHTCTRL]);
    }
    if modifiers.shift {
        groups.push(vec![KeyCode::KEY_LEFTSHIFT, KeyCode::KEY_RIGHTSHIFT]);
    }
    if modifiers.alt {
        groups.push(vec![KeyCode::KEY_LEFTALT, KeyCode::KEY_RIGHTALT]);
    }
    if modifiers.super_key {
        groups.push(vec![KeyCode::KEY_LEFTMETA, KeyCode::KEY_RIGHTMETA]);
    }
    groups
}

/// Start listening. The initial device scan happens here so a machine with no
/// readable keyboard reports a failure instead of pretending to be ready.
pub fn start(engine: Arc<Engine>, binding: Binding, mode: PttMode) -> Result<String, String> {
    let target =
        keycode(&binding.key).ok_or_else(|| format!("key {} has no evdev code", binding.key))?;
    let mut devices = HashMap::new();
    scan(&mut devices);
    if devices.is_empty() {
        return Err("no keyboard devices under /dev/input".to_string());
    }
    let label = binding.label();
    let groups = modifier_groups(binding.modifiers);
    std::thread::spawn(move || run(engine, devices, target, groups, mode));
    Ok(label)
}

/// Add readable keyboard devices, drop nodes that no longer exist.
fn scan(devices: &mut HashMap<PathBuf, Device>) {
    devices.retain(|path, _| path.exists());
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_event_node(&path) || devices.contains_key(&path) {
            continue;
        }
        let Ok(device) = Device::open(&path) else {
            continue;
        };
        if is_injection_device(device.name()) || !is_keyboard(&device) {
            continue;
        }
        // Non-blocking is what keeps a lost device from parking the thread in
        // read(2) forever.
        if device.set_nonblocking(true).is_err() {
            continue;
        }
        devices.insert(path, device);
    }
}

/// What a key event means for dictation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
    Press,
    Release,
    Ignore,
}

/// Modifiers gate the press only. The release must stop the recording even if
/// a modifier was let go first, otherwise a hold-mode recording never ends.
fn action(value: i32, pressed: bool, modifiers_ok: bool) -> Action {
    match value {
        1 if !pressed && modifiers_ok => Action::Press,
        0 if pressed => Action::Release,
        _ => Action::Ignore,
    }
}

fn run(
    engine: Arc<Engine>,
    mut devices: HashMap<PathBuf, Device>,
    target: KeyCode,
    groups: Vec<Vec<KeyCode>>,
    mode: PttMode,
) {
    let mut held: HashSet<KeyCode> = HashSet::new();
    let mut pressed = false;
    let mut last_scan = Instant::now();

    loop {
        if last_scan.elapsed() >= RESCAN {
            let before = devices.len();
            scan(&mut devices);
            if devices.len() != before && pressed {
                // The keyboard we were listening to went away mid-hold. Emit
                // the release ourselves; no key-up is coming.
                pressed = false;
                engine.stop();
            }
            last_scan = Instant::now();
        }

        let mut events = Vec::new();
        let mut dead = Vec::new();
        for (path, device) in devices.iter_mut() {
            match device.fetch_events() {
                Ok(fetched) => events.extend(
                    fetched
                        .filter(|event| event.event_type() == EventType::KEY)
                        .map(|event| (event.code(), event.value())),
                ),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(_) => dead.push(path.clone()),
            }
        }
        for path in dead {
            devices.remove(&path);
        }

        let empty = devices.is_empty();
        for (code, value) in events {
            let code = KeyCode(code);
            if groups.iter().any(|group| group.contains(&code)) {
                match value {
                    1 => {
                        held.insert(code);
                    }
                    0 => {
                        held.remove(&code);
                    }
                    _ => {}
                }
                continue;
            }
            if code != target {
                continue;
            }
            let modifiers_ok = groups
                .iter()
                .all(|group| group.iter().any(|key| held.contains(key)));
            match action(value, pressed, modifiers_ok) {
                Action::Press => {
                    pressed = true;
                    match mode {
                        PttMode::Hold => {
                            let _ = engine.start();
                        }
                        PttMode::Toggle => toggle(&engine),
                    }
                }
                Action::Release => {
                    pressed = false;
                    if mode == PttMode::Hold {
                        engine.stop();
                    }
                }
                Action::Ignore => {}
            }
        }

        std::thread::sleep(if empty { IDLE_POLL } else { POLL });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use evdev::AttributeSet;

    #[test]
    fn injection_keyboards_are_ignored() {
        for name in [
            "ydotool",
            "wtype",
            "dotool keyboard",
            "xdotool fake",
            "Ydotool Virtual Keyboard",
        ] {
            assert!(is_injection_device(Some(name)), "{name}");
        }
        for name in ["AT Translated Set 2 keyboard", "Logitech K380", ""] {
            assert!(!is_injection_device(Some(name)), "{name}");
        }
        assert!(!is_injection_device(None));
    }

    #[test]
    fn only_full_letter_rows_count_as_keyboards() {
        let mut letters: AttributeSet<KeyCode> = AttributeSet::new();
        assert!(!has_letter_keys(&letters), "empty set is not a keyboard");
        letters.insert(KeyCode::KEY_A);
        letters.insert(KeyCode::KEY_Z);
        assert!(
            !has_letter_keys(&letters),
            "a keypad with A and Z is not a keyboard"
        );
        for code in KeyCode::KEY_A.0..=KeyCode::KEY_Z.0 {
            letters.insert(KeyCode(code));
        }
        assert!(has_letter_keys(&letters));
    }

    #[test]
    fn chord_keys_map_to_evdev_codes() {
        assert_eq!(keycode("F12"), Some(KeyCode::KEY_F12));
        assert_eq!(keycode("F10"), Some(KeyCode::KEY_F10));
        assert_eq!(keycode("A"), Some(KeyCode::KEY_A));
        assert_eq!(keycode("1"), Some(KeyCode::KEY_1));
        assert_eq!(keycode("0"), Some(KeyCode::KEY_0));
        assert_eq!(keycode("SPACE"), Some(KeyCode::KEY_SPACE));
        assert_eq!(keycode("PAGEUP"), Some(KeyCode::KEY_PAGEUP));
        assert_eq!(keycode("F25"), None);
        assert_eq!(keycode("NOTAKEY"), None);
    }

    #[test]
    fn modifiers_gate_press_but_never_release() {
        assert_eq!(action(1, false, true), Action::Press);
        assert_eq!(action(1, false, false), Action::Ignore, "chord not held");
        assert_eq!(action(1, true, true), Action::Ignore, "autorepeat");
        assert_eq!(action(2, false, true), Action::Ignore, "kinetic repeat");
        assert_eq!(action(0, true, false), Action::Release, "stop regardless");
        assert_eq!(action(0, false, false), Action::Ignore);
    }
}
