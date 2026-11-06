//! Application state and the window shell: title bar, rail, destinations.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, KeyBinding, MouseButton, ParentElement, Render,
    SharedString, Styled, Window, actions, div, prelude::*, px,
};

use floure_core::config::AppConfig;
use floure_core::output::copy_to_clipboard;
use floure_core::{
    Engine, EngineEvent, EngineState, History, Insights, InsightsRepo, TranscriptRow,
};

use crate::hotkeys::HotkeyStatus;
use crate::theme;
use crate::views;

actions!(floure, [GoDictate, GoHistory, GoStats, GoConfig, Quit]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Dictate,
    History,
    Stats,
    Configuration,
}

impl Destination {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dictate => "Dictate",
            Self::History => "History",
            Self::Stats => "Stats",
            Self::Configuration => "Configuration",
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Self::Dictate => "Ctrl+1",
            Self::History => "Ctrl+2",
            Self::Stats => "Ctrl+3",
            Self::Configuration => "Ctrl+4",
        }
    }

    fn all() -> [Self; 4] {
        [
            Self::Dictate,
            Self::History,
            Self::Stats,
            Self::Configuration,
        ]
    }
}

/// What the status line and mic button show. Mirrors `EngineState` but adds
/// the two states only the UI can know: no model installed, and model warm-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Idle,
    Listening,
    Stopping,
    Transcribing,
    Rewriting,
    Error,
    NoModel,
    Warming,
    Stopped,
}

impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Idle => "Ready",
            Self::Listening => "Listening…",
            Self::Stopping => "Stopping…",
            Self::Transcribing => "Transcribing…",
            Self::Rewriting => "Cleaning up…",
            Self::Error => "Something went wrong",
            Self::NoModel => "No speech model installed",
            Self::Warming => "Preparing models…",
            Self::Stopped => "Stopped",
        }
    }

    pub fn color(self) -> gpui::Rgba {
        match self {
            Self::Listening | Self::Transcribing | Self::Rewriting => theme::accent(),
            Self::Error | Self::NoModel => theme::error(),
            _ => theme::text_muted(),
        }
    }

    fn from_engine(state: EngineState) -> Self {
        match state {
            EngineState::Idle => Self::Idle,
            EngineState::Listening => Self::Listening,
            EngineState::Stopping => Self::Stopping,
            EngineState::Transcribing => Self::Transcribing,
            EngineState::Rewriting => Self::Rewriting,
            EngineState::Error => Self::Error,
            EngineState::NoModel => Self::NoModel,
            EngineState::Warming => Self::Warming,
            EngineState::Stopped => Self::Stopped,
        }
    }
}

pub struct Utterance {
    pub id: u64,
    pub raw: String,
    pub cleaned: String,
    pub streaming: bool,
}

impl Utterance {
    pub fn text(&self) -> &str {
        if self.cleaned.trim().is_empty() {
            &self.raw
        } else {
            &self.cleaned
        }
    }
}

#[derive(Default)]
pub struct Session {
    pub speech_secs: f32,
    pub words: usize,
}

impl Session {
    pub fn wpm(&self) -> usize {
        if self.speech_secs > 0.5 {
            (self.words as f32 / (self.speech_secs / 60.0)).round() as usize
        } else {
            0
        }
    }

