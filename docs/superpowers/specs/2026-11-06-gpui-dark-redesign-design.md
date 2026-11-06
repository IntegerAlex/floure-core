# GPUI Dark Redesign — Design

**Status:** draft for approval
**Date:** 2026-11-06
**Product record:** `PRODUCT.md`
**Branch:** `feat/gpui-dark-redesign`

## Intent

Replace the Tauri + React application with a native Rust GPUI application, and replace the current warm, card-heavy UI with a minimal monochrome interface built around the microphone. The migration is a full replacement: the webview shell, React pages, Vite build, and Tauri bundling are removed after feature parity.

## Why GPUI

GPUI renders through the GPU, removes the webview from the runtime, and lets the UI share Rust types with the audio/ASR/LLM pipeline. The result is one binary, one event loop, and no IPC boundary between UI and engine.

The cost is real: GPUI is pre-1.0, needs working Vulkan/OpenGL drivers, has no webview fallback, and its ecosystem is younger. This design pins exact versions and keeps the migration staged so every commit is buildable.

## Goals

- A native GPUI window with the complete Floure workflow: dictation, history, stats, configuration, onboarding, widget, tray.
- A near-black, monochrome visual system with one red recording accent; Apple-like restraint without glass or gradients.
- The existing pipeline and local data formats preserved: same config path, same SQLite history, same model directory, same loopback control server.
- A single Rust workspace with the engine logic reusable independently of the UI.
- Linux packaging for `.deb`, `.rpm`, `.pkg.tar.zst`, and a portable `.tar.gz`.

## Non-goals

- Light theme, theme switching, or user-selectable accent colours.
- i18n/RTL in the first GPUI release.
- Windows/macOS release packaging in the first release (the code remains portable; packaging follows).
- Cloud sync, mobile, or any account system.
- Replacing the ASR or LLM backends.
- Live word-by-word ASR partials. The current backend only produces final segments per VAD utterance; streaming ASR would require switching to an online recognizer. Cleanup tokens still stream.

## Anti-goals

- A dashboard of cards, KPI tiles, icon chips, or sparklines.
- A settings maze. Configuration is one page with clear sections, not a stack of modals.
- Hiding failure in a toast that disappears. Errors persist in the UI until dismissed or retried.
- Keeping both UIs alive indefinitely. The Tauri app is deleted at the end of the migration.

## Pinned visual direction

Minimal, monochrome, Apple-like dark. This is a user-pinned direction, so no style roll is used.

- One ground: `#000000`.
- Surfaces: `#0A0A0A`, `#141414`; borders `rgba(255,255,255,0.10)`; no shadows at rest.
- Text: `#F5F5F7` primary, `#A1A1A6` secondary, `#8E8E93` muted (≥4.5:1 on black), `#6E6E73` only at 17 px or larger.
- Accent: `#FF453A`, reserved for recording and live data. Error text uses `#FF6961`; destructive buttons use a neutral fill with red text, not white-on-red.
- Success `#30D158`, warning `#FFD60A` only for status text, never decoration.
- Type: Inter (OFL) bundled; 12/13/15/17/22/28 px; semibold only for titles and the active state; tabular numerals for every metric.
- Radius: 8 px controls, 12 px panels, full circle for the microphone.
- Spacing scale: 4/8/12/16/24/32/48.
- Motion: 120–180 ms ease-out; microphone pulse 1.8 s; transcript tokens fade in at 120 ms. A single reduce-motion gate covers every animation.
- Focus: 2 px white ring, 2 px offset; selected: `#141414` fill plus white text; disabled: 40% opacity and no pointer response.
- Icon set: bundled Lucide SVGs, 16/18/20 px, 1.5 px stroke; no icon fonts.

## Information architecture

Only four destinations, plus first-run onboarding and the widget:

| Destination | Job |
|---|---|
| **Dictate** | Is it recording, and what did it just hear? |
| **History** | What did I say, and when? |
| **Stats** | How much am I actually using this? |
| **Configuration** | What can I change right now? |

Models and Dictionary move inside Configuration and lose their standalone navigation items. Insights becomes Stats. The footer, theme selector, What's New gate, and standalone error page are removed; version lives at the bottom of the rail, and errors surface inline with a durable recent-errors log.

## Shell

