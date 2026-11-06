# GPUI Dark Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the Tauri/React app with a native GPUI application and ship the minimal monochrome Dictate / History / Stats / Configuration UI.

**Architecture:** A Rust workspace with `floure-core` (engine + repositories, no UI) and `floure-app` (GPUI views, tray, hotkeys, loopback, CLI). The engine emits a broadcast `EngineEvent` stream; GPUI consumes it on the foreground executor. Tauri and React are deleted after parity.

**Tech Stack:** Rust 2024, `gpui-pre =0.3.11`, `gpui-pre-platform =0.3.11` (wayland+x11), `gpui-pre-wgpu =0.3.11`, `gpui-component =0.9.0`, `gpui-base =0.9.0`, `gpui-kit-assets =0.9.0`, `ksni`, `global-hotkey`, `cargo-deb`, `cargo-generate-rpm`.

**Spec:** `docs/superpowers/specs/2026-11-06-gpui-dark-redesign-design.md`
**Product:** `PRODUCT.md`

## Global Constraints

- License: `GPL-3.0-or-later`.
- Exact pins: `gpui-pre`, `gpui-pre-platform`, `gpui-pre-wgpu` all `=0.3.11`; `gpui-component`, `gpui-base`, `gpui-kit-assets` all `=0.9.0`. Commit the root `Cargo.lock`.
- `gpui-pre-platform` must enable `wayland` and `x11`; without them the app runs headless and `open_window` fails.
- Data paths are literal and unchanged: `~/.config/floure/config.json`, `~/.local/share/floure/history.db`, `~/.local/share/floure/models/`. `STT_DATA_DIR` still overrides the data dir.
- No webview, no Node, no pnpm, no Tauri after Task 12.
- No fake stats, no invented usage data; render from the local DB.
- No gradients, glass, shadows at rest, icon tiles, or a second accent. One red (`#FF453A`) for recording.
- The widget never steals focus; on Wayland it is status-only.
- `sherpa-onnx` uses the default `static` feature, not `shared`.
- Every task ends with `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, and the tests it names.

## Review Focus

- **Focus theft:** showing the main window, widget, or tray menu must not move focus away from the target app; paste/typing must still land in the original window.
- **UI-thread blocking:** stop, cancel, model downloads, and llama/sherpa calls must never join or block the GPUI foreground executor.
- **Data loss:** hotkey/PTT/sound/onboarding values live only in WebKit localStorage; the migration must preserve them, and FTS/duration migrations must be idempotent.
- **Packaging start failure:** static sherpa/llama, real runtime deps, desktop/icon/metainfo assets, and the binary named `floure` must be asserted in CI before a release.
- **Wayland dead ends:** every hotkey/widget claim needs a visible fallback (`floure --toggle`, tray, clipboard) when the compositor cannot provide it.

---

### Task 1: Workspace scaffold and GPUI spike

**Files:**
- Create: `Cargo.toml`, `rust-toolchain.toml`, `crates/floure-core/Cargo.toml`, `crates/floure-core/src/lib.rs`, `crates/floure-app/Cargo.toml`, `crates/floure-app/src/main.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `floure_core::version() -> &'static str`; `floure` binary opens one 960×700 window.
- Consumes: exact GPUI pins listed above.

- [ ] Create the workspace and both crates; add `[[bin]] name = "floure"` and `rust-toolchain.toml` with `rustfmt` and `clippy`.
- [ ] Add a `--version` path that exits before `Application::new`, and a minimal GPUI window with a black background, 44 px title strip, and no decorations.
- [ ] Spike and record in `docs/adr/0005-gpui-platform.md`: frameless drag, resize, double-click maximize, layer-shell availability, AccessKit tree, `QuitMode::Explicit`, and `inactive_frame_interval: None`.
- [ ] Run `cargo check -p floure-app`, `cargo test -p floure-core`, and `timeout 10 ./target/debug/floure || test $? -eq 124` under a display.
- [ ] Commit `feat(gpui): scaffold native workspace`.

### Task 2: Extract the engine crate

**Files:**
- Move: `application/src-tauri/src/{audio,vad,parakeet,whisper,llm,output,models,config,compute,diagnostics}.rs` → `crates/floure-core/src/`
- Create: `crates/floure-core/src/{engine,events,paths,history,dictionary,insights,system_deps,models_api}.rs`
- Modify: `crates/floure-core/Cargo.toml`

**Interfaces:**
- Produces: `Engine::new(config, paths)`, `Engine::start()`, `Engine::stop()`, `Engine::stop_blocking()`, `Engine::cancel()`, `Engine::subscribe() -> async_broadcast::Receiver<EngineEvent>`, `Engine::state() -> EngineState`, `Engine::apply_settings(SettingsUpdate)`, `Engine::download_model(id)`, `Engine::start_mic_test()/stop_mic_test()`.
- Produces: typed `History`, `Dictionary`, `Insights` repositories and `EngineEvent` variants from the spec.

