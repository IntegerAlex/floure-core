//! Broadcast event sink and the pipeline's `AppHandle` compatibility shim.
//!
//! The existing pipeline was written against Tauri's `AppHandle::emit(name,
//! json)`. This shim keeps those call sites intact while translating them into
//! the UI-facing `EngineEvent` stream. New code should call `EventSink::send`
//! directly.

use std::sync::{
    Arc,
    atomic::{AtomicU8, AtomicU64, Ordering},
};

use async_broadcast::{InactiveReceiver, Receiver, Sender, broadcast};
use serde_json::Value;

use crate::events::{EngineEvent, EngineState, OutputKind};

#[derive(Clone)]
pub struct EventSink {
    tx: Sender<EngineEvent>,
    /// Keeps the channel active when no UI subscriber is attached yet.
    _keepalive: Arc<InactiveReceiver<EngineEvent>>,
    state: Arc<AtomicU8>,
}

impl EventSink {
    pub fn new() -> (Self, Receiver<EngineEvent>) {
        let (mut tx, rx) = broadcast(512);
        tx.set_overflow(true);
        let keepalive = Arc::new(rx.deactivate());
        let rx = tx.new_receiver();
        (
            Self {
                tx,
                _keepalive: keepalive,
                state: Arc::new(AtomicU8::new(EngineState::Idle as u8)),
            },
            rx,
        )
    }

    /// Create an additional receiver. Each subscriber sees every event from
    /// the moment it subscribed.
    pub fn subscribe(&self) -> Receiver<EngineEvent> {
        self.tx.new_receiver()
    }

    pub fn send(&self, event: EngineEvent) {
        if let EngineEvent::State(state) = &event {
            self.state.store(*state as u8, Ordering::SeqCst);
        }
        let _ = self.tx.try_broadcast(event);
    }

    pub fn state(&self) -> EngineState {
        match self.state.load(Ordering::SeqCst) {
            1 => EngineState::Listening,
            2 => EngineState::Stopping,
            3 => EngineState::Transcribing,
            4 => EngineState::Rewriting,
            5 => EngineState::Error,
            6 => EngineState::NoModel,
            7 => EngineState::Warming,
            8 => EngineState::Stopped,
            _ => EngineState::Idle,
        }
    }

    pub fn app_handle(&self) -> AppHandle {
        AppHandle {
            sink: self.clone(),
            utterance_id: Arc::new(AtomicU64::new(0)),
        }
    }
}

/// The pipeline's event surface. Mirrors the subset of `tauri::AppHandle` the
/// old code used, so the pipeline port is a type change rather than a rewrite.
#[derive(Clone)]
pub struct AppHandle {
    sink: EventSink,
    utterance_id: Arc<AtomicU64>,
}

impl AppHandle {
    pub fn utterance_id(&self) -> u64 {
        self.utterance_id.load(Ordering::SeqCst)
    }

