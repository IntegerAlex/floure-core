use anyhow::Result;
use std::process::Command;

pub fn detect_platform() -> (&'static str, &'static str) {
    let platform = std::env::consts::OS;
    let display_server = if platform == "linux" {
        if std::env::var("WAYLAND_DISPLAY").is_ok()
            || std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
        {
            "wayland"
        } else if std::env::var("DISPLAY").is_ok() {
            "x11"
        } else {
            "unknown"
        }
    } else {
        "native"
    };
    (platform, display_server)
}

pub fn type_text(text: &str) -> Result<bool> {
    if text.trim().is_empty() {
        return Ok(false);
    }

    let (platform, display_server) = detect_platform();

    match (platform, display_server) {
        ("windows", _) => type_windows_paste(text),
        ("linux", "wayland") => type_wayland(text),
        ("linux", "x11") | ("linux", "unknown") => {
            run_piped_command(text, "xdotool", &["type", "--clearmodifiers"])
        }
        ("macos", _) => {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!("tell application \"System Events\" to keystroke \"{escaped}\"");
            let output = Command::new("osascript").args(["-e", &script]).output()?;
            Ok(output.status.success())
        }
        _ => Err(anyhow::anyhow!("No typing backend available")),
    }
}

/// Wayland typing backends, tried in order.
///
/// `wtype` needs no daemon and handles Unicode best. `dotool` and `ydotool`
/// inject through uinput, so they still work when the compositor's virtual
/// keyboard protocol is missing — but both need a daemon or the `input` group.
const WAYLAND_TYPERS: [&str; 3] = ["wtype", "dotool", "ydotool"];

/// Type on Wayland, falling through the backend list.
///
/// A tool that is not installed is skipped; a tool that runs and fails is also
/// skipped, and only when every backend has failed is the failure reported.
fn type_wayland(text: &str) -> Result<bool> {
    let mut last_error = None;
    for tool in WAYLAND_TYPERS {
        match run_typer(text, tool) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(error) => last_error = Some(error),
        }
    }
    match last_error {
        Some(error) => Err(error),
        None => Ok(false),
    }
}

fn run_typer(text: &str, tool: &str) -> Result<bool> {
    match tool {
        "wtype" => run_piped_command(text, "wtype", &["-"]),
        // dotool is line-protocol on stdin: one `type` per line, explicit
        // Enter between them because a newline ends the command.
        "dotool" => run_piped_command(&dotool_stream(text), "dotool", &[]),
        // ydotool takes the text as an argument. One millisecond between keys
        // keeps KWin from dropping a modifier key-up.
        _ => {
            let output = Command::new("ydotool")
                .args(["type", "--key-delay", "1", "--", text])
                .output()?;
            Ok(output.status.success())
        }
    }
}

fn dotool_stream(text: &str) -> String {
    let mut stream = String::new();
    for (index, line) in text.split('\n').enumerate() {
        if index > 0 {
            stream.push_str("key enter\n");
        }
        if !line.is_empty() {
            stream.push_str("type ");
            stream.push_str(line);
            stream.push('\n');
        }
    }
    stream
}

pub fn copy_to_clipboard(text: &str) -> Result<bool> {
    if text.is_empty() {
        return Ok(false);
    }

    let (platform, display_server) = detect_platform();

    match (platform, display_server) {
        ("windows", _) => copy_to_windows_clipboard(text),
        ("linux", "wayland") => run_piped_command(text, "wl-copy", &[]),
        ("linux", "x11") | ("linux", "unknown") => {
            run_piped_command(text, "xclip", &["-selection", "clipboard"])
        }
        ("macos", _) => {
            let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
            let script = format!("tell application \"System Events\" to keystroke \"{escaped}\"");
            let output = Command::new("osascript").args(["-e", &script]).output()?;
            Ok(output.status.success())
        }
        _ => Err(anyhow::anyhow!("No clipboard backend available")),
    }
}

