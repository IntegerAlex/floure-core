// Tray icon state (research §7 rec 1, §2 rec 4): the icon itself shows idle vs.
// recording so the recording indicator doesn't rely solely on the animated
// pill. The default window icon gets a bottom-right badge — snow dot at idle,
// coral dot while recording — plus a tooltip and menu that agree.

use tauri::image::Image;
use tauri::menu::MenuItem;
use tauri::tray::TrayIcon;
use tauri::{AppHandle, Manager, Wry};

const CORAL: [u8; 4] = [0xFF, 0x3B, 0x56, 0xFF];
const SNOW: [u8; 4] = [0xFA, 0xF8, 0xF5, 0xFF];
const RING: [u8; 4] = [0x2C, 0x25, 0x20, 0xFF];

pub struct TrayState {
    tray: TrayIcon<Wry>,
    start: MenuItem<Wry>,
    stop: MenuItem<Wry>,
    status: MenuItem<Wry>,
    icon_idle: Image<'static>,
    icon_rec: Image<'static>,
}

/// Paint a badge in the bottom-right corner of the icon: dark backing disc
/// (reads on any icon, usually on transparent corner), then the state dot.
fn paint_badge(rgba: &mut [u8], w: u32, h: u32, dot: [u8; 4]) {
    let short = w.min(h) as i32;
    let r = short / 4;
    let margin = short / 16;
    let cx = w as i32 - margin - r;
    let cy = h as i32 - margin - r;
    let inner = r - (short / 48).max(1);
    for y in (cy - r).max(0)..(cy + r + 1).min(h as i32) {
        for x in (cx - r).max(0)..(cx + r + 1).min(w as i32) {
            let dx = x - cx;
            let dy = y - cy;
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                continue;
            }
            let i = ((y as u32 * w + x as u32) * 4) as usize;
            let px = &mut rgba[i..i + 4];
            let c = if d2 >= inner * inner { RING } else { dot };
            px[..4].copy_from_slice(&c);
        }
    }
}

/// Input can borrow any lifetime — pixels are copied into an owned 'static image.
fn badge_icon(base: &Image<'_>, dot: [u8; 4]) -> Image<'static> {
    let (w, h) = (base.width(), base.height());
    let mut rgba = base.rgba().to_vec();
    paint_badge(&mut rgba, w, h, dot);
    Image::new_owned(rgba, w, h)
}

impl TrayState {
    pub fn build(
        tray: TrayIcon<Wry>,
        start: MenuItem<Wry>,
        stop: MenuItem<Wry>,
        status: MenuItem<Wry>,
        base: &Image<'_>,
    ) -> Self {
        let icon_idle = badge_icon(base, SNOW);
        let icon_rec = badge_icon(base, CORAL);
        Self {
            tray,
            start,
            stop,
            status,
            icon_idle,
            icon_rec,
        }
    }
}

/// Sync tray icon/tooltip/menu with the engine's recording state.
#[tauri::command]
pub fn set_tray_state(app: AppHandle, recording: bool) -> Result<(), String> {
    let s = app.state::<TrayState>();
    let set = |r: tauri::Result<()>| r.map_err(|e| e.to_string());

    let (icon, tip) = if recording {
        (&s.icon_rec, "STT — Recording")
    } else {
        (&s.icon_idle, "STT — Speech to Text")
    };
    set(s.tray.set_icon(Some(icon.clone())))?;
    set(s.tray.set_tooltip(Some(tip)))?;
    set(s.start.set_enabled(!recording))?;
    set(s.stop.set_enabled(recording))?;
    set(s
        .status
        .set_text(if recording { "Recording…" } else { "Idle" }))?;
    Ok(())
}
