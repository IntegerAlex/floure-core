//! Application state and the window shell: title bar, rail, destinations.

use std::sync::Arc;

use std::collections::{HashMap, HashSet};

use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable, IntoElement, KeyBinding, MouseButton,
    ParentElement, Render, SharedString, Styled, Window, actions, div, prelude::*, px,
};
use gpui_component::input::InputState;

use floure_core::config::{AppConfig, AsrProfile, LlmProvider, PttMode, SettingsUpdate};
use floure_core::llm::LlmMode;
use floure_core::models::{ModelManager, ModelStatus};
use floure_core::output::copy_to_clipboard;
use floure_core::{
    Dictionary, DictionaryEntry, Engine, EngineEvent, EngineState, History, Insights, InsightsRepo,
    TranscriptRow,
};

use crate::hotkeys::HotkeyStatus;
use crate::theme;
use crate::views;

actions!(
    floure,
    [GoDictate, GoHistory, GoStats, GoConfig, FocusSearch, Quit]
);

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

    /// The widget is status-only: it exists while there is something to say.
    pub fn shows_widget(self) -> bool {
        matches!(
            self,
            Self::Listening | Self::Stopping | Self::Transcribing | Self::Rewriting
        )
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
    /// True once the engine has assembled this group into one sentence and
    /// run the deterministic/Polish pass over it.
    pub finalized: bool,
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

/// Replace the raw VAD lines of one assembled sentence with its final text.
/// The worker may emit several `Final` events before one `CleanupDone`, so the
/// panel must collapse them rather than showing a lower-case fragment as its
/// own sentence.
fn merge_finalized(utterances: &mut Vec<Utterance>, utterance_id: u64, text: String) {
    let first = utterances
        .iter()
        .position(|line| !line.finalized && line.id <= utterance_id);
    if let Some(first) = first {
        let last = utterances
            .iter()
            .rposition(|line| !line.finalized && line.id <= utterance_id)
            .unwrap_or(first);
        utterances.drain(first..=last);
        utterances.insert(
            first,
            Utterance {
                id: utterance_id,
                raw: text.clone(),
                cleaned: text,
                streaming: false,
                finalized: true,
            },
        );
    } else if let Some(line) = utterances.iter_mut().find(|line| line.id == utterance_id) {
        line.cleaned = text;
        line.streaming = false;
        line.finalized = true;
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
    pub transcript_copied: bool,
    pub session: Session,
    pub mic_level: f32,
    pub reduce_motion: bool,
    pub engine_error: Option<String>,
    pub hotkey_status: HotkeyStatus,
    pub widget: crate::widget::WidgetHost,
    pub history: Vec<TranscriptRow>,
    pub history_query: String,
    pub history_favorites_only: bool,
    pub history_mode: Option<&'static str>,
    pub history_limit: usize,
    pub history_selected: HashSet<i64>,
    pub history_expanded: HashSet<i64>,
    pub history_confirm_row: Option<i64>,
    pub history_today: String,
    pub history_yesterday: String,
    pub history_confirm_delete: Option<usize>,
    pub history_notice: Option<String>,
    pub search_input: Entity<InputState>,
    pub insights: Option<Insights>,
    pub history_error: Option<String>,
    pub recent_errors: Vec<String>,
    pub config_section: usize,
    pub config_dirty: bool,
    pub config_error: Option<String>,
    pub config_notice: Option<String>,
    pub model_statuses: Vec<ModelStatus>,
    pub model_progress: HashMap<String, u8>,
    pub dictionary_entries: Vec<DictionaryEntry>,
    pub dictionary_phrase: Entity<InputState>,
    pub dictionary_replacement: Entity<InputState>,
    pub api_key_input: Entity<InputState>,
    pub llm_model_input: Entity<InputState>,
    pub hotwords_input: Entity<InputState>,
    pub mic_devices: Vec<(String, String)>,
    pub helper_binaries: Vec<(&'static str, bool)>,
    pub mic_testing: bool,
    pub onboarding_step: usize,
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
        let (history_today, history_yesterday) = History::new(&db).today_and_yesterday();

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        window.set_window_title("Floure");
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search transcripts"));
        let dictionary_phrase =
            cx.new(|cx| InputState::new(window, cx).placeholder("Heard phrase"));
        let dictionary_replacement =
            cx.new(|cx| InputState::new(window, cx).placeholder("Replace with"));
        let api_key_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("OpenRouter API key"));
        let llm_model_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("openai/gpt-4o-mini")
                .default_value(config.llm_model.clone())
        });
        let hotwords_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Names, jargon, acronyms"));
        let mic_devices = floure_core::audio::list_input_devices().unwrap_or_default();
        let helper_binaries = ["wtype", "xdotool", "wl-copy", "ydotool"]
            .into_iter()
            .map(|name| {
                let found = std::process::Command::new("sh")
                    .args(["-c", &format!("command -v {name}")])
                    .output()
                    .is_ok_and(|out| out.status.success());
                (name, found)
            })
            .collect();
        let model_statuses = ModelManager::new(config.model_dir.clone()).status();
        let dictionary_entries = Dictionary::new(floure_core::config::history_db_path())
            .list(None, None, false)
            .unwrap_or_default();
        let recent_errors = load_recent_errors();

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
            transcript_copied: false,
            session: Session::default(),
            mic_level: 0.0,
            reduce_motion: detect_reduce_motion(),
            engine_error: None,
            hotkey_status,
            widget: crate::widget::WidgetHost::new(),
            history,
            history_query: String::new(),
            history_favorites_only: false,
            history_mode: None,
            history_limit: 50,
            history_selected: HashSet::new(),
            history_expanded: HashSet::new(),
            history_confirm_row: None,
            history_today,
            history_yesterday,
            history_confirm_delete: None,
            history_notice: None,
            search_input,
            config_section: 0,
            config_dirty: false,
            config_error: None,
            config_notice: None,
            model_statuses,
            model_progress: HashMap::new(),
            dictionary_entries,
            dictionary_phrase,
            dictionary_replacement,
            api_key_input,
            llm_model_input,
            hotwords_input,
            mic_devices,
            helper_binaries,
            mic_testing: false,
            onboarding_step: 0,
            insights,
            history_error,
            recent_errors,
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
                            this.sync_widget(cx);
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

        let search = self.search_input.clone();
        cx.observe(&search, |this, input, cx| {
            let query = input.read(cx).value().to_string();
            if this.history_query != query {
                this.history_query = query;
                this.history_limit = 50;
                this.refresh_history();
                cx.notify();
            }
        })
        .detach();

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
                    if crate::control_server::take_widget_request() {
                        this.show_widget(cx);
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
                // `recording` means the microphone is open, nothing else.
                // Transcribing and rewriting happen after stop too.
                self.recording = state == EngineState::Listening;
            }
            EngineEvent::MicLevel(level) => self.mic_level = level,
            EngineEvent::Final {
                utterance_id,
                text,
                duration_sec,
                ..
            } => {
                self.transcript_copied = false;
                self.session.speech_secs += duration_sec.max(0.0);
                self.session.words += text.split_whitespace().count();
                self.utterances.push(Utterance {
                    id: utterance_id,
                    cleaned: String::new(),
                    raw: text,
                    streaming: false,
                    finalized: false,
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
                merge_finalized(&mut self.utterances, utterance_id, text);
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
                self.record_error(message);
            }
            EngineEvent::OutputFailed { message, .. } => {
                self.record_error(message);
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
                self.record_error(message);
            }
            EngineEvent::Stopped { .. } => {
                self.status = RunState::Idle;
                self.recording = false;
                self.mic_level = 0.0;
            }
            EngineEvent::Cancelled { .. } => {
                // Drop the in-flight group entirely; its raw segments were
                // never finalised and must not remain in the panel.
                self.utterances.retain(|line| line.finalized);
                self.status = RunState::Idle;
                self.recording = false;
            }
            EngineEvent::ModelProgress { id, percent, .. } => {
                self.model_progress.insert(id, percent);
            }
            EngineEvent::ModelDone { id } => {
                self.model_progress.remove(&id);
                self.refresh_models();
            }
            EngineEvent::ModelFailed { id, message } => {
                self.model_progress.remove(&id);
                self.config_error = Some(format!("{id}: {message}"));
                self.refresh_models();
            }
            EngineEvent::AsrReady { .. } => {
                if self.status == RunState::Warming {
                    self.status = RunState::Idle;
                }
            }
        }
    }

    /// Keep the layer-shell widget in step with the engine. The widget owns
    /// its own window; this only feeds it the state it must not guess.
    fn sync_widget(&mut self, cx: &mut Context<Self>) {
        self.widget.sync(self.status, self.mic_level, cx);
    }

    /// Tray "Show widget": visible while idle too, showing the real state.
    pub fn show_widget(&mut self, cx: &mut Context<Self>) {
        if !self.widget.supported() {
            self.config_notice = Some(self.widget.status());
            return;
        }
        self.widget.show_manual(self.status, self.mic_level, cx);
    }

    /// Exposed in Diagnostics: the widget is optional, and its absence must be
    /// explainable rather than invisible.
    pub fn widget_supported(&self) -> bool {
        self.widget.supported()
    }

    pub fn widget_status(&self) -> String {
        self.widget.status()
    }

    /// Persist an error so it survives the toast/banner disappearing. The
    /// Diagnostics section shows the same list.
    fn record_error(&mut self, message: String) {
        self.engine_error = Some(message.clone());
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.recent_errors.push(format!("[{stamp}] {message}"));
        if self.recent_errors.len() > 50 {
            self.recent_errors.remove(0);
        }
        let path = floure_core::config::data_dir().join("recent-errors.log");
        let _ = std::fs::create_dir_all(floure_core::config::data_dir());
        let _ = std::fs::write(&path, self.recent_errors.join("\n"));
    }

    fn utterance(&mut self, id: u64) -> Option<&mut Utterance> {
        self.utterances.iter_mut().find(|line| line.id == id)
    }

    pub fn copy_transcript(&mut self) {
        let start = self.utterances.len().saturating_sub(4);
        let text = self.utterances[start..]
            .iter()
            .map(Utterance::text)
            .collect::<Vec<_>>()
            .join("\n\n");
        if !text.is_empty() {
            let _ = copy_to_clipboard(&text);
            self.transcript_copied = true;
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
        self.transcript_copied = false;
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

    pub fn go_models(&mut self, cx: &mut Context<Self>) {
        self.destination = Destination::Configuration;
        self.config_section = 2;
        self.refresh_data();
        cx.notify();
    }

    pub fn go(&mut self, destination: Destination, cx: &mut Context<Self>) {
        if self.destination != destination {
            self.destination = destination;
            if destination == Destination::History {
                self.refresh_history();
            }
            self.refresh_data();
            cx.notify();
        }
    }

    fn refresh_data(&mut self) {
        let db = floure_core::config::history_db_path();
        self.insights = InsightsRepo::new(&db).summary().ok();
        if self.destination == Destination::History {
            self.refresh_history();
        }
    }

    /// Load the current filter into `history`. Search is FTS-backed and
    /// immediate; filtering runs before the UI sees the rows.
    pub fn refresh_history(&mut self) {
        let db = floure_core::config::history_db_path();
        let repo = History::new(&db);
        let (today, yesterday) = repo.today_and_yesterday();
        if !today.is_empty() {
            self.history_today = today;
            self.history_yesterday = yesterday;
        }
        let query = self.history_query.trim();
        let result = if query.is_empty() {
            repo.recent(self.history_limit)
        } else {
            repo.search(query, self.history_limit)
        };
        match result {
            Ok(rows) => {
                self.history = rows
                    .into_iter()
                    .filter(|row| {
                        (!self.history_favorites_only || row.favorite != 0)
                            && self.history_mode.is_none_or(|mode| row.mode == mode)
                    })
                    .collect();
                self.history_error = None;
            }
            Err(error) => self.history_error = Some(error.to_string()),
        }
    }

    pub fn toggle_history_mode(&mut self, mode: Option<&'static str>) {
        self.history_mode = if self.history_mode == mode {
            None
        } else {
            mode
        };
        self.history_limit = 50;
        self.refresh_history();
    }

    pub fn toggle_favorite_filter(&mut self) {
        self.history_favorites_only = !self.history_favorites_only;
        self.history_limit = 50;
        self.refresh_history();
    }

    pub fn toggle_selected(&mut self, id: i64) {
        if !self.history_selected.insert(id) {
            self.history_selected.remove(&id);
        }
        self.history_confirm_delete = None;
        self.history_confirm_row = None;
    }

    pub fn toggle_expanded(&mut self, id: i64) {
        if !self.history_expanded.insert(id) {
            self.history_expanded.remove(&id);
        }
    }

    pub fn set_favorite(&mut self, id: i64, favorite: bool) {
        let db = floure_core::config::history_db_path();
        let _ = History::new(&db).set_favorite(id, favorite);
        self.refresh_history();
    }

    /// Two-step delete: the first click arms, the second deletes. A transcript
    /// is not recoverable, so a single stray click must not destroy one.
    pub fn delete_history_row(&mut self, id: i64) {
        if self.history_confirm_row != Some(id) {
            self.history_confirm_row = Some(id);
            return;
        }
        let db = floure_core::config::history_db_path();
        let _ = History::new(&db).delete(&[id]);
        self.history_selected.remove(&id);
        self.history_confirm_row = None;
        self.refresh_history();
        self.insights = InsightsRepo::new(&db).summary().ok();
    }

    pub fn confirm_delete_selected(&mut self) {
        let ids: Vec<i64> = self.history_selected.iter().copied().collect();
        let db = floure_core::config::history_db_path();
        let _ = History::new(&db).delete(&ids);
        self.history_selected.clear();
        self.history_confirm_delete = None;
        self.refresh_history();
        self.insights = InsightsRepo::new(&db).summary().ok();
    }

    pub fn load_more_history(&mut self) {
        self.history_limit += 50;
        self.refresh_history();
    }

    /// Export exactly what the screen shows, so the count in the notice is
    /// the count in the file.
    pub fn export_history(&mut self) {
        let csv = History::new(floure_core::config::history_db_path()).export_csv(&self.history);
        let path = floure_core::config::data_dir().join("history-export.csv");
        match std::fs::write(&path, csv) {
            Ok(()) => {
                self.history_notice = Some(format!(
                    "Exported {} transcript{} to {}",
                    self.history.len(),
                    if self.history.len() == 1 { "" } else { "s" },
                    path.display()
                ));
            }
            Err(error) => self.history_notice = Some(format!("Export failed: {error}")),
        }
    }

    pub fn set_config_section(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.config_section != index {
            self.config_section = index;
            self.config_error = None;
            self.config_notice = None;
            cx.notify();
        }
    }

    fn refresh_models(&mut self) {
        self.model_statuses = ModelManager::new(self.config.model_dir.clone()).status();
    }

    pub fn refresh_dictionary(&mut self) {
        self.dictionary_entries = Dictionary::new(floure_core::config::history_db_path())
            .list(None, None, false)
            .unwrap_or_default();
    }

    pub fn save_config_now(&mut self) {
        match self.config.save() {
            Ok(()) => {
                self.config_error = None;
                self.config_notice = Some("Saved".to_string());
            }
            Err(error) => self.config_error = Some(error.to_string()),
        }
    }

    /// Engine-affecting rows stage; this is the one explicit apply.
    pub fn apply_config(&mut self, cx: &mut Context<Self>) {
        if self.recording {
            self.config_error = Some("Stop recording before applying settings.".to_string());
            cx.notify();
            return;
        }
        let update = SettingsUpdate {
            asr_profile: self.config.asr_profile.clone(),
            language: self.config.language.clone(),
            llm_provider: self.config.llm_provider.clone(),
            llm_mode: self.config.llm_mode,
            llm_model: self.config.llm_model.clone(),
            typing_enabled: self.config.typing_enabled,
            clipboard_enabled: self.config.clipboard_enabled,
            hotwords: self.config.hotwords.clone(),
        };
        if let Some(engine) = &self.engine {
            engine.apply_settings(update);
        }
        self.config_dirty = false;
        self.save_config_now();
        if let Some(engine) = &self.engine {
            engine.warm();
        }
        cx.notify();
    }

    pub fn cycle_language(&mut self) {
        const LANGS: [&str; 5] = ["en", "de", "es", "fr", "auto"];
        let index = LANGS
            .iter()
            .position(|lang| *lang == self.config.language)
            .unwrap_or(0);
        self.config.language = LANGS[(index + 1) % LANGS.len()].to_string();
        self.config_dirty = true;
    }

    pub fn cycle_profile(&mut self) {
        self.config.asr_profile = match self.config.asr_profile {
            AsrProfile::Parakeet => AsrProfile::ParakeetUltra,
            AsrProfile::ParakeetUltra => AsrProfile::WhisperTurbo,
            AsrProfile::WhisperTurbo => AsrProfile::WhisperBase,
            AsrProfile::WhisperBase => AsrProfile::Parakeet,
        };
        self.config_dirty = true;
    }

    pub fn cycle_llm_mode(&mut self) {
        self.config.llm_mode = match self.config.llm_mode {
            LlmMode::Off => LlmMode::Cleanup,
            LlmMode::Cleanup => LlmMode::BulletList,
            LlmMode::BulletList => LlmMode::Email,
            LlmMode::Email => LlmMode::CommitMessage,
            LlmMode::CommitMessage => LlmMode::Off,
        };
        self.config_dirty = true;
    }

    pub fn toggle_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let going_cloud = matches!(self.config.llm_provider, LlmProvider::Local);
        self.config.llm_provider = if going_cloud {
            LlmProvider::OpenRouter
        } else {
            LlmProvider::Local
        };
        // The two providers use different ids. Never carry a local id into an
        // OpenRouter request (or the reverse) after a provider switch.
        if going_cloud && !self.config.llm_model.contains('/') {
            self.config.llm_model = "openai/gpt-4o-mini".to_string();
        } else if !going_cloud
            && (self.config.llm_model.contains('/') || self.config.llm_model.trim().is_empty())
        {
            self.config.llm_model = "s1-mini-q4_k_m".to_string();
        }
        if going_cloud {
            let value = self.config.llm_model.clone();
            self.llm_model_input
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.config_dirty = true;
    }

    pub fn select_local_llm(&mut self, id: &str) {
        self.config.llm_model = id.to_string();
        self.config_dirty = true;
    }

    pub fn toggle_ptt_mode(&mut self) {
        self.config.ptt_mode = match self.config.ptt_mode {
            PttMode::Hold => PttMode::Toggle,
            PttMode::Toggle => PttMode::Hold,
        };
        self.save_config_now();
    }

    pub fn toggle_typing(&mut self) {
        self.config.typing_enabled = !self.config.typing_enabled;
        self.save_config_now();
    }

    pub fn toggle_clipboard(&mut self) {
        self.config.clipboard_enabled = !self.config.clipboard_enabled;
        self.save_config_now();
    }

    pub fn set_mic(&mut self, device: String) {
        self.config.selected_mic_id = if device.is_empty() {
            None
        } else {
            Some(device)
        };
        self.save_config_now();
    }

    pub fn toggle_mic_test(&mut self, cx: &mut Context<Self>) {
        self.mic_testing = !self.mic_testing;
        if let Some(engine) = &self.engine {
            if self.mic_testing {
                if let Err(error) = engine.start_mic_test() {
                    self.mic_testing = false;
                    self.config_error = Some(error.to_string());
                }
            } else {
                engine.stop_mic_test();
            }
        }
        cx.notify();
    }

    pub fn download_model(&mut self, id: &str, cx: &mut Context<Self>) {
        match self.engine.as_ref().map(|engine| engine.download_model(id)) {
            Some(Ok(())) => {
                self.model_progress.insert(id.to_string(), 0);
                self.config_error = None;
            }
            Some(Err(error)) => self.config_error = Some(error.to_string()),
            None => self.config_error = Some("Engine unavailable".to_string()),
        }
        cx.notify();
    }

    pub fn delete_model(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.recording {
            self.config_error = Some("Stop recording before deleting a model.".to_string());
            cx.notify();
            return;
        }
        match floure_core::models::delete_model(&self.config.model_dir, id) {
            Ok(()) => {
                self.refresh_models();
                self.config_notice = Some(format!("Deleted {id}"));
            }
            Err(error) => self.config_error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub fn add_dictionary_entry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let phrase = self.dictionary_phrase.read(cx).value().to_string();
        let replacement = self.dictionary_replacement.read(cx).value().to_string();
        let repo = Dictionary::new(floure_core::config::history_db_path());
        match repo.add(&phrase, &replacement, None, None) {
            Ok(Some(_)) => {
                self.dictionary_phrase
                    .update(cx, |state, cx| state.set_value("", window, cx));
                self.dictionary_replacement
                    .update(cx, |state, cx| state.set_value("", window, cx));
                self.refresh_dictionary();
                self.config_notice = Some("Added".to_string());
                self.config_error = None;
            }
            Ok(None) => {
                self.config_error =
                    Some("Enter a phrase and replacement (60 characters max).".to_string())
            }
            Err(error) => self.config_error = Some(error.to_string()),
        }
        cx.notify();
    }

    pub fn delete_dictionary_entry(&mut self, id: i64) {
        let _ = Dictionary::new(floure_core::config::history_db_path()).delete(id);
        self.refresh_dictionary();
    }

    pub fn toggle_dictionary_favorite(&mut self, id: i64) {
        let _ = Dictionary::new(floure_core::config::history_db_path()).toggle_favorite(id);
        self.refresh_dictionary();
    }

    pub fn save_openrouter_key(&mut self, cx: &mut Context<Self>) {
        let key = self.api_key_input.read(cx).value().to_string();
        floure_core::llm::set_openrouter_api_key(key.trim().to_string());
        self.config_notice = Some("OpenRouter key set for this session".to_string());
        cx.notify();
    }

    pub fn save_dictionary_hotwords(&mut self, cx: &mut Context<Self>) {
        let hotwords = self.hotwords_input.read(cx).value().to_string();
        self.config.hotwords = floure_core::config::normalize_hotwords(&hotwords);
        self.save_config_now();
        if let Some(engine) = &self.engine {
            engine.apply_settings(SettingsUpdate {
                asr_profile: self.config.asr_profile.clone(),
                language: self.config.language.clone(),
                llm_provider: self.config.llm_provider.clone(),
                llm_mode: self.config.llm_mode,
                llm_model: self.config.llm_model.clone(),
                typing_enabled: self.config.typing_enabled,
                clipboard_enabled: self.config.clipboard_enabled,
                hotwords: self.config.hotwords.clone(),
            });
        }
        cx.notify();
    }

    pub fn copy_diagnostics(&mut self, cx: &mut Context<Self>) {
        match floure_core::diagnostics::get_diagnostics() {
            Ok(json) => {
                let _ = copy_to_clipboard(&json);
                self.config_notice = Some("Diagnostics copied".to_string());
            }
            Err(error) => self.config_error = Some(error),
        }
        cx.notify();
    }

    pub fn models_ready(&self) -> bool {
        speech_model_ready(&self.model_statuses)
    }

    pub fn onboarding_next(&mut self, cx: &mut Context<Self>) {
        if !can_advance(self.onboarding_step, self.models_ready()) {
            self.config_error = Some("Download a speech model to continue.".to_string());
            cx.notify();
            return;
        }
        self.onboarding_step = (self.onboarding_step + 1).min(2);
        self.config_error = None;
        cx.notify();
    }

    pub fn onboarding_back(&mut self, cx: &mut Context<Self>) {
        self.onboarding_step = self.onboarding_step.saturating_sub(1);
        self.config_error = None;
        cx.notify();
    }

    pub fn rerun_setup(&mut self, cx: &mut Context<Self>) {
        self.config.onboarding_completed = false;
        self.onboarding_step = 0;
        self.save_config_now();
        cx.notify();
    }

    pub fn discard_config(&mut self, cx: &mut Context<Self>) {
        self.config = AppConfig::load();
        self.config_dirty = false;
        self.config_error = None;
        self.config_notice = Some("Staged changes discarded".to_string());
        cx.notify();
    }

    pub fn complete_onboarding(&mut self, cx: &mut Context<Self>) {
        self.config.onboarding_completed = true;
        self.save_config_now();
        self.destination = Destination::Dictate;
        self.onboarding_step = 2;
        cx.notify();
    }

    pub fn copy_to_clipboard_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let _ = copy_to_clipboard(text);
        self.config_notice = Some("Copied".to_string());
        cx.notify();
    }

    pub fn copy_row(&mut self, text: &str) {
        let _ = copy_to_clipboard(text);
        self.history_notice = Some("Copied".to_string());
    }

    pub fn toggle_recording(&mut self, cx: &mut Context<Self>) {
        let Some(engine) = self.engine.clone() else {
            self.engine_error = Some("Engine unavailable".to_string());
            cx.notify();
            return;
        };
        if engine.state() == EngineState::Stopping {
            return;
        }
        if engine.state() == EngineState::Listening {
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
        self.quit(cx);
    }

    pub fn quit(&mut self, cx: &mut Context<Self>) {
        // Flush the tail before the process exits; the engine may be recording
        // even when the window is not the focused surface.
        if let Some(engine) = &self.engine {
            engine.stop_blocking();
        }
        cx.quit();
    }
}

fn speech_model_ready(statuses: &[ModelStatus]) -> bool {
    statuses.iter().any(|model| {
        model.downloaded
            && matches!(
                model.id.as_str(),
                "parakeet-tdt-0.6b-v2-int8" | "whisper-large-v3-turbo-q5_1" | "whisper-base-q5_1"
            )
    })
}

fn can_advance(step: usize, models_ready: bool) -> bool {
    step != 1 || models_ready
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
        KeyBinding::new("ctrl-f", FocusSearch, None),
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
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.destination = Destination::History;
                this.refresh_history();
                let handle = this.search_input.read(cx).focus_handle(cx);
                window.focus(&handle, cx);
                cx.notify();
            }))
            .on_action(cx.listener(Self::on_quit))
            .size_full()
            .flex()
            .flex_row()
            .bg(theme::ground())
            .text_color(theme::text())
            .font_family(theme::FONT_FAMILY)
            .when(!self.config.onboarding_completed, |root| root)
            .when(self.config.onboarding_completed, |root| {
                root.child(self.render_rail(cx))
            })
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
                    .child(if self.config.onboarding_completed {
                        self.destination.label()
                    } else {
                        "Setup"
                    }),
            )
            .when(self.recording, |bar| {
                bar.child(
                    div()
                        .mr(theme::SPACE_4)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(div().size(px(6.0)).rounded(px(9999.0)).bg(theme::accent()))
                        .child(
                            div()
                                .text_size(theme::TEXT_XS)
                                .text_color(theme::accent())
                                .child("Recording"),
                        ),
                )
            })
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
        if !self.config.onboarding_completed {
            return views::onboarding::render(self, cx);
        }
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
        .hover(|style| style.bg(theme::hover()));
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

