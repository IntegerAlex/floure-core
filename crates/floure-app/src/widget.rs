//! Status-only dictation widget: a 240×56 layer-shell surface that shows what
//! the engine is doing.
//!
//! It is deliberately deaf and blind: `KeyboardInteractivity::None`, so a
//! compositor never hands it keyboard focus, and it is opened while an
//! utterance is in flight and closed again when the engine goes idle. The
//! target application keeps the caret.
//!
//! Layer-shell is wlroots/Hyprland/sway territory. Compositors without
//! `zwlr_layer_shell_v1` fail at `open_window`; that reason is recorded and the
//! in-app status line stays the indicator instead of crashing or silently
//! dropping the widget.

use std::sync::{Arc, Mutex};

use gpui::{
    App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowBounds, WindowHandle, WindowOptions, div, point, px, size,
};

use crate::app::RunState;
use crate::theme;

pub const WIDTH: f32 = 360.0;
pub const HEIGHT: f32 = 56.0;
/// Gap between the widget and the bottom screen edge, above a dock.
const BOTTOM_INSET: f32 = 48.0;

/// Where the widget can be drawn, and why it cannot when it cannot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Support {
    /// Wayland with `zwlr_layer_shell_v1` (verified by the first successful
    /// open; compositors that lack it land in `Unsupported`).
    LayerShell,
    /// X11: a plain borderless window placed bottom-centre, best effort.
    X11,
    Unsupported(String),
}

impl Support {
    pub fn detect() -> Self {
        #[cfg(target_os = "linux")]
        {
            if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                return Support::LayerShell;
            }
            if std::env::var_os("DISPLAY").is_some() {
                return Support::X11;
            }
        }
        Support::Unsupported("no display server".to_string())
    }

    pub fn label(&self) -> String {
        match self {
            Support::LayerShell => "Layer-shell overlay (Wayland)".to_string(),
            Support::X11 => "Floating window (X11, best effort)".to_string(),
            Support::Unsupported(reason) => format!("Unavailable: {reason}"),
        }
    }
}

/// Bottom-centre placement for the X11 fallback.
pub fn bottom_centre(visible: Bounds<gpui::Pixels>) -> (f32, f32) {
    let origin_x = f32::from(visible.origin.x);
    let origin_y = f32::from(visible.origin.y);
    let width = f32::from(visible.size.width);
    let height = f32::from(visible.size.height);
    (
        origin_x + (width - WIDTH) / 2.0,
        origin_y + height - HEIGHT - BOTTOM_INSET,
    )
}

#[derive(Debug, Clone)]
struct WidgetState {
    state: RunState,
    level: f32,
    /// Live preview of the sentence being spoken; empty otherwise.
    partial: String,
    /// Last BARS *smoothed* mic levels, oldest first. Peak-hold with a fast
    /// attack and slow decay is what makes the bars dance instead of
    /// flickering — raw 15 Hz samples read as random spikes.
    history: [f32; BARS],
    smoothed: f32,
}

const BARS: usize = 7;
/// Bar geometry: wide enough to read at a glance, matching the approved mock.
const BAR_W: f32 = 4.0;
const BAR_MAX: f32 = 24.0;

pub struct FloureWidget {
    state: Arc<Mutex<WidgetState>>,
}

impl FloureWidget {
    fn snapshot(&self) -> WidgetState {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }
}

impl Render for FloureWidget {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.snapshot();
        let recording = snapshot.state == RunState::Listening;
        let dot = if recording {
            theme::accent()
        } else {
            theme::text_muted()
        };
        // While listening the partial transcript IS the label; the static
        // state label returns for everything else.
        let label = if recording && !snapshot.partial.is_empty() {
            snapshot.partial.clone()
        } else {
            snapshot.state.label().to_string()
        };
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .h_full()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(theme::SPACE_3)
                    .px(theme::SPACE_4)
                    .rounded(theme::RADIUS_PANEL)
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::surface())
                    .overflow_hidden()
                    .child(div().size(px(8.0)).flex_shrink_0().rounded_full().bg(dot))
                    .child(waveform(&snapshot.history, recording))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(theme::TEXT_SM)
                            .text_color(theme::text())
                            .whitespace_nowrap()
                            .overflow_hidden()
                            .child(label),
                    ),
            )
    }
}