    pub fn elapsed(&self) -> String {
        let secs = self.speech_secs.max(0.0) as u64;
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

pub struct FloureApp {
    pub destination: Destination,
    pub engine: Option<Arc<Engine>>,
    pub config: AppConfig,
    pub version: &'static str,
    pub recording: bool,
    pub status: RunState,
    pub utterances: Vec<Utterance>,
    pub session: Session,
    pub mic_level: f32,
    pub reduce_motion: bool,
    pub engine_error: Option<String>,
    pub hotkey_status: HotkeyStatus,
    pub history: Vec<TranscriptRow>,
    pub insights: Option<Insights>,
    pub history_error: Option<String>,
    activate_next_frame: bool,
    focus_handle: FocusHandle,
}

impl FloureApp {
    pub fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        config: AppConfig,
        engine: Option<Arc<Engine>>,
        version: &'static str,
        hotkey_status: HotkeyStatus,
    ) -> Self {
        let db = floure_core::config::history_db_path();
        let history_repo = History::new(&db);
        let (history, history_error) = match history_repo.recent(200) {
            Ok(rows) => (rows, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let insights = InsightsRepo::new(&db).summary().ok();

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        let warming = engine.as_ref().is_some_and(|engine| engine.is_warming());
        Self {
            destination: Destination::Dictate,
            engine,
            config,
            version,
            recording: false,
            status: if warming {
                RunState::Warming
            } else {
                RunState::Idle
            },
            utterances: Vec::new(),
            session: Session::default(),
            mic_level: 0.0,
            reduce_motion: detect_reduce_motion(),
            engine_error: None,
            hotkey_status,
            history,
            insights,
            history_error,
            activate_next_frame: false,
            focus_handle,
        }
    }

    /// Drain engine events on the foreground executor and apply external
    /// show requests (tray, loopback) without blocking either.
    pub fn attach_events(&self, cx: &mut Context<Self>) {
        if let Some(engine) = self.engine.clone() {
            let mut rx = engine.subscribe();
            cx.spawn(async move |this, cx| {
                while let Ok(event) = rx.recv().await {
                    if this
                        .update(cx, |this, cx| {
                            this.handle_event(event);
                            cx.notify();
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        }

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(250))
                    .await;
                let Ok(()) = this.update(cx, |this, cx| {
                    let mut changed = false;
                    if crate::control_server::take_show_request() {
                        this.activate_next_frame = true;
                        changed = true;
                    }
                    if crate::control_server::take_dictate_request() {
                        this.destination = Destination::Dictate;
                        this.activate_next_frame = true;
                        changed = true;
                    }
                    if crate::control_server::take_config_request() {
                        this.destination = Destination::Configuration;
                        this.activate_next_frame = true;
                        changed = true;
                    }
                    if changed {
                        cx.notify();
                    }
                }) else {
                    break;
                };
            }
        })
        .detach();
    }

    fn handle_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::State(state) => {
                self.status = RunState::from_engine(state);
                self.recording = matches!(
                    state,
                    EngineState::Listening | EngineState::Transcribing | EngineState::Rewriting
                );
                if matches!(state, EngineState::Idle | EngineState::Stopped) {
                    self.recording = false;
                }
            }
            EngineEvent::MicLevel(level) => self.mic_level = level,
            EngineEvent::Final {
                utterance_id,
                text,
                duration_sec,
                ..
            } => {
                self.recording = true;
                self.session.speech_secs += duration_sec.max(0.0);
                self.session.words += text.split_whitespace().count();
                self.utterances.push(Utterance {
                    id: utterance_id,
                    cleaned: String::new(),
                    raw: text,
                    streaming: false,
                });
                self.status = RunState::Transcribing;
            }
            EngineEvent::CleanupStarted { utterance_id } => {
                if let Some(line) = self.utterance(utterance_id) {
                    line.streaming = true;
                    line.cleaned.clear();
                }
                self.status = RunState::Rewriting;
            }
            EngineEvent::CleanupToken {
                utterance_id,
                token,
            } => {
                if let Some(line) = self.utterance(utterance_id) {
                    line.cleaned.push_str(&token);
                }
            }
            EngineEvent::CleanupDone { utterance_id, text } => {
                if let Some(line) = self.utterance(utterance_id) {
                    line.cleaned = text;
                    line.streaming = false;
                }
                self.status = if self.recording {
                    RunState::Listening
                } else {
                    RunState::Idle
                };
            }
            EngineEvent::CleanupFailed {
                utterance_id,
                message,
            } => {
                if let Some(line) = self.utterance(utterance_id) {
                    line.streaming = false;
                }
                self.engine_error = Some(message);
            }
            EngineEvent::OutputFailed { message, .. } => {
                self.engine_error = Some(message);
            }
            EngineEvent::Committed { .. } => {
                self.refresh_data();
                self.status = if self.recording {
                    RunState::Listening
                } else {
                    RunState::Idle
                };
            }
            EngineEvent::Error { message, .. } => {
                let lower = message.to_lowercase();
                self.status = if lower.contains("not downloaded")
                    || lower.contains("downloading")
                    || lower.contains("model")
                {
                    RunState::NoModel
                } else {
                    RunState::Error
                };
                self.recording = false;
                self.engine_error = Some(message);
            }
            EngineEvent::AsrReady { .. } => {
                if self.status == RunState::Warming {
                    self.status = RunState::Idle;
                }
            }
            EngineEvent::Stopped { .. } => {
                self.status = RunState::Idle;
                self.recording = false;
                self.mic_level = 0.0;
            }
            EngineEvent::Cancelled { .. } => {
                if let Some(line) = self.utterances.last_mut() {
                    line.streaming = false;
                }
                self.status = RunState::Idle;
                self.recording = false;
            }
            EngineEvent::ModelProgress { .. }
            | EngineEvent::ModelDone { .. }
            | EngineEvent::ModelFailed { .. } => {}
        }
    }

    fn utterance(&mut self, id: u64) -> Option<&mut Utterance> {
        self.utterances.iter_mut().find(|line| line.id == id)
    }

    pub fn copy_latest(&mut self) {
        if let Some(line) = self.utterances.last() {
            let _ = copy_to_clipboard(line.text());
        }
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if let Some(engine) = &self.engine {
            engine.cancel();
        }
        self.recording = false;
        self.status = RunState::Idle;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.utterances.clear();
        self.session = Session::default();
        cx.notify();
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.engine_error = None;
        if self.status == RunState::Error || self.status == RunState::NoModel {
            self.status = RunState::Idle;
        }
        cx.notify();
    }

    pub fn go(&mut self, destination: Destination, cx: &mut Context<Self>) {
        if self.destination != destination {
            self.destination = destination;
            self.refresh_data();
            cx.notify();
        }
    }

    fn refresh_data(&mut self) {
        let db = floure_core::config::history_db_path();
        match History::new(&db).recent(200) {
            Ok(rows) => {
                self.history = rows;
                self.history_error = None;
            }
            Err(error) => self.history_error = Some(error.to_string()),
        }
        self.insights = InsightsRepo::new(&db).summary().ok();
    }

    pub fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.engine_error = Some("Engine unavailable".to_string());
            cx.notify();
            return;
        };
        if self.recording {
            self.recording = false;
            self.status = RunState::Stopping;
            engine.stop();
        } else {
            match engine.start() {
                Ok(()) => {
                    self.recording = true;
                    self.status = RunState::Listening;
                    self.engine_error = None;
                }
                Err(error) => {
                    let message = error.to_string();
                    let lower = message.to_lowercase();
                    self.recording = false;
                    self.status = if lower.contains("model") {
                        RunState::NoModel
                    } else {
                        RunState::Error
                    };
                    self.engine_error = Some(message);
                }
            }
        }
        cx.notify();
    }

