# Floure — Local Speech-to-Text

Floure listens to your mic, transcribes your speech, cleans it up with an LLM, and types it into whatever you're focused on. No cloud required. No account. The default setup runs entirely on your machine.

```
mic (cpal) → Silero VAD → Parakeet / Whisper (sherpa-onnx)
  → LLM cleanup (local S1-Mini via llama.cpp, or OpenRouter cloud)
  → type into focused window / clipboard
```

### ▶ [Watch the promo](promo.mp4)

## Models

Downloaded on demand from the Models page (resumable, verified — see `docs/adr/0003-model-downloads.md`).

| Model | Backend | Size | Notes |
|---|---|---|---|
| Parakeet TDT 0.6B v2 (int8) | sherpa-onnx | ~460 MB | Default ASR. Fast English dictation. Recommended. |
| Whisper large-v3-turbo | sherpa-onnx | ~540 MB | Multilingual, high accuracy |
| Whisper base | sherpa-onnx | ~200 MB | Lightweight, any language |
| Silero VAD | sherpa-onnx | ~630 KB | Speech detection, auto-downloaded |
| S1-Mini Q4_K_M | llama.cpp | ~462 MB | Default local LLM for cleanup. Recommended. |
| Gemma 3 1B IT Q4_K_M | llama.cpp | ~806 MB | Alternative local LLM |

Cloud cleanup (optional): OpenRouter (`OPENROUTER_API_KEY`, default `openai/gpt-4o-mini`).

## Run from source

Prerequisites: pnpm, Rust toolchain, a mic, and (Linux) PipeWire/PulseAudio.

```bash
cd application && pnpm install
pnpm tauri dev          # full desktop app (Vite + Rust backend)
```

Useful slices:

```bash
cd application && pnpm dev                 # frontend only
cd application && pnpm build               # frontend build
cd application/src-tauri && cargo check    # Rust check
cd application/src-tauri && cargo test     # Rust tests
cd application && npx vitest run           # frontend tests
```

### Local GPU offload (optional)

Default builds link llama.cpp statically with CPU inference and need no Vulkan
toolchain — the result is a single self-contained binary. ASR is CPU either way
(int8 is already sub-second per utterance).

To offload the local LLM to a GPU, build the Vulkan backend:

```bash
cd application && pnpm tauri build -- --features vulkan
```

That requires a Vulkan toolchain at build time (Vulkan loader + headers, a GPU
with a Vulkan ICD, `glslc` and SPIRV-Headers). At runtime the device is picked
automatically — discrete NVIDIA → AMD → CPU (`application/src-tauri/src/compute.rs`) — and can be overridden with `FLOURE_COMPUTE=cpu|vulkan` and `FLOURE_MAIN_GPU=<index>`.

## Configuration

| What | Where |
|---|---|
| App config | `~/.config/floure/config.json` |
| Transcripts (SQLite + FTS5) | `~/.local/share/floure/history.db` |
| Models | `~/.local/share/floure/models/` |
| Diagnostics record | `~/.local/share/floure/diagnostics.json` |
| Data dir override | `STT_DATA_DIR` env var |

Copy `.env.example` to `.env` for API keys and optional overrides.

## Desktop UI

Every section is a page in the main window — nothing opens as a modal:

- **Onboarding wizard** — system checks, mic test, model download, permissions
- **Live transcription feed** — mic level, waveform, push-to-talk
- **Model management** — browse, download with progress, status, delete, disk usage
- **History** — full-text search over past transcripts (SQLite FTS5), date-range filter, recopy, favorites
- **Dictionary** — custom vocabulary that also biases recognition (see below)
- **Settings** — ASR profile, language, hotwords, LLM provider/mode, API keys, push-to-talk, permissions, diagnostics
- **Insights** — usage heatmap, streaks, stats
- **Widget mode** — compact always-on-top mini window; **system tray** with start/stop

## Push-to-talk

Hold to record, release to transcribe.

| Platform | Trigger |
|---|---|
| Windows | `Ctrl+Shift+F12`, or hold bare `Ctrl+Win` |
| Linux (X11) | `Ctrl+Shift+F12` |
| Linux (Wayland) | The loopback control server — see below |
| Any | Tray menu (Start / Stop), or the widget's mic button |

### Wayland: why the hotkey may not fire

Wayland has no core key-grab, and whether `org.freedesktop.portal.GlobalShortcuts`
works is up to the compositor — GNOME and KDE implement it, wlroots-based
compositors (Sway, Hyprland) historically do not. A hotkey can therefore
register successfully and still never fire.

Floure ships a loopback-only control server that always works. Bind a compositor
key to it:

```bash
curl -X POST localhost:17833/toggle
```

```ini
# Sway / i3
bindsym $mod+d exec "curl -X POST localhost:17833/toggle"
```
```ini
# Hyprland
bind = $mod, D, exec, curl -X POST localhost:17833/toggle
```

Settings → Push-to-Talk shows the same command with a copy button. The server is
also what the Waybar and `wm-config/` integrations drive:

| Endpoint | Effect |
|---|---|
| `GET /status` | `{"state":"listening"\|"idle","recording":bool}` |
| `POST /toggle` | flip recording |
| `POST /start` / `POST /stop` | explicit control |
| `POST /show` | focus the main window |

Port defaults to 17833; override with `FLOURE_PORT`. The server binds to
`127.0.0.1` only.

## Dictionary and recognition accuracy

Words added on the Dictionary page are not just a find-and-replace table: the
replacement forms are merged into the recogniser's decode bias, so a correction
improves future transcriptions rather than only the stored text. Favourites are
weighted first, and the list is capped so a large dictionary cannot slow the
beam search. Manual hotwords from Settings are merged in alongside them.

## Troubleshooting

Settings → **Diagnostics** has a **Copy diagnostics** button that copies a local
record of what Floure ran against: OS and version, display server
(wayland / x11), audio server (pipewire / pulseaudio / alsa), the microphone
format it negotiated (sample rate, channels, sample format), and the selected
engines. It is written to `~/.local/share/floure/diagnostics.json` and is never
sent anywhere — copying it is a deliberate action. `DO_NOT_TRACK=1` disables
even the local write.

Most audio problems are environment-specific, and that record is what makes them
diagnosable:

- **Microphone not detected** — a saved device that has gone missing falls back
  to the system default instead of failing, and every sample format cpal reports
  is handled (f32, f64, i8–i64, u8–u64). If a device still fails, the
  diagnostics record names the format it reported.
- **Bluetooth headset** — switching between A2DP (44.1/48 kHz, stereo) and HFP
  (8/16 kHz, mono) mid-recording changes the device under the stream. Floure
  stops the run cleanly and says so, rather than typing from a dead buffer.
- **Last words missing** — the trailing segment needs ~0.7 s of silence to
  close, derived from the negotiated sample rate rather than a fixed sample
  count, so high-rate microphones are not cut short.

Transcripts are saved with raw text, cleaned text, LLM mode, provider, model, and timestamp.

## Docs

- [CONTEXT.md](CONTEXT.md) — project brief, layout, commands
- [docs/adr/](docs/adr/) — architecture decisions (native backend, DB path, model downloads)
- [docs/](docs/) — algorithms and voice research notes
- [docs/ux/](docs/ux/) — UX and cross-environment robustness research

## License

GNU General Public License v2.0 — see [LICENSE](LICENSE).

---

<p align="center">
  <sub>Designed and developed by <a href="https://www.akshatkotpalliwar.in/"><b>Akshat Kotpalliwar</b></a></sub>
</p>