- Frameless 960 × 700 window (min 720 × 560), custom 44 px title bar, draggable, double-click maximizes.
- 64 px icon rail on the left: Dictate (`mic`), History (`clock`), Stats (`chart`), Configuration (`sliders`). Active item is a white icon on `#141414`; inactive icons are `#6E6E73`. Every icon has a tooltip and accessible name.
- Product mark is a 20 px white monogram; name appears in the title bar, not repeated in the rail.
- Bottom of the rail: version and a small recording dot.
- Title bar: current destination name, right-aligned shortcut hint, window controls (minimize, maximize, close) as 32 px hit targets with 16 px glyphs.
- Content gutter: 32 px; one left edge across title and content.
- Keyboard: `Ctrl+1–4` switches destinations; `Escape` closes dialogs; `Space` toggles recording only when the main window has focus and no text field is active.
- Breakpoints: at ≤860 px the Configuration section list becomes a horizontal selector; History filters wrap to a second row; Stats figures wrap 2×2. At ≤720 px the rail collapses to 52 px with icons only.

## Screens

### Dictate (home)

The microphone is the largest thing on the screen and answers the screen's question immediately.

- 112 px circular mic button, vertically centred above the transcript. Idle: `#141414` fill, 1 px white/10 border, white mic glyph. Listening: accent fill, expanding ring, waveform under it. Transcribing/rewriting: white ring, spinner arc, status text.
- Status line under the mic: `Ready`, `Listening…`, `Stopping…`, `Transcribing…`, `Cleaning up…`, `Error`, `Tap to set up` when no model exists.
- Shortcut pill under the status. On Wayland where registration fails, replace it with `Set up your compositor key` and the `floure --toggle` command with a copy button.
- Live waveform: 24 bars, monochrome, flat; coalesce mic levels to ~30 Hz; no glow.
- Transcript panel: the latest utterance in 22 px text, previous lines in 15 px muted below it; `Copy`, `Cancel`, `Clear` as text buttons in the panel header. Cleanup tokens update the current segment in place. `Clear` is session-only and says history is unchanged.
- Session strip: WPM, words, elapsed, plus a muted `Parakeet · CPU` model/device label. One line, no tiles.
- Inline error row above the transcript when the engine, paste, or model fails; `Retry` and `Details`. Retry is category-specific: engine restart, model reload/download, paste re-deliver the same text, mic re-open the device.
- Quick Type/Clipboard toggles beside the session strip; full output settings live in Configuration.
- Empty state: `Press the shortcut and speak` with the shortcut pill; no illustration.
- Blocking states: no model (`Download a model to start` with the recommended model), engine warming, engine stopped/crashed (`Restart engine`), no speech detected (`Nothing heard — try again`).

### History

- Title row: `History`, count, search field, `Export` menu (CSV/text, filtered result with a count), date filter, mode filter.
- Rows grouped by Today / Yesterday / date; each row: time, transcript, favorite star, copy/delete actions, and raw/cleaned expansion when they differ.
- Multi-select with `Delete N` confirmation; filter chip for favorites; load-more pagination.
- Row height 56–72 px depending on wrapping; no cards, no zebra; one divider tone.
- Search is FTS-backed and immediate; empty result keeps the frame and says `No transcripts match`.
- Hover-only actions also appear on focus and at coarse pointer; hit targets ≥44 px.
- `Add to dictionary…` on a row deep-links to Configuration > Dictionary with the phrase prefilled.
- Delete requires the existing confirmation dialog.

### Stats

- Four figures in one strip: words this week, sessions, current streak, average WPM. Labels above values, values 28 px semibold, units 12 px muted.
- Definitions: week starts Monday 00:00 local; WPM = words ÷ active speech duration; sessions = utterances with a saved transcript.
- One 12-week activity grid (7 rows), four discrete white opacity steps; today is a 1 px white outline, not accent; a text summary carries the totals.
- One weekly words bar chart, 12 bars, monochrome, hover value and a labelled max; no legends or gridlines beyond a baseline.
- One finding sentence under the title, computed from real history (e.g. `You dictated 1,240 words this week, 18% more than last week`). Show the comparison only with at least three prior-week sessions; otherwise a neutral sentence. If there is no history, show zeros and `Your first session will appear here`.
- Keep the useful Insights data: most active day/hour, average dictation length, and most-used language as one compact row, not extra cards.

### Configuration

One page, two columns: 200 px section list on the left, content on the right. Sections: General, Shortcuts, Models, Dictionary, Cleanup, Output, Microphone & Output, Diagnostics.