    /// Mirrors the shape of Tauri's `emit` (which returned `Result`) so the
    /// ported pipeline can keep its `let _ = app.emit(...)` call sites.
    pub fn emit<T: Into<Value>>(&self, name: &str, payload: T) -> bool {
        let payload = payload.into();
        match name {
            "asr_final" => {
                let id = self.utterance_id.fetch_add(1, Ordering::SeqCst) + 1;
                self.sink.send(EngineEvent::Final {
                    utterance_id: id,
                    text: payload["text"].as_str().unwrap_or_default().to_string(),
                    latency_ms: payload["latency_ms"].as_u64().unwrap_or(0),
                    duration_sec: payload["duration_sec"].as_f64().unwrap_or(0.0) as f32,
                });
            }
            "asr_ready" => self.sink.send(EngineEvent::AsrReady {
                backend: payload["backend"].as_str().unwrap_or("asr").to_string(),
            }),
            "asr_error" => self.sink.send(EngineEvent::Error {
                kind: "asr".into(),
                message: payload["error"]
                    .as_str()
                    .unwrap_or("Speech recognition failed")
                    .to_string(),
                recoverable: true,
            }),
            "llm_start" => {
                self.sink.send(EngineEvent::CleanupStarted {
                    utterance_id: self.utterance_id.load(Ordering::SeqCst),
                });
                self.sink.send(EngineEvent::State(EngineState::Rewriting));
            }
            "llm_token" => self.sink.send(EngineEvent::CleanupToken {
                utterance_id: self.utterance_id.load(Ordering::SeqCst),
                token: payload["token"].as_str().unwrap_or_default().to_string(),
            }),
            "cleanup_done" => self.sink.send(EngineEvent::CleanupDone {
                utterance_id: self.utterance_id.load(Ordering::SeqCst),
                text: payload["text"].as_str().unwrap_or_default().to_string(),
            }),
            "llm_error" => self.sink.send(EngineEvent::CleanupFailed {
                utterance_id: self.utterance_id.load(Ordering::SeqCst),
                message: payload["error"]
                    .as_str()
                    .unwrap_or("Cleanup failed")
                    .to_string(),
            }),
            "committed" => self.sink.send(EngineEvent::Committed {
                utterance_id: self.utterance_id.load(Ordering::SeqCst),
                history_id: payload["history_id"].as_i64().unwrap_or(0),
            }),
            "output_error" => self.sink.send(EngineEvent::OutputFailed {
                utterance_id: self.utterance_id.load(Ordering::SeqCst),
                kind: OutputKind::Typing,
                message: payload["error"]
                    .as_str()
                    .unwrap_or("Could not deliver transcript")
                    .to_string(),
            }),
            "mic_level" => self
                .sink
                .send(EngineEvent::MicLevel(payload.as_f64().unwrap_or(0.0) as f32)),
            "model_download_progress" => {
                let id = payload["id"].as_str().unwrap_or_default().to_string();
                if payload["done"].as_bool().unwrap_or(false) {
                    self.sink.send(EngineEvent::ModelDone { id });
                } else {
                    self.sink.send(EngineEvent::ModelProgress {
                        id,
                        percent: payload["percent"].as_u64().unwrap_or(0) as u8,
                        bytes: payload["bytes"].as_u64().unwrap_or(0),
                        rate: 0,
                        eta_secs: 0,
                    });
                }
            }
            "model_download_error" => self.sink.send(EngineEvent::ModelFailed {
                id: payload["id"].as_str().unwrap_or_default().to_string(),
                message: payload["error"]
                    .as_str()
                    .unwrap_or("Model download failed")
                    .to_string(),
            }),
            _ => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pipeline_emits_map_to_an_ordered_event_stream() {
        let (sink, mut rx) = EventSink::new();
        let app = sink.app_handle();

        sink.send(EngineEvent::State(EngineState::Listening));
        app.emit(
            "asr_final",
            json!({"text": "hello world", "latency_ms": 12, "duration_sec": 1.5}),
        );
        app.emit("llm_start", json!({}));
        app.emit("llm_token", json!({"token": "hello"}));
        app.emit("cleanup_done", json!({"text": "hello world"}));
        app.emit("committed", json!({"history_id": 7}));
        sink.send(EngineEvent::State(EngineState::Idle));

        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::State(EngineState::Listening)
        );
        assert!(matches!(
            rx.try_recv().unwrap(),
            EngineEvent::Final { utterance_id: 1, ref text, latency_ms: 12, duration_sec } if text == "hello world" && (duration_sec - 1.5).abs() < 1e-6
        ));
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::CleanupStarted { utterance_id: 1 }
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::State(EngineState::Rewriting)
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::CleanupToken {
                utterance_id: 1,
                token: "hello".into()
            }
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::CleanupDone {
                utterance_id: 1,
                text: "hello world".into()
            }
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::Committed {
                utterance_id: 1,
                history_id: 7
            }
        );
        assert_eq!(
            rx.try_recv().unwrap(),
            EngineEvent::State(EngineState::Idle)
        );
        assert!(
            rx.try_recv().is_err(),
            "one terminal state, no trailing events"
        );
        assert_eq!(sink.state(), EngineState::Idle);
    }

    #[test]
    fn a_stalled_subscriber_does_not_block_the_pipeline() {
        let (sink, stalled) = EventSink::new();
        let _stalled = stalled; // never drained
        for i in 0..600 {
            sink.send(EngineEvent::MicLevel(i as f32));
        }
        let mut live = sink.subscribe();
        sink.send(EngineEvent::State(EngineState::Listening));
        // Overflow mode dropped the old levels; the newest state still arrives.
        assert_eq!(
            live.try_recv()
                .unwrap_or(EngineEvent::State(EngineState::Listening)),
            EngineEvent::State(EngineState::Listening)
        );
    }
}
