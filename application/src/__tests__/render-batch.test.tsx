import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";

type Cb = (e: { type: string; text?: string }) => void;
let handler: Cb = () => {};

vi.mock("@/api-tauri", () => ({
  createTauriApi: () => ({
    onEvent: (cb: Cb) => {
      handler = cb;
    },
    spawn: vi.fn(async () => {}),
    kill: vi.fn(),
    start: vi.fn(),
    stop: vi.fn(),
    sendCommand: vi.fn(),
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => "ready"),
}));

import { useEngine } from "@/hooks/useEngine";

const noop = () => {};

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useEngine token batching", () => {
  it("coalesces rapid llm_token bursts into one render flush", async () => {
    const { result } = renderHook(() =>
      useEngine({
        settingsVersion: 0,
        addError: noop,
        dismissErrorsOfCategory: noop,
        notify: noop,
      }),
    );

    // Let the spawn effect run.
    await act(async () => {
      await Promise.resolve();
    });

    // One finished line to stream cleanup tokens onto.
    act(() => {
      handler({ type: "asr_final", text: "hello world" });
    });
    expect(result.current.lines).toHaveLength(1);

    // Burst of 10 tokens: nothing flushes synchronously anymore.
    act(() => {
      for (let i = 0; i < 10; i++) handler({ type: "llm_token", text: "x" });
    });
    expect(result.current.lines[0].processed).toBe("hello world");

    // Inside the batch window still nothing…
    act(() => {
      vi.advanceTimersByTime(50);
    });
    expect(result.current.lines[0].processed).toBe("hello world");

    // …then one flush applies the whole burst at once.
    act(() => {
      vi.advanceTimersByTime(100);
    });
    expect(result.current.lines[0].processed).toBe("hello worldxxxxxxxxxx");
  });

  it("flushes pending tokens on llm_end even inside the window", async () => {
    const { result } = renderHook(() =>
      useEngine({
        settingsVersion: 0,
        addError: noop,
        dismissErrorsOfCategory: noop,
        notify: noop,
      }),
    );
    await act(async () => {
      await Promise.resolve();
    });
    act(() => {
      handler({ type: "asr_final", text: "raw text" });
      handler({ type: "llm_token", text: "partial" });
      handler({ type: "llm_end", text: "Cleaned." });
    });
    expect(result.current.lines[0].processed).toBe("Cleaned.");
    expect(result.current.lines[0].status).toBe("done");
  });
});