    fn on_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }
}

/// Linux users think in Ctrl, not CommandOrControl. The stored value stays in
/// the frontend's cross-platform format; only the label is localised.
pub fn display_hotkey(hotkey: &str) -> String {
    hotkey
        .replace("CommandOrControl", "Ctrl")
        .replace("CmdOrCtrl", "Ctrl")
        .replace('+', " + ")
}

fn detect_reduce_motion() -> bool {
    if std::env::var("FLOURE_REDUCE_MOTION").is_ok_and(|v| v == "1" || v == "true") {
        return true;
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let output = if desktop.to_lowercase().contains("gnome") {
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
    } else if desktop.to_lowercase().contains("kde") {
        std::process::Command::new("kreadconfig6")
            .args([
                "--group",
                "KDE",
                "--key",
                "AnimationDurationFactor",
                "--file",
                "kdeglobals",
            ])
            .output()
    } else {
        return false;
    };
    match output {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stdout);
            text.trim() == "false" || text.trim() == "0" || text.trim() == "0.0"
        }
        Err(_) => false,
    }
}

pub fn key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("ctrl-1", GoDictate, None),
        KeyBinding::new("ctrl-2", GoHistory, None),
        KeyBinding::new("ctrl-3", GoStats, None),
        KeyBinding::new("ctrl-4", GoConfig, None),
        KeyBinding::new("ctrl-q", Quit, None),
    ]
}