- **General**: launch at login, start hidden (only when a tray host exists), language, ASR profile, reduce motion, run setup again.
- **Shortcuts**: press-to-record recorder with “already in use”, hold/toggle, tray toggle, loopback command with copy button. On Wayland, compositor binding snippets replace the hotkey pill.
- **Models**: ASR and LLM cards with size, status, download/delete, disk usage; downloading (percent/rate/ETA), paused/resume, verifying, failed/retry, disk-full, deleting-active-model confirmation. No marketing copy; one functional line per model.
- **Dictionary**: searchable list, add/edit dialog, categories, favorites, CSV import/export; explanation that entries bias decoding. `hotwords` appears here as a row.
- **Cleanup**: mode (off/cleanup/bullet/email/commit), provider (local/OpenRouter), local model, OpenRouter model and API key with reveal toggle; key policy inline.
- **Output**: type/clipboard toggles, trailing space, auto-submit, paste method, audio feedback and volume.
- **Microphone & Output**: device selection, open status, mic test meter, helper-binary presence (`wtype`, `xdotool`, `wl-clipboard`, `ydotool`) with install hints and “Open system settings”. This is not a web permission prompt.
- **Diagnostics**: copy diagnostics, system checks, recent errors log, data paths, version.
- Every row is 44 px with a label, optional hint, and control. Cosmetic/output rows apply immediately; engine-affecting rows (ASR profile, language, provider/model) stage a row-level Apply and defer while recording. Never restart the engine on a mis-click.
- No global Save button.

### Onboarding

Three steps in the same shell, no modal stack:
1. **Microphone & Output** — device, level test, helper-binary checks. No fake permission request.
2. **Models** — recommended Parakeet + S1-Mini preselected; existing models detected; download progress inline. Skip is allowed only when at least one ASR model exists.
3. **Ready** — shortcut or compositor binding, tray/loopback instructions, `Start dictating`.

Back works on every step; progress survives quit; “Run setup again” lives in General. Returning installs skip the model step only when models actually exist. On Wayland, show the compositor command, not a hotkey that will not fire.

### Widget

The widget is a platform-matrix feature, not a universal always-on-top window:

- **Hyprland / sway (wlroots):** layer-shell surface, `Overlay` layer, anchored bottom, 240 × 56, keyboard interactivity off, status-only, no drag. Clicking is reserved for the tray/hotkey/loopback.
- **X11:** normal floating window at bottom-centre, initially placed there; status-only; WM controls drag.
- **GNOME / KDE Wayland:** no positioned always-on-top surface. The widget is unavailable; the tray/loopback and a compact in-app recording indicator remain. Onboarding says so.
- The widget never auto-shows on Wayland without verified no-activate, because showing it can steal focus from the target app.
- Closing the widget is allowed; the tray menu has `Show widget` when the platform supports it.
- A `PttOverlay`-style minimal indicator remains available inside the main window when the external widget is unavailable.

### Tray

Linux StatusNotifierItem via `ksni` (`default-features = false`, `async-io`): Open Floure, Toggle recording, Configuration, Show widget, Quit. The tray icon reflects idle/recording state. When no SNI host exists, the app still runs with no tray and the loopback/CLI is the documented trigger.

Lifecycle: `QuitMode::Explicit`. Linux GPUI has no per-window hide, so “close to tray” destroys the main window and recreates it from the app-level entity; if no tray host exists, close quits. Start-hidden is enabled only when a tray host exists. Closing or quitting while recording flushes the in-flight transcript before exit.

## Architecture

### Workspace

```
crates/
  floure-core/     # engine: audio, vad, asr, llm, output, pipeline,
                   # config, models, history, dictionary, insights, diagnostics
  floure-app/      # GPUI binary: app state, views, tray, hotkeys,
                   # control server, CLI, widget window
packaging/         # desktop entry, icons, metainfo, PKGBUILD, deb/rpm metadata
docs/              # ADRs, design, plans
```

The existing `application/src-tauri` becomes the source for `floure-core`; Tauri-specific code (`commands`, `AppHandle`, `emit`, plugins, tray, widget) is removed or replaced with plain Rust APIs and an event channel. `lib.rs` is first split into `core::{history,dictionary,insights,system_deps,models_api}` with no behaviour change; that split is the real cost of phase 2.

### Dependency pins

```toml
gpui-pre = "=0.3.11"
gpui-pre-platform = { version = "=0.3.11", features = ["wayland", "x11"] }
gpui-pre-wgpu = "=0.3.11"
gpui-component = "=0.9.0"
gpui-base = "=0.9.0"
gpui-kit-assets = "=0.9.0"
```