- [ ] Move the clean modules first; replace every `tauri::` use with a core type or delete it. Keep `audio`/`vad`/`parakeet`/`whisper`/`llm`/`output`/`models`/`config`/`compute` free of UI imports.
- [ ] Split `lib.rs` into typed repositories (`history`, `dictionary`, `insights`, `system_deps`, `models_api`) with no behaviour change; replace `serde_json::Value` returns with typed structs.
- [ ] Implement `EngineEvent`, a process singleton guard, poisoned-lock recovery, `apply_settings`, and the async stop/cancel contract. `stop()` only signals; the worker emits `Stopped` after flush+join. `cancel()` sets a flag checked before typing/clipboard/history and emits `Cancelled`.
- [ ] Add `Engine::subscribe` as a cloneable `async_broadcast` fan-out and `Engine::state` as an atomic snapshot.
- [ ] Port the surviving Rust tests; delete the IPC wire-shape tests and the tests that re-implement the algorithm in the test body.
- [ ] Add tests: event sequence has one terminal state; cancel produces no output/history; two subscribers both receive; second `Engine::new` fails cleanly; dropping an Engine joins the worker.
- [ ] Add a `trait Recognizer`/`trait Vad` seam and use it in one state-machine test without loading 500 MB models.
- [ ] Run `cargo test -p floure-core` and commit `refactor(core): extract engine from tauri`.

### Task 3: Data migrations and webview-settings bridge

**Files:**
- Modify: `crates/floure-core/src/history.rs`, `dictionary.rs`, `config.rs`, `output.rs`
- Create: `crates/floure-core/src/migrate.rs`
- Test: `crates/floure-core/src/migrate.rs` tests

**Interfaces:**
- Produces: idempotent `migrate(db, config) -> Result<MigrationReport>`; `transcripts_fts` external-content table with triggers; `save_to_history(..., duration_sec, language)`.
- Consumes: existing `history.db`, `config.json`, and legacy `stt` directory.

- [ ] Write failing tests for: FTS table creation/backfill/search; re-running migration does nothing; duration/language persisted; legacy `~/.local/share/stt` DB moved forward; hotkey/PTT/sound/onboarding values read from `ui-state.json` then retired.
- [ ] Implement `transcripts_fts` with insert/update/delete triggers and a backfill keyed by rowid; make the migration idempotent.
- [ ] Pass the real VAD segment duration (`segment.len()/16000`) and selected profile into `save_to_history`; remove the hardcoded `en`/`0.0`.
- [ ] Implement the UI-state bridge: read `ui-state.json` if present, merge into `config.json` (`hotkey`, `ptt_mode`, `sound_enabled`, `sound_volume`, `onboarding_completed`), then rename it `.migrated`.
- [ ] Add path tests asserting the exact three data paths and `STT_DATA_DIR` override.
- [ ] Run `cargo test -p floure-core` and commit `feat(core): migrate history, stats, and webview settings`.

### Task 4: GPUI shell and tokens

**Files:**
- Create: `crates/floure-app/src/{app,theme,shell,rail,titlebar}.rs`
- Create: `crates/floure-app/src/assets/` (Inter font, Lucide SVGs, logo)
- Test: `crates/floure-app/src/theme.rs` tests

**Interfaces:**
- Produces: `FloureApp` entity with `Destination`, `EngineHandle`, `Settings`, `ToastQueue`, `OnboardingState`; `theme::tokens()`.

