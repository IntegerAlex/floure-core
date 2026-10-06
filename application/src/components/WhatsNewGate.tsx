import { useState } from "react";
import { Sparkles } from "lucide-react";
import Dialog from "./Dialog";
import { APP_VERSION } from "./Footer";

const SEEN_KEY = "stt-whats-new-seen";

function seenVersion(): string | null {
  if (typeof window === "undefined") return APP_VERSION;
  return localStorage.getItem(SEEN_KEY);
}

/// One-shot release-notes gate (Handy's WhatsNewGate equivalent, stdlib-only).
/// Shows once per APP_VERSION bump on the main view; never during onboarding.
export default function WhatsNewGate() {
  const [open, setOpen] = useState(() => seenVersion() !== APP_VERSION);

  if (!open) return null;

  const dismiss = () => {
    try {
      localStorage.setItem(SEEN_KEY, APP_VERSION);
    } catch {
      /* private mode — show again next launch */
    }
    setOpen(false);
  };

  return (
    <Dialog onClose={dismiss} label="What's new" className="max-w-[440px] p-6">
      <h3 className="flex items-center gap-2 text-[16px] font-semibold text-text-primary">
        <Sparkles size={16} className="text-accent" aria-hidden="true" />
        What&apos;s new in v{APP_VERSION}
      </h3>
      <ul className="mt-3 flex list-disc flex-col gap-1.5 pl-5 text-[14px] text-text-secondary">
        <li>Stacked notifications for recording, paste, transcription and model events</li>
        <li>Shorter first-run: permissions → models → ready, with copy-ready Wayland setup</li>
        <li>Home controls: hotkey recorder, hold/toggle mode, output and sound pickers</li>
        <li>Theme choice, permission banner, and per-section error recovery</li>
      </ul>
      <div className="mt-5 flex justify-end">
        <button
          onClick={dismiss}
          autoFocus
          className="inline-flex h-9 items-center rounded-button bg-accent px-4 text-[13px] font-medium text-white transition-colors hover:bg-accent-warm"
        >
          Got it
        </button>
      </div>
    </Dialog>
  );
}
