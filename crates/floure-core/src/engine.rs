//! Engine lifecycle and the public API consumed by the GPUI app.

use std::path::PathBuf;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use anyhow::{Result, bail};
use async_broadcast::Receiver;

use crate::config::{AppConfig, SettingsUpdate};
use crate::events::{EngineEvent, EngineState};
use crate::sink::EventSink;

pub struct Paths {
    pub data_dir: PathBuf,
    pub history_db: PathBuf,
    pub model_dir: PathBuf,
}

impl Paths {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            data_dir: crate::config::data_dir(),
            history_db: crate::config::history_db_path(),
            model_dir: config.model_dir.clone(),
        }
    }
}

/// One engine per process. `llama.cpp`'s backend is process-global, so a
/// second construction would abort rather than fail cleanly.
static INSTANCE: OnceLock<()> = OnceLock::new();

/// Bumped by start/stop mic-test calls. Each preview thread exits when its own
/// generation is no longer current, so a quick start-stop-start cannot leave
/// two capture streams fighting over the device.
static MIC_TEST_GEN: AtomicU64 = AtomicU64::new(0);

pub struct Engine {
    sink: EventSink,
    config: Mutex<AppConfig>,
    started: Arc<AtomicBool>,
}

impl Engine {
    pub fn new(config: AppConfig, paths: Paths) -> Result<Self> {
        if INSTANCE.set(()).is_err() {
            bail!("floure engine already created in this process");
        }
        let mut config = config;
        config.model_dir = paths.model_dir;
        let (sink, _initial) = EventSink::new();
        Ok(Self {
            sink,
            config: Mutex::new(config),
            started: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn subscribe(&self) -> Receiver<EngineEvent> {
        self.sink.subscribe()
    }

    pub fn state(&self) -> EngineState {
        self.sink.state()
    }

    pub fn start(&self) -> Result<()> {
        if self.started.swap(true, Ordering::SeqCst) {
            bail!("recording already running");
        }
        let config = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        self.sink.send(EngineEvent::State(EngineState::Listening));
        if let Err(e) = crate::pipeline::start_pipeline(self.sink.app_handle(), config) {
            self.started.store(false, Ordering::SeqCst);
            let message = e.to_string();
            self.sink.send(EngineEvent::Error {
                kind: "audio".into(),
                message: message.clone(),
                recoverable: true,
            });
            self.sink.send(EngineEvent::State(EngineState::Error));
            return Err(anyhow::anyhow!(message));
        }
        Ok(())
    }

    /// Signal stop and return. The worker flushes the tail, joins, and emits
    /// `Stopped`; the GPUI foreground thread never blocks on ASR or llama.cpp.
    pub fn stop(&self) {
        if !self.started.swap(false, Ordering::SeqCst) {
            return;
        }
        self.sink.send(EngineEvent::State(EngineState::Stopping));
        let sink = self.sink.clone();
        std::thread::spawn(move || {
            crate::pipeline::stop_pipeline();
            sink.send(EngineEvent::State(EngineState::Idle));
            sink.send(EngineEvent::Stopped { flushed: true });
        });
    }

    /// Blocking stop for process exit and tests.
    pub fn stop_blocking(&self) {
        self.started.store(false, Ordering::SeqCst);
        crate::pipeline::stop_pipeline();
        self.sink.send(EngineEvent::State(EngineState::Idle));
        self.sink.send(EngineEvent::Stopped { flushed: true });
    }

    pub fn cancel(&self) {
        crate::pipeline::cancel_pipeline();
        self.started.store(false, Ordering::SeqCst);
        self.sink.send(EngineEvent::Cancelled {
            utterance_id: self.sink.app_handle().utterance_id(),
        });
        self.sink.send(EngineEvent::State(EngineState::Idle));
    }

    pub fn apply_settings(&self, update: SettingsUpdate) {
        let mut config = self.config.lock().unwrap_or_else(|e| e.into_inner());
        config.apply_update(update);
    }

    pub fn is_warming(&self) -> bool {
        crate::pipeline::is_warming()
    }

    pub fn is_ready(&self) -> bool {
        crate::pipeline::is_ready()
    }

    /// Start a model download in the background. Progress and the terminal
    /// state travel only through `EngineEvent`; the call returns immediately.
    pub fn download_model(&self, id: &str) -> Result<()> {
        let model_dir = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .model_dir
            .clone();
        if crate::models::is_downloading(&model_dir.join(id)) {
            return Ok(());
        }
        let manager = crate::models::ModelManager::new(model_dir);
        let sink = self.sink.clone();
        let id = id.to_string();
        std::thread::spawn(move || {
            let progress_sink = sink.clone();
            let progress_id = id.clone();
            let result = manager.download(&id, move |percent, bytes| {
                progress_sink.send(EngineEvent::ModelProgress {
                    id: progress_id.clone(),
                    percent: percent.min(100) as u8,
                    bytes,
                    rate: 0,
                    eta_secs: 0,
                });
            });
            match result {
                Ok(()) => sink.send(EngineEvent::ModelDone { id }),
                Err(error) => sink.send(EngineEvent::ModelFailed {
                    id,
                    message: error.to_string(),
                }),
            }
        });
        Ok(())
    }

    /// Preview capture for onboarding and the microphone test: emits
    /// `MicLevel` only — no transcription, no typing, no history.
    pub fn start_mic_test(&self) -> Result<()> {
        self.stop_mic_test();
        let generation = MIC_TEST_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        let mic_hint = self
            .config
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .selected_mic_id
            .clone();
        let sink = self.sink.clone();
        let error_sink = sink.clone();
        std::thread::spawn(move || {
            let level_sink = sink.clone();
            let capture = crate::audio::start_capture(
                mic_hint.as_deref(),
                move |samples| {
                    let n = samples.len().max(1);
                    let level = (samples.iter().map(|s| s * s).sum::<f32>() / n as f32)
                        .sqrt()
                        .clamp(0.0, 1.0);
                    level_sink.send(EngineEvent::MicLevel(level));
                },
                move |message| {
                    error_sink.send(EngineEvent::Error {
                        kind: "microphone".into(),
                        message,
                        recoverable: true,
                    });
                },
            );
            match capture {
                Ok(_capture) => {
                    while MIC_TEST_GEN.load(Ordering::SeqCst) == generation {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
                Err(error) => sink.send(EngineEvent::Error {
                    kind: "microphone".into(),
                    message: error.to_string(),
                    recoverable: true,
                }),
            }
        });
        Ok(())
    }

    pub fn stop_mic_test(&self) {
        MIC_TEST_GEN.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        // The worker owns the audio stream and the llama backend; leaving it
        // running past the engine would keep the process alive.
        if self.started.load(Ordering::SeqCst) {
            self.stop_blocking();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    /// One test, not several: the engine is a process singleton and the test
    /// binary runs its tests in parallel threads sharing that static.
    #[test]
    fn engine_contract() {
        let config = AppConfig::load();
        let paths = Paths::from_config(&config);
        let engine = Engine::new(config, paths).expect("first engine");

        assert_eq!(engine.state(), EngineState::Idle);

        let mut a = engine.subscribe();
        let mut b = engine.subscribe();
        engine.sink.send(EngineEvent::MicLevel(0.25));
        assert_eq!(
            a.try_recv().expect("first subscriber"),
            b.try_recv().expect("second subscriber")
        );

        engine.sink.send(EngineEvent::State(EngineState::Listening));
        assert_eq!(engine.state(), EngineState::Listening);

        let config = AppConfig::load();
        let paths = Paths::from_config(&config);
        assert!(
            Engine::new(config, paths).is_err(),
            "a second engine in one process must fail cleanly, not abort"
        );
    }
}
