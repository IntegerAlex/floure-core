// ── Stacked toasts: minimal pub/sub store (no dependency) ──
//
// Replaces the single-string toast in App.tsx. Handy surfaces
// recording/paste/transcription/model failures as stacked sonner toasts that
// persist across onboarding steps; this is the same shape without the dep:
// any module can push, <Toaster/> renders the stack.

export interface Toast {
  id: string;
  kind: "success" | "error" | "info";
  title: string;
  description?: string;
}

type Listener = (toasts: Toast[]) => void;

let toasts: Toast[] = [];
const listeners = new Set<Listener>();
let seq = 0;

function emit() {
  const snapshot = [...toasts];
  for (const cb of listeners) cb(snapshot);
}

function push(kind: Toast["kind"], title: string, description?: string): string {
  const id = `${Date.now()}-${seq++}`;
  toasts = [...toasts.slice(-4), { id, kind, title, description }];
  emit();
  // Auto-dismiss after 5s; errors linger slightly longer.
  const delay = kind === "error" ? 6000 : 4000;
  window.setTimeout(() => dismiss(id), delay);
  return id;
}

export function dismiss(id: string) {
  if (!toasts.some((t) => t.id === id)) return;
  toasts = toasts.filter((t) => t.id !== id);
  emit();
}

export function clearToasts() {
  if (toasts.length === 0) return;
  toasts = [];
  emit();
}

export const toast = {
  success: (title: string, description?: string) => push("success", title, description),
  error: (title: string, description?: string) => push("error", title, description),
  info: (title: string, description?: string) => push("info", title, description),
};

export function subscribeToasts(cb: Listener): () => void {
  listeners.add(cb);
  cb([...toasts]);
  return () => {
    listeners.delete(cb);
  };
}

/** Test-only reset: clears stack and listeners between cases. */
export function __resetToasts() {
  toasts = [];
  listeners.clear();
  seq = 0;
}
