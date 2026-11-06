# ADR-0005: GPUI platform stack

**Status:** accepted
**Date:** 2026-11-06

## Context

The existing desktop UI is Tauri v2 + React. The redesign replaces it with a native Rust GPUI application. GPUI is pre-1.0 and split across several crates; the platform crate is required to select a windowing backend. The app must run on Wayland and X11, keep the existing audio/ASR/LLM engine, and preserve local data paths.

## Decision

- `gpui-pre = 0.3.11` (lib name `gpui`) for the framework.
- `gpui-pre-platform = 0.3.11` with `wayland` and `x11` features for the windowing backend.
- `gpui-pre-wgpu = 0.3.11` for the GPU renderer.
- `gpui-component = 0.9.0`, `gpui-base = 0.9.0`, `gpui-kit-assets = 0.9.0`, pinned exactly; commit the workspace `Cargo.lock`.
- `ksni` for the Linux tray, `global-hotkey` for X11/Windows shortcuts. Wayland keeps the loopback control server and CLI.
- Rust toolchain is pinned with `mise.toml` to 1.99.1 because the system toolchain is not guaranteed to be present.

## Spike results

- `cargo check`, `cargo clippy --workspace -- -D warnings`, and `cargo test --workspace` pass.
- `floure --version` exits before any GPU/window initialization.
- The first window opens on the current Wayland session and stays alive for six seconds (timeout 124), with client-side window decorations, no system titlebar, and `inactive_frame_interval: None`.
- Frameless drag, resize, double-click maximize, layer-shell, AccessKit/AT-SPI, and `QuitMode::Explicit` lifecycle are API-available but are acceptance items in Task 4, not claimed as verified here.
- GPU failure must be probed before `Application::run`; without a GPU there is no window in which to show an error.

## Consequences

- The React/Tauri app is removed after parity.
- The app requires working Vulkan/OpenGL drivers; software Vulkan (lavapipe) is accepted for startup tests but is slow.
- GPUI API churn is contained by exact pins and a committed lockfile.
- `gpui-component`'s `TitleBar`/`WindowBorder` are not used because they paint a gradient and a shadow; the shell draws its own chrome.