impl Render for FloureApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.activate_next_frame {
            self.activate_next_frame = false;
            window.activate_window();
        }
        div()
            .id("floure-root")
            .role(gpui::Role::Application)
            .aria_label("Floure")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &GoDictate, _, cx| this.go(Destination::Dictate, cx)))
            .on_action(cx.listener(|this, _: &GoHistory, _, cx| this.go(Destination::History, cx)))
            .on_action(cx.listener(|this, _: &GoStats, _, cx| this.go(Destination::Stats, cx)))
            .on_action(
                cx.listener(|this, _: &GoConfig, _, cx| this.go(Destination::Configuration, cx)),
            )
            .on_action(cx.listener(Self::on_quit))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme::ground())
            .text_color(theme::text())
            .font_family(theme::FONT_FAMILY)
            .child(self.render_rail(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(self.render_titlebar(cx))
                    .child(self.render_content(cx)),
            )
    }
}

impl FloureApp {
    fn render_rail(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut rail = div()
            .flex()
            .flex_col()
            .items_center()
            .w(theme::RAIL_WIDTH)
            .h_full()
            .py(theme::SPACE_3)
            .border_r_1()
            .border_color(theme::border())
            .bg(theme::ground());

        // Product mark: a white monogram, not a logo lockup.
        rail = rail.child(
            div()
                .size(px(20.0))
                .rounded(px(9999.0))
                .bg(theme::text())
                .flex()
                .items_center()
                .justify_center()
                .mb(theme::SPACE_6)
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_weight(theme::semibold())
                        .text_color(theme::ground())
                        .child("F"),
                ),
        );

        for destination in Destination::all() {
            let active = self.destination == destination;
            rail = rail.child(nav_item(destination, active).on_click(cx.listener(
                move |this, _, _, cx| {
                    this.go(destination, cx);
                },
            )));
        }

        rail.child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(theme::SPACE_1)
                    .child(
                        div()
                            .size(px(5.0))
                            .rounded(px(9999.0))
                            .bg(if self.recording {
                                theme::accent()
                            } else {
                                theme::raised()
                            }),
                    )
                    .child(
                        div()
                            .text_size(theme::TEXT_XS)
                            .text_color(theme::text_faint())
                            .child(format!("v{}", self.version)),
                    ),
            )
            .into_any_element()
    }

    fn render_titlebar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(theme::TITLEBAR_HEIGHT)
            .px(theme::GUTTER)
            .border_b_1()
            .border_color(theme::border())
            .bg(theme::ground())
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    window.start_window_move();
                }
            })
            .child(
                div()
                    .flex_1()
                    .text_size(theme::TEXT_SM)
                    .font_weight(theme::semibold())
                    .text_color(theme::text())
                    .child(self.destination.label()),
            )
            .child(
                div()
                    .mr(theme::SPACE_4)
                    .text_size(theme::TEXT_XS)
                    .text_color(theme::text_faint())
                    .child(self.destination.shortcut()),
            )
            .child(window_controls(cx))
            .into_any_element()
    }

    fn render_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.destination {
            Destination::Dictate => views::dictate::render(self, cx),
            Destination::History => views::history::render(self, cx),
            Destination::Stats => views::stats::render(self, cx),
            Destination::Configuration => views::config::render(self, cx),
        }
    }
}

fn nav_item(destination: Destination, active: bool) -> gpui::Stateful<gpui::Div> {
    let color = if active {
        theme::text()
    } else {
        theme::text_faint()
    };
    let mut item = div()
        .id(SharedString::from(destination.label()))
        .role(gpui::Role::Button)
        .aria_label(destination.label())
        .size(px(40.0))
        .mb(theme::SPACE_1)
        .flex()
        .items_center()
        .justify_center()
        .rounded(theme::RADIUS_CONTROL)
        .cursor_pointer()
        .hover(|style| style.bg(theme::surface()));
    if active {
        item = item.bg(theme::raised());
    }
    item.child(icon(destination, color))
}

fn icon(destination: Destination, color: gpui::Rgba) -> AnyElement {
    match destination {
        Destination::Dictate => mic_icon(color),
        Destination::History => clock_icon(color),
        Destination::Stats => bars_icon(color),
        Destination::Configuration => sliders_icon(color),
    }
}