pub fn save_to_history(
    text: &str,
    raw_text: &str,
    mode: &str,
    model: &str,
    language: &str,
    duration_sec: f64,
    db_path: &std::path::Path,
) -> Result<i64> {
    use rusqlite::Connection;
    let conn = Connection::open(db_path)?;
    crate::history::ensure_schema(&conn)?;
    conn.execute(
        "INSERT INTO transcripts (raw_text, processed_text, language, mode, model, duration_sec)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![raw_text, text, language, mode, model, duration_sec],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn type_windows_paste(text: &str) -> Result<bool> {
    // Clipboard first (proven reliable via clip.exe), then Ctrl+V injected
    // with SendInput. Powershell + SendKeys is out: Send throws without a
    // message pump, SendWait blocks forever on busy windows.
    if !copy_to_windows_clipboard(text)? {
        return Ok(false);
    }
    // Brief beat so the clipboard settles before the keystroke lands.
    // ponytail: fixed 150ms; no paste-complete signal exists to wait on instead.
    std::thread::sleep(std::time::Duration::from_millis(150));
    send_ctrl_v()
}

/// Windows clipboard write, Unicode-safe.
///
/// `clip.exe` decodes stdin with the console/OEM code page unless the bytes
/// are UTF-16 with a BOM, so the raw UTF-8 the Unix arms write pastes "café"
/// as "cafÃ©" — and the app ships Whisper profiles for exactly those
/// languages. `CREATE_NO_WINDOW` stops the console window from flashing and
/// taking focus away from the paste target.
#[cfg(windows)]
fn copy_to_windows_clipboard(text: &str) -> Result<bool> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let mut wide = vec![0xFEFFu16]; // BOM: marks the payload as UTF-16LE
    wide.extend(text.encode_utf16());
    let mut bytes = Vec::with_capacity(wide.len() * 2);
    for unit in wide {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }

    let mut child = Command::new("clip.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(&bytes)?;
    }
    Ok(child.wait()?.success())
}

/// Unreachable in practice: the Windows arms run only when
/// `std::env::consts::OS == "windows"`. Present so they resolve on every
/// target, exactly like the `send_ctrl_v` stub.
#[cfg(not(windows))]
fn copy_to_windows_clipboard(text: &str) -> Result<bool> {
    run_piped_command(text, "clip.exe", &[])
}

#[cfg(windows)]
fn send_ctrl_v() -> Result<bool> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }
    let inputs = [
        key(VK_CONTROL, false),
        key(VK_V, false),
        key(VK_V, true),
        key(VK_CONTROL, true),
    ];
    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    Ok(sent == inputs.len() as u32)
}

/// Non-Windows stub: `type_windows_paste` is only reachable through the
/// `("windows", _)` match arm, but arms compile on every platform, so the
/// symbol must exist everywhere to keep Linux/macOS builds green.
#[cfg(not(windows))]
fn send_ctrl_v() -> Result<bool> {
    Err(anyhow::anyhow!("No typing backend available"))
}

pub fn run_piped_command(text: &str, tool: &str, prefix_args: &[&str]) -> Result<bool> {
    let mut child = Command::new(tool)
        .args(prefix_args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        use std::io::Write;
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wayland_typers_keep_the_documented_order() {
        assert_eq!(WAYLAND_TYPERS, ["wtype", "dotool", "ydotool"]);
    }

    #[test]
    fn dotool_gets_a_line_protocol_stream() {
        assert_eq!(dotool_stream("hello world"), "type hello world\n");
        assert_eq!(dotool_stream("one\ntwo"), "type one\nkey enter\ntype two\n");
        // A blank line still needs its Enter; `type ` with nothing after it is
        // not a command dotool understands.
        assert_eq!(
            dotool_stream("one\n\ntwo"),
            "type one\nkey enter\nkey enter\ntype two\n"
        );
    }
}
