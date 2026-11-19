# Floure — Project Context

## What it is

Floure is a Linux-first desktop dictation app: speak into the mic → speech is
transcribed locally → deterministic punctuation/casing repair → an optional
LLM polish pass → text is typed into the focused window or copied to the
clipboard. No cloud account is required for the local path.

## Stack

- **UI:** Rust + GPUI (`gpui-pre` 0.3.11, `gpui-component` 0.9) in
  `crates/floure-app`. Native GPU window, no webview.
- **Engine:** Rust in `crates/floure-core`, with no UI dependency.
  - Audio capture: `cpal` (`audio.rs`).
  - VAD: Silero (`vad.rs`).
  - ASR: Parakeet + Whisper via `sherpa-onnx` (`parakeet.rs`, `whisper.rs`, `models.rs`).
  - Deterministic repair: sentence assembly across VAD segments, sentence
    breaks, question marks, spacing, casing, terminal full stops, money/time/
    percent/unit ITN (`text.rs`).
  - Output replacements from the dictionary and opt-in spoken punctuation.
  - Live partial text is display-only; the ASR cache unloads after 3 idle
    minutes.
  - Optional Polish: local models via `llama.cpp`, or OpenRouter (`llm.rs`).
  - Output: typing + clipboard (`output.rs`); loop in `pipeline.rs`.
  - Lifecycle and events: `engine.rs`, `events.rs`, `sink.rs`.
  - Data: `history.rs` (FTS5), `dictionary.rs`, `insights.rs`, `config.rs`,
    `migrate.rs`, `diagnostics.rs`.
- **Integration:** `ksni` tray, `global-hotkey` on X11/Windows/macOS, evdev
  `/dev/input` listener on Linux Wayland (`input` group, opt-in), a status-only
  layer-shell widget, and the loopback server and CLI in
  `crates/floure-app/src/control_server.rs`.

## Key paths

- UI: `crates/floure-app/src` · engine: `crates/floure-core/src`
- Packaging: `packaging/` (desktop entry, metainfo, icons, PKGBUILD, portable script)
- History DB: `~/.local/share/floure/history.db` (`STT_DATA_DIR` overrides it;
  legacy `~/.local/share/stt/history.db` is migrated forward) — see
  `docs/adr/0002-history-db-path.md`.
- Config: `~/.config/floure/config.json`.
- Settings bridge: `~/.config/floure/ui-state.json`, written once by the old
  Tauri app and retired by `floure_core::migrate`.
- Decisions: `docs/adr/` (0005 GPUI platform, 0006 license, 0007 packaging,
  0008 Wayland input and widget).

## Build / check / test

```bash
mise install                                  # Rust 1.99.1
cargo build -p floure-app                     # target/debug/floure
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
./target/debug/floure --version
./target/debug/floure --diagnostics
```

## Legacy

`application/` still contains the Tauri + React app. It is kept only as the
source of the one-time settings bridge (hotkey, PTT mode, sound, onboarding)
and is not built, packaged, or shipped by the GPUI line. Delete it after that
bridge release has been cut and the manual migration matrix has passed.

## Glossary

- **Profile:** ASR model preset (Parakeet / Whisper Turbo / Whisper Base).
- **Repair:** deterministic, non-generative punctuation/casing/spacing pass.
- **Polish:** optional LLM pass over the repaired text.
- **Pipeline:** mic → VAD → ASR → cleanup → output loop (`pipeline.rs`).
- **Engine state:** `Idle / Listening / Stopping / Transcribing / Rewriting /
  Error / NoModel / Warming / Stopped`, fanned out as `EngineEvent`.