`gpui-pre-platform` is mandatory: without `wayland`/`x11` the backend selection silently runs headless and `open_window` fails at runtime. Commit a root `Cargo.lock` and `rust-toolchain.toml`; all four GPUI crates move together. `gpui-component` is used for input, scroll, virtual list, table, menu, and date picker only; its `TitleBar` paints a gradient and `WindowBorder` paints a shadow, so the app draws its own 44 px title bar, rail, and borders with plain GPUI primitives.

### Engine boundary

`floure-core` exposes:

- `Engine::new(config, paths) -> Result<Engine>` — process singleton; a second construction returns a typed error, never aborts on `BackendAlreadyInitialized`.
- `Engine::start()`, `Engine::stop()` (signals and returns), `stop_blocking()` (tests/exit), `Engine::cancel()`.
- `Engine::subscribe() -> async_broadcast::Receiver<EngineEvent>` — cloneable fan-out; every view/tray/status reads a synchronous `Engine::state()` snapshot.
- `Engine::apply_settings(update)` — mutates config, saves, invalidates the engine key on the next press.
- `Engine::download_model(id) -> Result<()>` — spawns internally; progress only via events; `delete_model` refuses while downloading.
- `Engine::start_mic_test()` / `stop_mic_test()` — preview capture for onboarding/permissions, emitting `MicLevel` only.
- `History`, `Dictionary`, `Insights` repositories.
- `Config` load/save, `Diagnostics`, typed `SystemDeps`.

`EngineEvent`:

```
State(EngineState)
MicLevel(f32)
Final { utterance_id, text, latency_ms, duration_sec }
CleanupStarted { utterance_id }
CleanupToken { utterance_id, token }
CleanupDone { utterance_id, text }
CleanupFailed { utterance_id, message }
OutputFailed { utterance_id, kind: Typing|Clipboard, message }
Committed { utterance_id, history_id }
AsrReady { backend }
ModelProgress { id, percent, bytes, rate, eta }
ModelDone { id }
ModelFailed { id, message }
Error { kind, message, recoverable }
Stopped { flushed: bool }
Cancelled { utterance_id }
```

`Final` arrives per VAD segment; no `Partial`. `Committed` is emitted after the history insert, so History/Stats refresh cannot race the row. `stop()` must not join on the UI thread; the worker emits `Stopped` after the flush+join. `cancel()` cannot interrupt a synchronous llama/sherpa decode; it sets a flag checked before typing, clipboard, and history, and emits `Cancelled`.

### Threading

- Audio callback: `cpal`, lock-free level emission.
- VAD/ASR/LLM: dedicated worker thread; llama.cpp is process-global.
- GPUI: foreground executor; each window drains its own broadcast receiver via `cx.spawn`, coalesces mic levels to ~30 Hz, and updates the entity.
- Locks: recover poisoned mutexes with `unwrap_or_else(|e| e.into_inner())`; `OMP_NUM_THREADS` is set at the first line of `main()`.

### Hotkeys and triggers

- X11/Windows/macOS: `global-hotkey`, hold and toggle modes, only enabled when `WAYLAND_DISPLAY` is unset.
- Wayland: no reliable global grab; keep `127.0.0.1:17833` (`/status`, `/toggle`, `/start`, `/stop`, `/show`) and `floure --toggle` CLI. `--toggle` POSTs to the loopback server and exits before `Application::new`, so it never starts a second instance. Document Hyprland/sway/GNOME/KDE snippets.
- Tray menu calls the same `toggle()` path.
- Keyboard/pointer focus is never moved by Floure; the widget is status-only.

### Data compatibility

Keep `~/.config/floure/config.json`, `~/.local/share/floure/history.db`, `~/.local/share/floure/models/`, and the legacy STT migration. Use literal `floure` paths, not a new `ProjectDirs` qualifier; add tests asserting the exact strings.

Two migrations are required:

1. **Webview settings bridge.** Hotkey, PTT mode, sound enabled/volume, and `onboarding_completed` live only in WebKit localStorage. Ship one final Tauri point release that writes them into `config.json` (or `ui-state.json`), and have GPUI read and retire them. If the local-storage read is unreliable, show an explicit “re-enter your shortcut” step.
2. **History/Stats schema.** There is no FTS5 table today; `get_history` loads up to 50,000 rows and filters in JS. Add `transcripts_fts` (external-content) with triggers and an idempotent backfill on open. `save_to_history` currently hardcodes `language='en'` and `duration_sec=0.0`; pass the real profile and VAD segment duration (`segment.len()/16000`) so WPM and language stats are not empty. Keep `created_at` as UTC on disk and local in the view.

