# Product

<!-- impeccable:product-schema 1 -->

## Platform

desktop — native Linux-first application built with GPUI; Windows and macOS code paths compile, packaging follows after Linux.

## Stack

Rust workspace. GPUI (`gpui-pre` 0.3.11) for rendering, `gpui-component` 0.9 for controls, `ksni` for the Linux tray, and `global-hotkey` for X11/Windows shortcuts. The Tauri/React application is removed after parity.

## Users

People who dictate into other applications all day: developers, writers, support staff, and anyone who prefers speech to typing. They care that audio stays local, that the app is always one shortcut away, and that it never steals focus from the window they are writing into. The first release targets Linux users on Arch, Hyprland, GNOME, and KDE; Windows and macOS follow.

## Product Purpose

Floure turns speech into clean text locally: microphone → VAD → Parakeet or Whisper → optional local or cloud LLM cleanup → typing into the focused window or clipboard. Success is first dictation within minutes, sub-second transcription for short utterances, and no account or cloud dependency on the default path.

## Positioning

A local-first dictation pipeline that also fixes what the recogniser hears: the dictionary feeds decode bias, not just find-and-replace, and a local LLM cleans the transcript before it reaches the focused app. The competitor can copy the hotkey; it cannot truthfully claim the same local dictionary loop and on-device cleanup without the same pipeline.

## Operating Context

The app runs in the background with a tray icon and an optional always-on-top widget. A global shortcut starts and stops recording; on Wayland the loopback control server (`127.0.0.1:17833`) and CLI toggle remain the reliable triggers. Models are downloaded on demand into the user's data directory. History is a local SQLite database with full-text search. The target window already has focus; Floure types or pastes into it and must not move focus.

## Capabilities and Constraints

- Capture: cpal, any sample format, device fallback when a saved device disappears.
- Recognition: sherpa-onnx Parakeet TDT 0.6B v2 (int8), Whisper large-v3-turbo, Whisper base; Silero VAD.
- Cleanup: local llama.cpp models (S1-Mini, Gemma 3 1B) or OpenRouter; cleanup can be off.
- Output: type into the focused window and/or copy to clipboard; trailing-space and auto-submit options.
- Data: history with FTS5, favorites, date filtering, export; dictionary with categories and favorites; insights from real history.
- Models: on-demand downloads with progress, resume, verification, and deletion.
- Controls: configurable global shortcut, hold/toggle activation, audio feedback, permissions, diagnostics.
- Constraints: GPL-3.0-or-later (relicensed for Apache-2.0 GPUI dependencies); GPU rendering required by GPUI (Vulkan/OpenGL); no reliable system-wide hotkey grab on Wayland; GNOME Wayland is clipboard-only unless `ydotool` is installed; local models occupy 200 MB–800 MB.
- Out of scope for this redesign: light theme, i18n, cloud accounts, mobile, Windows/macOS packaging in the first GPUI release.

## Brand Commitments

The name Floure, the local-first privacy claim, the GPL-3.0-or-later license, and the new pinned visual direction: minimal, monochrome, near-black, Apple-like restraint. One red accent reserved for recording. No gradients, glass, ornamental cards, or icon-tile decoration.

## Evidence on Hand

- `README.md` documents the pipeline, models, configuration paths, Wayland loopback, dictionary behaviour, and diagnostics.
- `CONTEXT.md` documents the stack, key paths, and build commands.
- `crates/floure-core/` contains the audio, VAD, ASR, LLM, output, pipeline, history, model, config, and diagnostics logic. `crates/floure-app/` contains the GPUI shell.
- `application/src/` and `packaging/arch/` are the current UI and packaging evidence.
- There is no real usage dataset in the repo; stats must render from the user's local history, never invented figures.

## Product Principles

1. One job: press, speak, get clean text where the cursor is.
2. The microphone is the interface; everything else is secondary.
3. Local by default, cloud only when explicitly chosen.
4. Never steal focus, never lose a transcript, never hide an error.
5. Remove controls before adding them.

## Accessibility & Inclusion

Keyboard-first operation, visible focus, screen-reader names and states through AccessKit, reduced-motion support, contrast of at least 4.5:1 for body text, and no interaction that requires a precise pointer.
