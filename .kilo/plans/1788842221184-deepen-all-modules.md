# Deepen Rust Backend Modules

**Branch:** core-work  
**Base:** 1b4a318  

## Goal
Turn shallow Rust modules in `stt-ui/src-tauri/src/` into deep modules: small interfaces, concentrated behavior, clean seams, testable through those interfaces.

## Order
1. Candidate 3 — Add behavior to `AsrProfile` in `config.rs` (prerequisite for candidate 2)
2. Candidate 5 — Extract `ModelManager` from `models.rs` (prerequisite for candidate 2)
3. Candidate 2 — Deepen `pipeline.rs` into `PipelineController` + `LlmProcessor` (depends on 3, 5)
4. Candidate 4 — Consolidate `output.rs` + `lib.rs` `win32` into `SystemInteractor` (independent)
5. Candidate 1 — Extract `InsightsEngine` from `lib.rs` (independent)

## Candidate 3 — Add behavior to `AsrProfile`
**Interface:** add three methods to `AsrProfile`:
- `model_id(&self) -> &'static str`
- `model_dir(&self, base: &Path) -> PathBuf`
- `build_recognizer(&self, model_dir: &Path, threads: i32) -> Result<AnyRecognizer>`

**Implementation:**
- Move model IDs (`parakeet-tdt-0.6b-v2-int8`, `whisper-large-v3-turbo-q5_1`, `whisper-base-q5_1`) into `AsrProfile` variants or match arms inside `config.rs`.
- Move recognizer construction (threads, feature flags) from `pipeline.rs` into `config.rs`.
- `AnyRecognizer` becomes a small enum/trait object; construction logic lives in `config.rs`.

**Files changed:** `stt-ui/src-tauri/src/config.rs`, `stt-ui/src-tauri/src/pipeline.rs`

**Validation:** `cargo check` passes. `AsrProfile::build_recognizer` testable with temp dir + fake model files.

## Candidate 5 — Extract `ModelManager`
**Interface:**
```rust
pub struct ModelManager { model_dir: PathBuf }
impl ModelManager {
    pub fn new(model_dir: PathBuf) -> Self;
    pub fn status(&self) -> Vec<ModelStatus>;
    pub fn verify(&self, id: &str) -> bool;
    pub fn download(&self, id: &str, progress: impl FnMut(usize, u64)) -> Result<()>;
}
```

**Implementation:**
- Move `MODEL_MANIFEST`, `find_model`, `verify_model`, `download_model` into `ModelManager`.
- `MODEL_MANIFEST` becomes private to `models.rs`.
- `ModelStatus` type moves into `models.rs`.
- `check_model_status` Tauri command in `lib.rs` becomes thin call to `ModelManager::status`.

**Files changed:** `stt-ui/src-tauri/src/models.rs`, `stt-ui/src-tauri/src/lib.rs`

**Validation:** `cargo check` passes. `ModelManager::verify` testable with temp dir + fake model files.

## Candidate 2 — Deepen `pipeline.rs` into `PipelineController` + `LlmProcessor`
**Interface:**
```rust
pub struct PipelineController {
    config: AppConfig,
    app: tauri::AppHandle,
}
impl PipelineController {
    pub fn new(config: AppConfig, app: tauri::AppHandle) -> Result<Self>;
    pub fn run(&mut self) -> Result<()>;
    pub fn stop(&mut self);
}

pub struct LlmProcessor {
    config: AppConfig,
    app: tauri::AppHandle,
}
impl LlmProcessor {
    pub fn new(config: AppConfig, app: tauri::AppHandle) -> Self;
    pub fn process(&mut self, text: &str) -> Result<()>;
}
```

**Implementation:**
- `start_pipeline` becomes `PipelineController::new()` + `run()`.
- VAD model download/verification moves into `PipelineController::new()`.
- Audio capture, ASR dispatch, LLM dispatch, output dispatch all move inside `PipelineController::run()`.
- `process_llm` logic (prompt construction, streaming, event emission, typing, clipboard, history) moves into `LlmProcessor::process()`.
- `output.rs` `type_text`/`copy_to_clipboard` become thin wrappers around `SystemInteractor` (after candidate 4) or keep current logic until candidate 4 is done.

**Files changed:** `stt-ui/src-tauri/src/pipeline.rs`, `stt-ui/src-tauri/src/output.rs`, `stt-ui/src-tauri/src/lib.rs`

**Validation:** `cargo check` passes. `PipelineController` testable with mock `AppHandle` (use `tauri::test::mock_app`) and fake recognizers. `LlmProcessor::process` testable with fake LLM backend.

## Candidate 4 — Consolidate `output.rs` + `lib.rs` `win32` into `SystemInteractor`
**Interface:**
```rust
pub struct SystemInteractor;
impl SystemInteractor {
    pub fn type_text(text: &str) -> Result<bool>;
    pub fn copy_to_clipboard(text: &str) -> Result<bool>;
    pub fn get_platform_info() -> PlatformInfo;
    pub fn get_foreground_hwnd() -> u64;
    pub fn set_foreground_hwnd(hwnd: u64) -> bool;
}
```

**Implementation:**
- Move `win32` module from `lib.rs` into `output.rs` (or new `system_interactor.rs`).
- `output.rs::type_text`/`copy_to_clipboard` delegate to `SystemInteractor`.
- `lib.rs::type_text` Tauri command delegates to `SystemInteractor`.
- `widget.rs::detect_window_manager` keeps its own detection (widget concern), but clipboard/typing use `SystemInteractor`.

**Files changed:** `stt-ui/src-tauri/src/output.rs`, `stt-ui/src-tauri/src/lib.rs`, `stt-ui/src-tauri/src/widget.rs`

**Validation:** `cargo check` passes. `SystemInteractor::type_text` testable by manipulating env vars (`WAYLAND_DISPLAY`, `DISPLAY`) and asserting on command execution.

## Candidate 1 — Extract `InsightsEngine` from `lib.rs`
**Interface:**
```rust
pub struct InsightsEngine { conn: Connection }
impl InsightsEngine {
    pub fn compute(&self) -> InsightsData;
    pub fn voice_intelligence(&self) -> VoiceIntelligenceData;
}
```

**Implementation:**
- Move all SQL from `get_insights()` and `get_voice_intelligence()` into `InsightsEngine`.
- Move `InsightsData`, `InsightsStreak`, `InsightsHeatmapDay`, `WeeklyWordDay`, `VoiceIntelligenceData` into `insights.rs` (or `types.rs`).
- Move `ensure_dict_table` and dictionary commands into `InsightsEngine` or keep in `lib.rs` as thin adapters.
- Tauri commands become thin adapters: open DB, create `InsightsEngine`, call method, serialize.

**Files changed:** `stt-ui/src-tauri/src/lib.rs`, new `stt-ui/src-tauri/src/insights.rs`

**Validation:** `cargo check` passes. `InsightsEngine::compute` testable with `Connection::open_in_memory()` + seed data, no Tauri runtime.

## Global Validation
- `cargo check` after each candidate.
- Move existing `tests.rs` tests into the deepened module's `#[cfg(test)]` block where they belong.
- Ensure no `AppHandle` leaks across seams.

## Out of Scope
- Frontend changes.
- Changing the database schema.
- Adding new ASR/LLM backends.