fn load_recent_errors() -> Vec<String> {
    std::fs::read_to_string(floure_core::config::data_dir().join("recent-errors.log"))
        .map(|text| {
            text.lines()
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
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

    fn model(id: &str, downloaded: bool) -> ModelStatus {
        ModelStatus {
            name: id.to_string(),
            id: id.to_string(),
            downloaded,
            path: String::new(),
            size_bytes: 0,
            url: String::new(),
            backend: String::new(),
            recommended: false,
        }
    }

    #[test]
    fn onboarding_requires_a_downloaded_speech_model() {
        assert!(!speech_model_ready(&[]));
        assert!(!speech_model_ready(&[model(
            "parakeet-tdt-0.6b-v2-int8",
            false
        )]));
        assert!(speech_model_ready(&[model(
            "parakeet-tdt-0.6b-v2-int8",
            true
        )]));
        assert!(speech_model_ready(&[model("whisper-base-q5_1", true)]));
        // A downloaded LLM or VAD is not enough to dictate.
        assert!(!speech_model_ready(&[model("s1-mini-q4_k_m", true)]));

        assert!(can_advance(0, false), "mic step never requires a model");
        assert!(!can_advance(1, false));
        assert!(can_advance(1, true));
        assert!(can_advance(2, false));
    }

    #[test]
    fn assembled_sentence_collapses_raw_vad_lines() {
        let mut lines = vec![
            Utterance {
                id: 1,
                raw: "So here is the testing.".into(),
                cleaned: String::new(),
                streaming: false,
                finalized: false,
            },
            Utterance {
                id: 2,
                raw: "of the application I'm running.".into(),
                cleaned: String::new(),
                streaming: false,
                finalized: false,
            },
        ];
        merge_finalized(
            &mut lines,
            2,
            "So here is the testing of the application I'm running.".into(),
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].text(),
            "So here is the testing of the application I'm running."
        );
        assert!(lines[0].finalized);
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
