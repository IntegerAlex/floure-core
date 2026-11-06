//! Floure engine: audio capture, VAD, ASR, cleanup, output, and local data.
//!
//! This crate is UI-agnostic. The GPUI application owns windows and input;
//! `floure-core` owns the pipeline and the local repositories.

pub mod audio;
pub mod compute;
pub mod config;
pub mod diagnostics;
pub mod events;
pub mod history;
pub mod llm;
pub mod models;
pub mod output;
pub mod parakeet;
pub mod vad;
pub mod whisper;

pub use events::{EngineEvent, EngineState};

/// The application version compiled into the binary.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_workspace_package() {
        assert_eq!(super::version(), "0.3.0");
    }
}