- [ ] Add the token module with the exact black/surface/text/border/accent values, type scale, radii, spacing, focus ring, and reduce-motion gate. Test contrast constants (`#8E8E93` on `#000` ≥ 4.5).
- [ ] Build the frameless shell: 64 px rail, 44 px title bar, content gutter, window controls, drag/maximize, `Ctrl+1–4` destination switching.
- [ ] Wire `QuitMode::Explicit`: close destroys the main window; recreate from the app entity; quit when no tray host exists; flush recording before exit.
- [ ] Add the reduce-motion preference from `gsettings`/`kreadconfig6` plus an explicit Configuration override.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): add gpui shell and dark tokens`.

### Task 5: Dictate, hotkeys, tray, loopback

**Files:**
- Create: `crates/floure-app/src/views/dictate.rs`, `controls.rs`, `tray.rs`, `hotkeys.rs`, `control_server.rs`
- Modify: `crates/floure-app/src/main.rs`, `app.rs`

**Interfaces:**
- Consumes: `Engine::start/stop/cancel/subscribe/state`.
- Produces: status states `Idle | Listening | Stopping | Transcribing | Rewriting | Error | NoModel | Warming | Stopped`; `toggle()` shared by hotkey, tray, loopback, and widget.

- [ ] Build Dictate: 112 px mic, status line, shortcut pill, waveform (30 Hz coalescing), transcript panel, streaming cleanup tokens, Copy/Cancel/Clear, session strip, model badge, inline error row.
- [ ] Add `NoModel`, `Warming`, `Stopped`, and `Nothing heard` states; `Download a model to start` opens Configuration > Models.
- [ ] Implement `global-hotkey` only when `WAYLAND_DISPLAY` is unset; add hold/toggle modes and the press/release grace window.
- [ ] Implement the loopback server (`/status`, `/toggle`, `/start`, `/stop`, `/show`) and `--toggle`/`--status` early-exit CLI that POSTs and exits before GPUI starts.
- [ ] Implement `ksni` tray (`async-io`, no default tokio): Open, Toggle recording, Configuration, Show widget, Quit; no-tray mode still runs.
- [ ] Add tests: `--toggle` does not start a second app; `/status` reflects `Engine::state` without a subscriber; a failed hotkey registration replaces the pill with the compositor command.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): wire dictation, hotkeys, tray, and loopback`.

### Task 6: History

**Files:**
- Create: `crates/floure-app/src/views/history.rs`
- Modify: `crates/floure-app/src/app.rs`

**Interfaces:**
- Consumes: `History::search(query, from, to, mode, limit)`, `History::delete(ids)`, `History::set_favorite(id)`, `History::export(filtered)`.

- [ ] Build the title row, search, date/mode filters, favorite chip, grouped rows, multi-select, `Delete N` confirmation, load more, raw/cleaned expansion, copy, and `Add to dictionary…` deep link.
- [ ] Add loading, empty, no-match, and error states; hover actions also on focus and at coarse pointer; 44 px hit targets.
- [ ] Add tests for grouping, selection, filtered export, and deep-link prefill.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): add history`.

### Task 7: Stats

**Files:**
- Create: `crates/floure-app/src/views/stats.rs`, `charts.rs`
- Modify: `crates/floure-app/src/app.rs`

**Interfaces:**
- Consumes: `Insights::summary()`, `Insights::weekly_words()`, `Insights::heatmap()`, `Insights::voice_stats()`.

- [ ] Build the four-figure strip with tabular numerals and the definitions from the spec (Monday week, active-speech WPM).
- [ ] Draw the 12-week discrete heatmap (today = 1 px white outline), weekly bars with hover value and labelled max, and the compact most-active-day/hour/length/language row.
- [ ] Add the finding sentence with the ≥3-session comparison guard and the zero-history state; no invented data.
- [ ] Add tests for week boundary, WPM, comparison guard, and heatmap bucket thresholds.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): add stats`.

### Task 8: Configuration and onboarding

**Files:**
- Create: `crates/floure-app/src/views/config/{general,shortcuts,models,dictionary,cleanup,output,microphone,diagnostics}.rs`
- Create: `crates/floure-app/src/views/onboarding.rs`
- Modify: `crates/floure-app/src/app.rs`

**Interfaces:**
- Consumes: `Engine::apply_settings`, `Engine::download_model`, `Engine::start_mic_test`, `Dictionary` repository, `Diagnostics`, `SystemDeps`.