## Packaging and CI

Tauri’s bundler is gone. Build the binary once, then package it.

- Binary name is `floure` (`[[bin]] name = "floure"`), not `floure-app`.
- Revert `sherpa-onnx` from `shared` to the default `static` feature. The shared build links `libsherpa-onnx-c-api.so`/`libonnxruntime.so` and no package ships them; static is self-contained and the Windows-only reason for shared (ADR-0004) does not apply. Verify with `readelf -d` and `ldd`.
- `.deb` with `cargo-deb`, `$auto` dependencies, explicit `.desktop`, icon, metainfo, license, and `libgomp1` if OpenMP is linked.
- `.rpm` with `cargo-generate-rpm`, `auto-req`, the same assets.
- Arch `.pkg.tar.zst` from `packaging/arch/PKGBUILD` updated to `cargo build --release -p floure-app` and the new runtime deps (no webkit/gtk/appindicator).
- Portable `.tar.gz` (binary + desktop/icon + LICENSE/OFL + install note) replaces AppImage. AppImage is dropped for the first GPUI release; the tar.gz covers distro-agnostic users without FUSE/linuxdeploy.
- Desktop assets move to `packaging/icons/` with 32/48/64/128/256/512 + scalable SVG before `application/` is deleted. Add `StartupWMClass=floure`, `Keywords`, `X-GNOME-UsesNotifications`, and `packaging/floure.metainfo.xml` validated by `appstreamcli`.
- Runtime deps are derived from the actual binary: `libvulkan1` + ICD recommendation (`mesa-vulkan-drivers`), `libxkbcommon0`, `libwayland-client0`, `libfreetype6`/`libfontconfig1`, `libasound2`, `libssl3`, `libgomp1`, `libstdc++6`, possibly `libegl1`/`libgles2` and `libdbus-1-3`. Use `dpkg-shlibdeps`/`auto-req`, not a hand list.
- Version stamping: one tag → `[workspace.package] version`; cargo-deb/rpm derive theirs; PKGBUILD maps `-`→`_`; the rail renders `env!("CARGO_PKG_VERSION")`. Add a release check that deb/rpm/PKGBUILD versions agree with the tag.
- Canonical release repo is `IntegerAlex/floure-core`; `resolve-release` and `gh release upload --repo` must target it, and the PKGBUILD must fetch the same repo.
- CI: `rust` job (fmt, clippy `-D warnings`, `cargo test --workspace` with `gpui-pre-platform/test-support`), plus a `package` job on PRs (`cargo build --release`, `cargo deb --no-build`, `cargo generate-rpm`, assert `/usr/bin/floure`, desktop, icon, `ldd` resolution). Tag releases build all four artifacts and upload via `gh release upload --clobber`. Fix `pull_request.branches` to include `master`.
- GPU-free smoke: install packages in a clean container, run `floure --version` and `floure --diagnostics`, check `ldd`; GUI smoke uses `mesa-vulkan-drivers` + lavapipe + Xvfb and asserts the window opens. “Launch and dictate” stays a manual check.
- Remove `tauri-action`, the AppImage plugin pin, `APPIMAGE_EXTRACT_AND_RUN`, and `NO_STRIP` from the release workflow.

## Migration phases

Each commit builds and tests.

1. **Scaffold** — workspace crates, exact dependency pins, `Cargo.lock`, `rust-toolchain.toml`, `PRODUCT.md`, this design, branch.
2. **Core extraction** — split `lib.rs` into typed repositories, remove Tauri types, add the broadcast event bus and async stop/cancel, port Rust tests, add the FTS/duration migrations and webview-settings bridge.
3. **GPUI shell** — verify gpui-pre-platform with a spike (frameless, drag, resize, maximize, layer-shell, AccessKit), then window, rail, title bar, tokens, Dictate idle/listening/transcribing/error, hotkey, loopback, tray, lifecycle.
4. **History and Stats** — FTS search, grouping, selection, export, real aggregates, empty/loading/error states, breakpoints.
5. **Configuration and onboarding** — all sections, model lifecycle, dictionary, cleanup/API key, output routing, mic test, diagnostics, first-run, settings migration.
6. **Widget and polish** — platform-matrix widget, toasts/recent errors, reduce motion, focus rings, keyboard navigation, visual pass at 960×700 and 1440×900.
7. **Packaging, CI, cleanup** — static sherpa, deb/rpm/Arch/tar.gz, release workflow, delete `application/` and Tauri configs, update README/CONTEXT/ADRs.