/// Bars ride the level history, newest on the right. Idle states settle to a
/// flat baseline so the widget never looks frozen.
fn waveform(history: &[f32; BARS], recording: bool) -> impl IntoElement {
    let color = if recording {
        theme::text()
    } else {
        theme::text_muted()
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(3.0))
        .h(px(BAR_MAX))
        .flex_shrink_0();
    for &value in history {
        let height = if recording {
            (value.clamp(0.0, 1.0) * BAR_MAX).max(3.0)
        } else {
            3.0
        };
        row = row.child(div().w(px(BAR_W)).h(px(height)).rounded(px(1.0)).bg(color));
    }
    row
}

/// Owns the widget window and its visibility.
pub struct WidgetHost {
    support: Support,
    window: Option<WindowHandle<FloureWidget>>,
    state: Arc<Mutex<WidgetState>>,
    /// Shown from the tray while the engine is idle. Cleared as soon as the
    /// widget hides again.
    manual: bool,
}

impl WidgetHost {
    pub fn new() -> Self {
        Self {
            support: Support::detect(),
            window: None,
            state: Arc::new(Mutex::new(WidgetState {
                state: RunState::Idle,
                level: 0.0,
                partial: String::new(),
                history: [0.0; BARS],
                smoothed: 0.0,
            })),
            manual: false,
        }
    }

    /// True unless layer-shell/X11 turned out to be unavailable. The first
    /// open is what proves it on Wayland.
    pub fn supported(&self) -> bool {
        !matches!(self.support, Support::Unsupported(_))
    }

    pub fn status(&self) -> String {
        self.support.label()
    }

    /// The tray's "Show widget": visible even while idle, showing the honest
    /// current state.
    pub fn show_manual(&mut self, state: RunState, level: f32, cx: &mut App) {
        self.manual = true;
        self.sync(state, level, String::new(), cx);
    }

    /// Mirror the engine state into the widget, opening and closing the window
    /// as the state dictates.
    pub fn sync(&mut self, state: RunState, level: f32, partial: String, cx: &mut App) {
        if let Ok(mut current) = self.state.lock() {
            // Peak-hold envelope: instant attack, ~0.5s decay. Raw samples
            // make the bars flicker; this is what "reacts to audio" means.
            let smoothed = if state == RunState::Listening {
                (level.max(current.smoothed * 0.85)).clamp(0.0, 1.0)
            } else {
                level.clamp(0.0, 1.0)
            };
            current.smoothed = smoothed;
            // Oldest drops off the left, the fresh level lands on the right.
            current.history.rotate_left(1);
            current.history[BARS - 1] = smoothed;
            current.level = level;
            current.state = state;
            current.partial = partial;
        }
        let wanted = state.shows_widget() || self.manual;
        if wanted {
            self.open(cx);
        } else {
            self.manual = false;
            self.close(cx);
        }
        if let Some(window) = self.window {
            let _ = window.update(cx, |_, _, cx| cx.notify());
        }
    }

    fn open(&mut self, cx: &mut App) {
        if self.window.is_some() || !self.supported() {
            return;
        }
        let options = self.options(cx);
        let state = self.state.clone();
        match cx.open_window(options, move |_, cx| {
            cx.new(|_| FloureWidget {
                state: state.clone(),
            })
        }) {
            Ok(window) => self.window = Some(window),
            Err(error) => {
                // Layer-shell missing, or the compositor refused the surface.
                // Record it once; never retry a known-bad backend.
                eprintln!("[widget] unavailable: {error}");
                self.support = Support::Unsupported(error.to_string());
                self.manual = false;
                self.window = None;
            }
        }
    }

