# Security Best Practices Report

**Project:** Floure (Tauri 2 desktop STT app)  
**Scope:** Frontend (React 19 + TypeScript + Vite) + Backend (Rust + Tauri 2)  
**Date:** 2026-09-08  
**Reviewer:** Kilo (passive scan during development)

---

## Executive Summary

The project follows several good security defaults: parameterized SQL, no `dangerouslySetInnerHTML`, hardcoded model URLs, and a configured CSP. However, there are notable gaps around local API authentication, file deletion validation, and a potential CSP/connect-src mismatch.

| Severity | Count |
|----------|-------|
| High     | 1     |
| Medium   | 3     |
| Low      | 3     |
| Info     | 4     |

---

## High

### SEC-001: Local API server has no authentication

**Location:** `stt-engine` sidecar binary (port 8765)  
**Evidence:** Frontend connects to `http://127.0.0.1:8765/api/*` and `http://127.0.0.1:${wsPort}` without any auth token or origin check (`App.tsx:861`, `api-web-audio.ts:37`).  
**Impact:** Any local process on the machine can read transcripts, insights, dictionary entries, and inject audio/commands. Data exfiltration or manipulation by local malware is possible.  
**Fix:** Add a shared secret or Unix domain socket permission check between the Tauri app and `stt-engine`. Alternatively, add a simple token handshake.  
**Mitigation:** Document the trust model: this is a local-only API, so exposure is limited to local processes.  
**False positive notes:** If `stt-engine` already validates the caller via `SO_PEERCRED` or similar, verify at runtime.

---

## Medium

### SEC-002: CSP `connect-src` may block local API connections

**Location:** `stt-ui/src-tauri/tauri.conf.json`  
**Evidence:**
```json
"security": {
  "csp": "default-src 'self'; ... connect-src 'self' ipc: http://ipc.localhost; script-src 'self'"
}
```
The frontend connects to `http://127.0.0.1:8765` for history, insights, and WebSocket audio streaming. This origin is not in `connect-src`.

**Impact:** If the CSP is enforced, the app cannot communicate with its own local backend, breaking core functionality. If the CSP is not enforced, the app lacks defense-in-depth against data exfiltration to attacker-controlled origins.  
**Fix:** Add `http://127.0.0.1:8765` to `connect-src` (and any other local ports used). Example: `connect-src 'self' ipc: http://ipc.localhost http://127.0.0.1:8765;`.  
**Mitigation:** Verify at runtime whether CSP violations are being reported in the WebView console.  
**False positive notes:** Tauri v2 may relax CSP for IPC; verify actual WebView behavior.

### SEC-003: Arbitrary file deletion via `delete_model_file`

**Location:** `stt-ui/src-tauri/src/lib.rs:901-910`  
**Evidence:**
```rust
#[tauri::command]
fn delete_model_file(path: String) -> Result<(), AppError> {
    let p = std::path::Path::new(&path);
    if p.is_dir() {
        std::fs::remove_dir_all(p).map_err(AppError::Io)?;
    } else if p.is_file() {
        std::fs::remove_file(p).map_err(AppError::Io)?;
    }
    Ok(())
}
```
The function accepts any user-provided path and deletes it without validating that it resides within the app's data directory.

**Impact:** A compromised frontend (XSS) or malicious IPC caller could delete arbitrary files the app has access to.  
**Fix:** Validate that the path is within the app's model/data directory before deletion. Use `std::path::Path::canonicalize()` and check the prefix.  
**Mitigation:** Ensure the frontend can only obtain model paths from the backend's `get_models` command.  
**False positive notes:** If Tauri's filesystem plugin already restricts write access to specific directories, verify those restrictions cover this path.

### SEC-004: LLM API keys sourced from environment variables

**Location:** `stt-ui/src-tauri/src/llm.rs:140-150`  
**Evidence:**
```rust
let api_key = match backend {
    LlmBackend::DeepSeek => std::env::var("DEEPSEEK_API_KEY").ok(),
    LlmBackend::OpenRouter => std::env::var("OPENROUTER_API_KEY").ok(),
    LlmBackend::Local => None,
};
```
API keys are read from environment variables at runtime.

