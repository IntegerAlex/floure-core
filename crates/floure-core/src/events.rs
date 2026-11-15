//! Events emitted by the engine and consumed by every UI surface.
//!
//! A broadcast channel fans these out to the main window, widget, tray and
//! loopback status endpoint. `Engine::state()` remains the synchronous snapshot
//! for callers that cannot wait on an event.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EngineState {
    Idle = 0,
    Listening = 1,
    Stopping = 2,
    Transcribing = 3,
    Rewriting = 4,
    Error = 5,
    NoModel = 6,
    Warming = 7,
    Stopped = 8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    State(EngineState),
    MicLevel(f32),
    /// Provisional text for the sentence currently being spoken. Display
    /// only: never typed, saved, or sent to cleanup.
    Partial {
        text: String,
    },
    Final {
        utterance_id: u64,
        text: String,
        latency_ms: u64,
        duration_sec: f32,
    },
    CleanupStarted {
        utterance_id: u64,
    },
    CleanupToken {
        utterance_id: u64,
        token: String,
    },
    CleanupDone {
        utterance_id: u64,
        text: String,
    },
    CleanupFailed {
        utterance_id: u64,
        message: String,
    },
    OutputFailed {
        utterance_id: u64,
        kind: OutputKind,
        message: String,
    },
    Committed {
        utterance_id: u64,
        history_id: i64,
    },
    AsrReady {
        backend: String,
    },
    ModelProgress {
        id: String,
        percent: u8,
        bytes: u64,
        rate: u64,
        eta_secs: u32,
    },
    ModelDone {
        id: String,
    },
    ModelFailed {
        id: String,
        message: String,
    },
    Error {
        kind: String,
        message: String,
        recoverable: bool,
    },
    Stopped {
        flushed: bool,
    },
    Cancelled {
        utterance_id: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Typing,
    Clipboard,
}