- [ ] Build the two-column Configuration page and all eight sections from the spec, with 44 px rows and a horizontal selector below 860 px.
- [ ] Implement model lifecycle states (downloading %, rate/ETA, paused/resume, verifying, failed/retry, disk-full, deleting-active confirmation) and a global download indicator.
- [ ] Implement Cleanup (provider/model/API key/reveal), Output, Microphone & Output (device, level, helper binaries, system-settings guidance), and Diagnostics (copy, checks, recent errors, paths, version).
- [ ] Implement staged Apply for engine-affecting rows; never restart while recording; show `Restarting engine…`.
- [ ] Build the 3-step onboarding with Back, resume, existing-model detection, no skip without an ASR model, and Wayland/GNOME copy.
- [ ] Add tests for staged-vs-immediate apply, onboarding step rules, and model-state transitions.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): add configuration and onboarding`.

### Task 9: Widget, errors, polish

**Files:**
- Create: `crates/floure-app/src/views/widget.rs`, `toasts.rs`, `errors.rs`
- Modify: `crates/floure-app/src/theme.rs`, `app.rs`

**Interfaces:**
- Produces: platform-matrix widget (`LayerShell` on wlroots, floating X11, unavailable GNOME/KDE Wayland); durable recent-errors log in Diagnostics.

- [ ] Implement the widget exactly as the matrix: layer-shell bottom overlay, status-only, no drag, keyboard interactivity off; X11 floating bottom-centre; GNOME/KDE shows `Widget unavailable on this desktop`.
- [ ] Add the in-app recording indicator fallback and `Show widget` in the tray when supported.
- [ ] Add the toast stack (4 s, 6 s errors) and the durable recent-errors log; `Details` opens the log entry; Retry is category-specific.
- [ ] Add reduced-motion fallbacks, focus rings, keyboard navigation, coarse-pointer visibility, and 720 px breakpoints.
- [ ] Capture screenshots at 960×700 and 1440×900; fix clipping, contrast, and spacing; run the detector on changed UI.
- [ ] Run `cargo test -p floure-app` and commit `feat(ui): add widget, error log, and polish`.

### Task 10: Packaging

**Files:**
- Modify: `Cargo.toml`, `crates/floure-core/Cargo.toml`, `crates/floure-app/Cargo.toml`
- Create: `packaging/floure.desktop`, `packaging/floure.metainfo.xml`, `packaging/icons/*`, `packaging/deb/*`, `packaging/rpm/*`
- Modify: `packaging/arch/PKGBUILD`, `LICENSE`

- [ ] Set `sherpa-onnx` to the default `static` feature; verify `readelf -d`/`ldd` shows no private `.so` dependency.
- [ ] Add workspace/package metadata: GPL-3.0-or-later, repository, homepage, readme, maintainer; `[[bin]] name = "floure"`.
- [ ] Add desktop entry (`Exec=floure`, `Icon=floure`, `StartupWMClass=floure`, `Keywords`, `X-GNOME-UsesNotifications`), icon set 32–512 + scalable SVG, metainfo, license/OFL.
- [ ] Configure `cargo-deb` (`depends = "$auto"`, assets) and `cargo-generate-rpm` (`auto-req`, same assets); update the PKGBUILD to build the new crate and install the new assets.
- [ ] Add a portable `.tar.gz` target; remove AppImage/linuxdeploy from the design and workflow.
- [ ] Add `floure --version` and `floure --diagnostics` GPU-free smoke paths.
- [ ] Run `cargo build --release -p floure-app`, `cargo deb --no-build -p floure-app`, `cargo generate-rpm -p floure-app`; inspect package contents and `ldd`; commit `build: package the gpui app`.

### Task 11: CI and release workflow

**Files:**
- Modify: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/*`
- Delete: frontend jobs, `Pin linuxdeploy AppImage plugin`, `APPIMAGE_EXTRACT_AND_RUN`, `NO_STRIP`, `tauri-action`

- [ ] Replace frontend CI with one Rust job: fmt, clippy `-D warnings`, `cargo test --workspace` with `gpui-pre-platform/test-support`, and a headless UI smoke.
- [ ] Add a PR packaging job: release build, `cargo deb --no-build`, `cargo generate-rpm`, assert `/usr/bin/floure`, desktop, icon, metainfo, and `ldd`.
- [ ] Fix `pull_request.branches` to include `master`.
- [ ] Keep `resolve-release`; target canonical `IntegerAlex/floure-core`; upload `.deb`, `.rpm`, `.pkg.tar.zst`, `.tar.gz` with `gh release upload --clobber`.
- [ ] Add a version-consistency check (tag = Cargo = deb = rpm = PKGBUILD).
- [ ] Validate workflow YAML and the PKGBUILD; commit `ci: build native packages`.

### Task 12: Remove Tauri and React, update docs

**Files:**
- Delete: `application/` (after the final bridge release), Tauri configs, frontend workflows
- Modify: `README.md`, `CONTEXT.md`, `docs/adr/*`, `.github/workflows/*`

- [ ] Delete `application/` only after the Tauri bridge release has written `ui-state.json` and a GPUI build passes the manual matrix.
- [ ] Update README: GPUI stack, GPU requirement, Wayland/GNOME limits, widget matrix, install commands, data paths, build/test commands.
- [ ] Update CONTEXT and add ADR-0005 (GPUI), ADR-0006 (license), ADR-0007 (packaging without Tauri).
- [ ] Remove sttauri/React references from docs and workflows; run `rg -n "tauri|pnpm|vite|react"` and justify anything left.
- [ ] Run the full verification suite and commit `chore: remove tauri app`.

### Task 13: Final verification

- [ ] `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`.
- [ ] Headless GPUI tests pass; manual smoke on Hyprland/Wayland and X11 passes.
- [ ] Install `.deb`, `.rpm`, Arch, and tar.gz in clean containers; `floure --version`, `--diagnostics`, `ldd`, and GUI startup with lavapipe pass.
- [ ] Screenshot review at 720, 960, and 1440 px: no clipping, one accent, contrast, focus, reduced motion.
- [ ] Confirm no fake stats, no focus theft, no lost settings/history, and a visible Wayland fallback on every trigger.
