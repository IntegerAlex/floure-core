import { describe, it, expect, beforeEach } from "vitest";
import { toast, subscribeToasts, dismiss, __resetToasts } from "@/lib/toast";
import { getPttMode, setPttMode } from "@/lib/ptt-mode";
import { formatHotkeyEvent } from "@/components/ShortcutRecorder";

beforeEach(() => {
  localStorage.clear();
  __resetToasts();
});

describe("stacked toasts", () => {
  it("pushes and dismisses", () => {
    let seen = 0;
    const un = subscribeToasts((t) => (seen = t.length));
    const id = toast.error("Paste failed", "wtype missing");
    expect(seen).toBe(1);
    dismiss(id);
    expect(seen).toBe(0);
    un();
  });

  it("caps the stack at 5", () => {
    for (let i = 0; i < 7; i++) toast.info(`t${i}`);
    let seen = 0;
    const un = subscribeToasts((t) => (seen = t.length));
    expect(seen).toBe(5);
    un();
  });
});

describe("ptt mode", () => {
  it("defaults to hold and round-trips toggle", () => {
    expect(getPttMode()).toBe("hold");
    setPttMode("toggle");
    expect(getPttMode()).toBe("toggle");
  });
});

describe("formatHotkeyEvent", () => {
  it("formats ctrl+shift+F12", () => {
    expect(
      formatHotkeyEvent({
        ctrlKey: true,
        metaKey: false,
        shiftKey: true,
        altKey: false,
        code: "F12",
        key: "F12",
      }),
    ).toBe("CommandOrControl+Shift+F12");
  });

  it("rejects modifier-only presses", () => {
    expect(
      formatHotkeyEvent({
        ctrlKey: true,
        metaKey: false,
        shiftKey: false,
        altKey: false,
        code: "ControlLeft",
        key: "Control",
      }),
    ).toBeNull();
  });
});