fn mic_icon(color: gpui::Rgba) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w(px(7.0))
                .h(px(12.0))
                .rounded(px(4.0))
                .border_1()
                .border_color(color),
        )
        .child(
            div()
                .w(px(13.0))
                .h(px(7.0))
                .mt(px(-3.0))
                .border_1()
                .border_t_0()
                .rounded_b(px(7.0))
                .border_color(color),
        )
        .child(div().w(px(1.5)).h(px(3.0)).bg(color))
        .into_any_element()
}

fn clock_icon(color: gpui::Rgba) -> AnyElement {
    div()
        .relative()
        .size(px(16.0))
        .rounded(px(9999.0))
        .border_1()
        .border_color(color)
        .child(
            div()
                .absolute()
                .left(px(7.0))
                .top(px(4.0))
                .w(px(1.5))
                .h(px(4.5))
                .bg(color),
        )
        .child(
            div()
                .absolute()
                .left(px(7.0))
                .top(px(7.5))
                .w(px(4.5))
                .h(px(1.5))
                .bg(color),
        )
        .into_any_element()
}

fn bars_icon(color: gpui::Rgba) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_end()
        .gap(px(2.5))
        .h(px(15.0))
        .child(div().w(px(3.0)).h(px(7.0)).rounded(px(1.0)).bg(color))
        .child(div().w(px(3.0)).h(px(15.0)).rounded(px(1.0)).bg(color))
        .child(div().w(px(3.0)).h(px(10.0)).rounded(px(1.0)).bg(color))
        .into_any_element()
}

fn sliders_icon(color: gpui::Rgba) -> AnyElement {
    let row = |dot_left: f32| {
        div().relative().w(px(16.0)).h(px(1.0)).bg(color).child(
            div()
                .absolute()
                .left(px(dot_left))
                .top(px(-2.0))
                .size(px(5.0))
                .rounded(px(9999.0))
                .bg(color),
        )
    };
    div()
        .flex()
        .flex_col()
        .gap(px(4.5))
        .child(row(3.0))
        .child(row(9.0))
        .child(row(5.0))
        .into_any_element()
}

fn window_controls(cx: &mut Context<FloureApp>) -> AnyElement {
    // 32 px hit targets with 16 px glyphs; the glyphs are drawn, not font
    // characters, so they stay crisp at any scale.
    let control = |id: &'static str, glyph: AnyElement| {
        div()
            .id(id)
            .role(gpui::Role::Button)
            .aria_label(id)
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(theme::raised()))
            .child(glyph)
    };

    div()
        .flex()
        .flex_row()
        .gap(px(4.0))
        .child(
            control(
                "Minimize",
                div()
                    .w(px(12.0))
                    .h(px(1.0))
                    .bg(theme::text_secondary())
                    .into_any_element(),
            )
            .on_click(|_, window, _| window.minimize_window()),
        )
        .child(
            control(
                "Maximize",
                div()
                    .size(px(10.0))
                    .border_1()
                    .border_color(theme::text_secondary())
                    .into_any_element(),
            )
            .on_click(|_, window, _| window.zoom_window()),
        )
        .child(
            control("Close", {
                div()
                    .text_size(px(14.0))
                    .text_color(theme::text_secondary())
                    .child("×")
                    .into_any_element()
            })
            .on_click(cx.listener(|_, _, _, cx| cx.quit())),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_destination_has_a_unique_label_and_shortcut() {
        let labels: std::collections::HashSet<_> =
            Destination::all().iter().map(|d| d.label()).collect();
        let shortcuts: std::collections::HashSet<_> =
            Destination::all().iter().map(|d| d.shortcut()).collect();
        assert_eq!(labels.len(), 4);
        assert_eq!(shortcuts.len(), 4);
    }

    #[test]
    fn hotkey_label_is_localised_without_changing_storage() {
        assert_eq!(
            display_hotkey("CommandOrControl+Shift+F12"),
            "Ctrl + Shift + F12"
        );
        // The stored value must stay in the cross-platform format.
        assert_eq!(AppConfig::default().hotkey, "CommandOrControl+Shift+F12");
    }
}