## Testing and acceptance

- `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`.
- Core tests cover config/path migration, localStorage bridge, history FTS migration/search/export, dictionary merge and decode bias, model manifest/download/resume, hotkey parsing, cleanup gating, and loopback status.
- Event contract test: `Listening → Final* → CleanupStarted/Token*/CleanupDone → Committed → Idle`, exactly one terminal state, error path.
- `cancel()` produces no `Final`, no typed text, and no history row; `stop()` flushes the tail; two subscribers both receive events; a stalled subscriber does not block the pipeline; a second `Engine::new` fails cleanly; dropping an Engine joins the worker.
- GPUI headless tests use the test context for destination changes, onboarding, dialogs, and state transitions. No GPU is needed for headless tests.
- Manual smoke on Hyprland/Wayland and X11: hotkey or loopback, typing into a focused editor, clipboard, tray toggle, widget behavior, model download, restart persistence, close-to-tray, cancel, offline cloud cleanup.
- Packaging smoke: install `.deb`, `.rpm`, Arch, and tar.gz in clean containers; `floure --version`/`--diagnostics`; `ldd` clean; GUI startup with lavapipe.
- Visual acceptance: no gradients, no glass, no cards-with-icons, one accent, one left edge, contrast ≥4.5:1, no clipped text at 720 px, focus visible everywhere, reduced motion honored.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| GPUI API churn | Pin `gpui-pre`/platform/wgpu/component exact versions; commit `Cargo.lock`; keep primitives behind local modules. |
| No GPU / broken drivers | Probe wgpu adapters before `Application::run`; show stderr + notification with driver guidance; document lavapipe; support `ZED_DEVICE_ID` and a `FLOURE_DEVICE_ID` alias. |
| Wayland hotkey gap | Loopback server + CLI; compositor snippets replace the hotkey pill; disable `global-hotkey` when `WAYLAND_DISPLAY` is set. |
| GNOME Wayland typing gap | Mutter lacks `virtual-keyboard`; document clipboard + paste, optional `ydotool`; do not claim full parity. |
| Tray support varies | `ksni` SNI; no-tray mode still works; start-hidden only with a tray. |
| Widget positioning | Platform matrix: layer-shell on wlroots, normal floating on X11, unavailable on GNOME/KDE Wayland; status-only. |
| Settings lost | Final Tauri bridge release writes localStorage values into `config.json`; GPUI migrates and retires them. |
| History/Stats empty | FTS migration; real duration/language on insert; test fixtures. |
| Package fails to start | Static sherpa/llama; `$auto` deps; `ldd` + package smoke in CI. |
| Accessibility regressions | AccessKit roles/labels, keyboard navigation, focus rings, reduce-motion gate, one Orca pass on Hyprland and X11. |
| Longer compile times | Workspace caching; release build only on tags; `rust-toolchain.toml`. |

## Feature parity matrix

| Current | New location |
|---|---|
| Onboarding wizard (5 steps) | 3-step onboarding |
| Live transcription feed | Dictate transcript panel |
| Push-to-talk hotkey/modes | Dictate + Configuration > Shortcuts |
| History search/export/favorites/bulk delete | History |
| Insights heatmap/streaks/voice stats | Stats |
| Dictionary + hotwords | Configuration > Dictionary |
| Models | Configuration > Models |
| Settings | Configuration |
| Cloud cleanup/API key | Configuration > Cleanup |
| Widget | Platform-matrix widget |
| Tray | Tray via ksni |
| Toasts/errors | Inline errors + durable recent-errors log |
| Theme selector | Removed (fixed dark) |
| What's New gate | Removed |
| Footer | Version in rail |
| Loopback server | Kept, documented in Shortcuts |
| Diagnostics | Configuration > Diagnostics |
| AppImage | Portable `.tar.gz` |

## Resolved decisions

1. **Licensing.** Relicense to `GPL-3.0-or-later` so the Apache-2.0-only GPUI and gpui-component dependencies are compatible. Update `LICENSE`, Cargo metadata, package licenses, and README.
2. **Widget scope.** Ship the platform matrix: layer-shell on Hyprland/sway, normal floating on X11, unavailable on GNOME/KDE Wayland. The widget is status-only and never moves focus.
3. **GNOME support.** Support GNOME with clipboard-only output and optional `ydotool`; do not claim full typing parity. Document the limitation in onboarding and Configuration.
