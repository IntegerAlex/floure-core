//! Linux StatusNotifierItem tray. Absence of an SNI host is not fatal: the
//! loopback server and CLI remain the documented trigger.

use std::sync::Arc;

use ksni::blocking::TrayMethods;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, Tray};

use floure_core::Engine;
use floure_core::EngineState;

struct FloureTray {
    engine: Arc<Engine>,
}

impl Tray for FloureTray {
    fn id(&self) -> String {
        "floure".into()
    }

    fn title(&self) -> String {
        "Floure".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![Icon {
            width: 16,
            height: 16,
            data: dot_icon(),
        }]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        crate::control_server::request_show();
        crate::control_server::request_dictate();
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let recording = matches!(
            self.engine.state(),
            EngineState::Listening | EngineState::Stopping
        );
        let toggle_label = if recording {
            "Stop recording"
        } else {
            "Start recording"
        }
        .to_string();
        vec![
            StandardItem {
                label: "Open Floure".into(),
                activate: Box::new(|_: &mut FloureTray| crate::control_server::request_show()),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: toggle_label,
                activate: Box::new(|tray: &mut FloureTray| {
                    if matches!(
                        tray.engine.state(),
                        EngineState::Listening | EngineState::Stopping
                    ) {
                        tray.engine.stop();
                    } else {
                        let _ = tray.engine.start();
                    }
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Configuration".into(),
                activate: Box::new(|_: &mut FloureTray| {
                    crate::control_server::request_show();
                    crate::control_server::request_config();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show widget".into(),
                activate: Box::new(|_: &mut FloureTray| crate::control_server::request_widget()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit Floure".into(),
                activate: Box::new(|tray: &mut FloureTray| {
                    // Flush the in-flight transcript before exiting.
                    tray.engine.stop_blocking();
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// A 16 px white dot, generated rather than shipped as a PNG so the tray works
/// before the packaging asset pass.
fn dot_icon() -> Vec<u8> {
    let mut data = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as f32 - 7.5;
            let dy = y as f32 - 7.5;
            let inside = dx * dx + dy * dy <= 42.0;
            if inside {
                data.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
            } else {
                data.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    data
}

/// Returns true when the tray was registered.
pub fn spawn(engine: Arc<Engine>) -> bool {
    let tray = FloureTray { engine };
    match tray.spawn() {
        Ok(handle) => {
            // The handle is the service; keep it alive for the process
            // lifetime. There is nothing to update from outside.
            std::mem::forget(handle);
            true
        }
        Err(error) => {
            eprintln!("[tray] unavailable: {error}");
            false
        }
    }
}