    fn close(&mut self, cx: &mut App) {
        if let Some(window) = self.window.take() {
            let _ = window.update(cx, |_, window, _| window.remove_window());
        }
    }

    fn options(&self, cx: &mut App) -> WindowOptions {
        let bounds = match self.support {
            Support::X11 => {
                let visible = cx
                    .primary_display()
                    .map(|display| display.visible_bounds())
                    .unwrap_or_else(|| Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(1920.0), px(1080.0)),
                    });
                let (x, y) = bottom_centre(visible);
                Bounds {
                    origin: point(px(x), px(y)),
                    size: size(px(WIDTH), px(HEIGHT)),
                }
            }
            _ => Bounds {
                origin: point(px(0.0), px(0.0)),
                size: size(px(WIDTH), px(HEIGHT)),
            },
        };
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            // Never take focus from the app being dictated into.
            focus: false,
            kind: window_kind(&self.support),
            is_resizable: false,
            is_minimizable: false,
            window_decorations: Some(gpui::WindowDecorations::Client),
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            app_id: Some("floure-widget".to_string()),
            ..Default::default()
        }
    }
}

#[cfg(target_os = "linux")]
fn window_kind(support: &Support) -> gpui::WindowKind {
    use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
    match support {
        Support::LayerShell => gpui::WindowKind::LayerShell(LayerShellOptions {
            namespace: "floure-widget".to_string(),
            layer: Layer::Overlay,
            anchor: Anchor::BOTTOM,
            exclusive_zone: None,
            // The one non-negotiable: the surface can never receive keyboard
            // focus, so the caret stays where the text will land.
            keyboard_interactivity: KeyboardInteractivity::None,
            margin: Some((px(0.0), px(0.0), px(BOTTOM_INSET), px(0.0))),
            ..Default::default()
        }),
        _ => gpui::WindowKind::Normal,
    }
}

#[cfg(not(target_os = "linux"))]
fn window_kind(_support: &Support) -> gpui::WindowKind {
    gpui::WindowKind::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x11_fallback_sits_bottom_centre() {
        let (x, y) = bottom_centre(Bounds {
            origin: point(px(0.0), px(0.0)),
            size: size(px(1920.0), px(1080.0)),
        });
        assert_eq!(x, (1920.0 - WIDTH) / 2.0);
        assert_eq!(y, 1080.0 - HEIGHT - BOTTOM_INSET);

        // A second monitor to the right keeps the widget on that monitor.
        let (x, y) = bottom_centre(Bounds {
            origin: point(px(1920.0), px(0.0)),
            size: size(px(2560.0), px(1440.0)),
        });
        assert_eq!(x, 1920.0 + (2560.0 - WIDTH) / 2.0);
        assert_eq!(y, 1440.0 - HEIGHT - BOTTOM_INSET);
    }

    #[test]
    fn only_live_states_keep_the_widget_mapped() {
        assert!(RunState::Listening.shows_widget());
        assert!(RunState::Stopping.shows_widget());
        assert!(RunState::Transcribing.shows_widget());
        assert!(RunState::Rewriting.shows_widget());
        assert!(!RunState::Idle.shows_widget());
        assert!(!RunState::Stopped.shows_widget());
        assert!(!RunState::Warming.shows_widget());
    }

    #[test]
    fn unsupported_platforms_say_why() {
        let support = Support::Unsupported("no display server".to_string());
        assert!(support.label().contains("no display server"));
        let host = WidgetHost {
            support: support.clone(),
            window: None,
            state: Arc::new(Mutex::new(WidgetState {
                state: RunState::Idle,
                level: 0.0,
                partial: String::new(),
                history: [0.0; BARS],
                smoothed: 0.0,
            })),
            manual: false,
        };
        assert!(!host.supported());
    }
}
