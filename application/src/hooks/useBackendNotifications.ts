// ── Backend → toast bridge ──
//
// Maps the Rust event names onto Handy-style user-facing notifications:
//   asr_error          → transcription error
//   llm_error          → transcription (cleanup) error, raw text kept
//   output_error       → paste error (typing/clipboard tool missing)
//   state:error        → recording error (mic/engine start failure)
//   model_download_*   → model state
// Every notification also lands in the ErrorBanner via addError, so the panel
// stays the durable log while toasts are the transient surface.

import { useEffect } from "react";
import { toast } from "../lib/toast";
import { isTauri } from "../lib/utils";
import type { AppError } from "../components/ErrorBanner";

type AddError = (
  category: AppError["category"],
  message: string,
  canRetry?: boolean,
  retryHint?: string,
) => void;

export function useBackendNotifications({ addError }: { addError: AddError }) {
  useEffect(() => {
    if (!isTauri()) return;
    const unlistenFns: Array<() => void> = [];
    let cancelled = false;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        if (cancelled) return;
        unlistenFns.push(
          await listen<{ error?: string }>("asr_error", (event) => {
            const msg = event.payload?.error ?? "Speech recognition failed";
            toast.error("Transcription failed", msg);
            addError("model", msg, true, "Try another ASR profile in Settings");
          }),
          await listen<{ error?: string }>("llm_error", (event) => {
            const msg = event.payload?.error ?? "Cleanup failed";
            toast.error("Cleanup failed — kept raw transcript", msg);
            addError("general", `Cleanup failed: ${msg}`);
          }),
          await listen<{ error?: string }>("output_error", (event) => {
            const msg = event.payload?.error ?? "Could not deliver transcript";
            toast.error("Paste failed", msg);
            addError("general", msg, true, "Wayland: install wtype · X11: install xdotool");
          }),
          await listen<{ id: string; error: string }>("model_download_error", (event) => {
            const { id, error } = event.payload;
            toast.error(`Model download failed: ${id}`, error);
            addError("model", `Download failed (${id}): ${error}`, true);
          }),
          await listen<{ id: string; percent: number; done?: boolean }>(
            "model_download_progress",
            (event) => {
              if (event.payload.done) {
                toast.success(`Model ready: ${event.payload.id}`);
              }
            },
          ),
        );
      } catch {
        /* not in Tauri */
      }
    })();
    return () => {
      cancelled = true;
      unlistenFns.forEach((un) => un());
    };
  }, [addError]);
}