**Impact:** If these keys are baked into the binary at build time, they can be extracted by reverse engineering. If injected at runtime, they must be passed securely.  
**Fix:** Document the key injection mechanism. Prefer runtime injection via Tauri's plugin-store or OS keychain rather than build-time env vars. Never commit keys to the repo.  
**Mitigation:** Ensure `DEEPSEEK_API_KEY` and `OPENROUTER_API_KEY` are not present in any `.env` files committed to version control.  
**False positive notes:** If keys are loaded from OS keychain at runtime, risk is low.

---

## Low

### SEC-005: `localStorage` stores settings accessible to XSS

**Location:** `stt-ui/src/App.tsx:108`, `components/SettingsPanel.tsx:23`  
**Evidence:**
```ts
const saved = localStorage.getItem(LOCAL_STORAGE_KEY);
localStorage.setItem(LOCAL_STORAGE_KEY, JSON.stringify({ ...settings }));
```
Settings, hotkeys, and onboarding state are stored in `localStorage`.

**Impact:** If an XSS vulnerability is introduced, all stored settings are accessible. Not critical for a local-only desktop app with no remote content.  
**Fix:** Consider Tauri's `plugin-store` (already a dependency) for persisted settings.  
**Mitigation:** Current XSS risk is low because the app loads no remote content.

### SEC-006: No source map restrictions in production

**Location:** `stt-ui/vite.config.ts` (implied)  
**Evidence:** No configuration found to restrict source map publication.

**Impact:** If source maps are published to production, attackers can reverse-engineer the frontend code more easily.  
**Fix:** Disable source map generation in production builds or ensure they are only published to error-reporting services behind auth.  
**Mitigation:** Vite does not emit source maps by default unless configured.

### SEC-007: Hardcoded localhost URLs without scheme validation

**Location:** `stt-ui/src/components/HistoryPage.tsx:19`, `stt-ui/src/api-web-audio.ts:37`  
**Evidence:**
```ts
const API_BASE = "http://127.0.0.1:8765/api";
const socket = io(`http://127.0.0.1:${wsPort}`, { transports: ["websocket"] });
```
URLs are hardcoded to `http://127.0.0.1`.

**Impact:** Low for a local desktop app. If the local server is ever replaced with a remote one, the hardcoded `http` scheme could expose data in transit.  
**Fix:** Document that the local API is HTTP-only. For future remote support, add TLS.  
**Mitigation:** The app is designed for local use only.

---

## Info / Good Practices Observed

1. **SQL injection prevented** — All database queries in `lib.rs` use parameterized statements (`?1`, `?2`). Example: `conn.execute("INSERT INTO transcripts ... VALUES (?1, ?2, ...)", rusqlite::params![...])`.

2. **No XSS escape hatches** — No `dangerouslySetInnerHTML`, `innerHTML`, `eval()`, or `new Function()` found in app source code.

3. **Model URLs are tamper-proof** — Download URLs are hardcoded in both Rust (`models.rs`) and TypeScript (`store.ts`), preventing runtime manipulation.

4. **CSP is configured** — `tauri.conf.json` includes a CSP that blocks `unsafe-eval` in `script-src` and restricts sources.

5. **No secrets in frontend** — No API keys, tokens, or passwords found in TypeScript source or `.env` files.

6. **Dependency lockfile present** — `pnpm-lock.yaml` is committed, enabling reproducible installs.

7. **Tauri capabilities are scoped** — `capabilities/default.json` restricts shell execution to the `stt-engine` sidecar only, limiting arbitrary command execution.

---

## Recommendations

1. **Fix SEC-002 first** — Ensure the CSP `connect-src` includes the local API port so the app functions correctly with defense-in-depth enabled.
2. **Address SEC-001** — Add authentication to the local API server, even if it's just a simple shared secret or Unix socket credential check.
3. **Validate SEC-003** — Add path canonicalization and prefix checking to `delete_model_file`.
4. **Document SEC-004** — Clarify how LLM API keys are injected and ensure they are not in the repo or build artifacts.
5. **Consider migrating SEC-005** — Move `localStorage` settings to Tauri's `plugin-store` for better isolation.
