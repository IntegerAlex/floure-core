import { useState } from "react";
import { Keyboard } from "lucide-react";
import { cn } from "@/lib/utils";
import { HOTKEY_STORAGE_KEY } from "@/lib/settings";

/// Format a KeyboardEvent into a tauri global-shortcut string, e.g.
/// `CommandOrControl+Shift+F12`. Returns null for modifier-only presses —
/// the backend rejects those (`parse_hotkey` requires a main key).
export function formatHotkeyEvent(e: {
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
  code: string;
  key: string;
}): string | null {
  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");
  // Super/Win reports as metaKey on most platforms; kept under CmdOrCtrl.
  let main = e.code;
  if (main.startsWith("Key")) main = main.slice(3).toUpperCase();
  else if (main.startsWith("Digit")) main = main.slice(5);
  else if (main === "Space") main = "Space";
  else if (/^F\d+$/.test(main)) void 0;
  else return null;
  if (["Control", "Shift", "Alt", "Meta"].includes(main)) return null;
  if (["CONTROL", "SHIFT", "ALT", "META", "SUPER"].includes(main)) return null;
  parts.push(main);
  if (parts.length < 2) return null;
  return parts.join("+");
}

const PRETTY: Record<string, string> = {
  CommandOrControl: "Ctrl",
  Shift: "Shift",
  Alt: "Alt",
};

function pretty(hotkey: string): string {
  return hotkey
    .split("+")
    .map((p) => PRETTY[p] ?? p)
    .join(" + ");
}

interface Props {
  value: string;
  onChange: (next: string) => void;
}

/// Press-to-record hotkey input (Handy-style GlobalShortcutInput, stdlib-only).
/// Click → "Press keys…" → next complete chord commits. Escape cancels.
export default function ShortcutRecorder({ value, onChange }: Props) {
  const [recording, setRecording] = useState(false);

  return (
    <button
      type="button"
      aria-label={
        recording
          ? "Press keys for hotkey, Escape to cancel"
          : `Hotkey: ${value}. Activate to change.`
      }
      onClick={() => setRecording(true)}
      onKeyDown={(e) => {
        if (!recording) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") {
          setRecording(false);
          return;
        }
        const next = formatHotkeyEvent({
          ctrlKey: e.ctrlKey,
          metaKey: e.metaKey,
          shiftKey: e.shiftKey,
          altKey: e.altKey,
          code: e.code,
          key: e.key,
        });
        if (next) {
          localStorage.setItem(HOTKEY_STORAGE_KEY, next);
          onChange(next);
          setRecording(false);
        }
      }}
      onBlur={() => setRecording(false)}
      className={cn(
        "inline-flex h-9 items-center gap-2 rounded-input border px-3 text-[13px] font-medium transition-colors",
        recording
          ? "border-accent bg-accent-surface text-accent-active"
          : "border-border bg-app-surface-secondary text-text-primary hover:border-border-hover",
      )}
    >
      <Keyboard size={14} className="text-text-muted" aria-hidden="true" />
      {recording ? "Press keys…" : <kbd className="font-semibold">{pretty(value)}</kbd>}
    </button>
  );
}
