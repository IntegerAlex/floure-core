import { useEffect, useRef, useState, useCallback, lazy, Suspense } from "react";
import OnboardingWizard from "./components/OnboardingWizard";
import MicPermissionModal from "./components/MicPermissionModal";
import PttOverlay from "./components/PttOverlay";
import ErrorBanner from "./components/ErrorBanner";
import type { AppError } from "./components/ErrorBanner";
import ErrorBoundary from "./components/ErrorBoundary";
import Toaster from "./components/Toaster";
import PermissionBanner from "./components/PermissionBanner";
import Footer from "./components/Footer";
import WhatsNewGate from "./components/WhatsNewGate";
// Secondary destinations load on demand so first paint only parses the Home
// path (mic + feed). Each is behind an ErrorBoundary + Suspense fallback.
// Tests import the pages directly, so laziness here changes no contract.
const HistoryPage = lazy(() => import("./components/HistoryPage"));
const SettingsPanel = lazy(() => import("./components/SettingsPanel"));
const InsightsPage = lazy(() => import("./components/InsightsPage"));
const DictionaryPage = lazy(() => import("./components/DictionaryPage"));
const ModelsPage = lazy(() => import("./components/ModelsPage"));
import { AppShell } from "./layouts/AppShell";
import { type AppView } from "./store";
import { useSettings } from "./hooks/useSettings";
import { useEngine } from "./hooks/useEngine";
import { useHistoryLog } from "./hooks/useHistoryLog";
import { useBackendNotifications } from "./hooks/useBackendNotifications";
import { FeedView, type TranscriptLine } from "./views/FeedView";
import { categoryForKind } from "./lib/errors";
import { toast } from "./lib/toast";
import { getStoredHotkey } from "./lib/settings";
import {
  getPttMode,
  setPttMode,
  isSoundEnabled,
  setSoundEnabled,
  getSoundVolume,
  setSoundVolume,
  type PttMode,
} from "./lib/ptt-mode";
import { initTheme } from "./components/ThemeSelector";

function App() {
  const { settings, setSettings, syncError } = useSettings();
  const [showErrors, setShowErrors] = useState(false);
  const [showMicModal, setShowMicModal] = useState(false);
  const [view, setView] = useState<AppView>(
    localStorage.getItem("onboarding_completed") === "true" ? "main" : "onboarding",
  );
  const [errors, setErrors] = useState<AppError[]>([]);
  const [activeItem, setActiveItem] = useState("Home");
  const [settingsVersion, setSettingsVersion] = useState(0);
  const [hotkey, setHotkey] = useState(() => getStoredHotkey());
  const [pttMode, setPttModeState] = useState<PttMode>(() => getPttMode());
  const [soundOn, setSoundOn] = useState(() => isSoundEnabled());
  const [soundVolume, setSoundVolumeState] = useState(() => getSoundVolume());

  const feedRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    initTheme();
  }, []);

  const addError = useCallback(
    (category: AppError["category"], message: string, canRetry = false, retryHint?: string) => {
      const id = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      setErrors((prev) => [
        ...prev,
        { id, category, message, canRetry, retryHint, dismissed: false },
      ]);
      setShowErrors(true);
      toast.error(message);
    },
    [],
  );

  const dismissError = useCallback((id: string) => {
    setErrors((prev) => prev.map((e) => (e.id === id ? { ...e, dismissed: true } : e)));
  }, []);

  const dismissErrorsOfCategory = useCallback((category: AppError["category"]) => {
    setErrors((prev) => prev.map((e) => (e.category === category ? { ...e, dismissed: true } : e)));
  }, []);

  const notify = useCallback((kind: "success" | "error" | "info", message: string) => {
    if (kind === "error") toast.error(message);
    else if (kind === "success") toast.success(message);
    else toast.info(message);
  }, []);

  // Handy-style backend event bridge: asr/llm/output/model failures surface as
  // stacked toasts + ErrorBanner entries (see useBackendNotifications).
  useBackendNotifications({ addError });

  const {
    connected,
    status,
    lines,
    resolvedModel,
    pttActive,
    start,
    stop,
    cancel,
    clearLines,
    connectedRef,
    statusRef,
    startRef,
    stopRef,
    cancelRef,
  } = useEngine({ settingsVersion, addError, dismissErrorsOfCategory, notify });
  void cancelRef;

  const history = useHistoryLog(feedRef, view === "main");

  useEffect(() => {
    const setVH = () => {
      const vh = window.innerHeight * 0.01;
      document.documentElement.style.setProperty("--vh", `${vh}px`);
    };
    setVH();
    window.addEventListener("resize", setVH);
    return () => window.removeEventListener("resize", setVH);
  }, []);

  // Keep the newest line visible, but only when a new utterance lands — not
  // on every streaming token edit. Resetting scrollTop per token forced a
  // sync layout on each render and stole the user's scroll position.
  const prevLineCountRef = useRef(0);
  useEffect(() => {
    if (!feedRef.current) return;
    if (lines.length !== prevLineCountRef.current) {
      prevLineCountRef.current = lines.length;
      feedRef.current.scrollTop = 0;
    }
  }, [lines]);

  useEffect(() => {
    (async () => {
      try {
        const { getCurrentWindow } = await import("@tauri-apps/api/window");
        const win = getCurrentWindow();
        const prefix =
          status === "idle"
            ? "[·]"
            : status === "listening"
              ? "[rec]"
              : status === "transcribing"
                ? "[tx]"
                : "[on]";
        await win.setTitle(`${prefix} STT — ${status}`);
      } catch {
        /* not in Tauri */
      }
      // Tray icon state (research §7 rec 1): a non-visual recording indicator
      // independent of the animated pill.
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("set_tray_state", {
          recording: status === "listening" || status === "transcribing" || status === "rewriting",
        });
      } catch {
        /* not in Tauri or no tray */
      }
    })();
  }, [status]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.repeat) return;
      if (view === "onboarding") return;
      const target = e.target as HTMLElement;
      const tag = target.tagName.toUpperCase();
      const isInteractive =
        tag === "BUTTON" ||
        target.getAttribute("role") === "button" ||
        target.closest('[contenteditable="true"]') !== null ||
        (target as HTMLInputElement).isContentEditable === true;
      if (
        e.code === "Space" &&
        tag !== "INPUT" &&
        tag !== "SELECT" &&
        tag !== "TEXTAREA" &&
        !isInteractive
      ) {
        e.preventDefault();
        if (connectedRef.current) stopRef.current();
        else startRef.current(undefined, "SpaceBar");
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [view, connectedRef, startRef, stopRef]);

  // A failed settings push leaves the engine on the previous config; surface
  // it instead of letting the change silently not apply. The banner category
  // comes from the Rust error variant, not from a hardcoded guess.
  useEffect(() => {
    if (syncError) {
      addError(
        categoryForKind(syncError.kind),
        `Settings did not reach the engine: ${syncError.message}`,
      );
    }
  }, [syncError, addError]);

  // --- Widget: emit status to widget window ---
  useEffect(() => {
    (async () => {
      try {
        const { emit } = await import("@tauri-apps/api/event");
        await emit("widget-status", status);
      } catch {
        /* not in Tauri */
      }
    })();
  }, [status]);

  // --- Widget: listen for toggle and show-main events ---
  useEffect(() => {
    let unlistenToggle: (() => void) | undefined;
    let unlistenShowMain: (() => void) | undefined;
    let unlistenReady: (() => void) | undefined;
    (async () => {
      try {
        const { listen, emit } = await import("@tauri-apps/api/event");
        unlistenToggle = await listen("widget-toggle", () => {
          if (connectedRef.current) stopRef.current();
          else startRef.current(undefined, "Widget");
        });
        unlistenShowMain = await listen("widget-show-main", async () => {
          try {
            const { getCurrentWindow } = await import("@tauri-apps/api/window");
            const win = getCurrentWindow();
            await win.unminimize();
            await win.show();
            await win.setFocus();
          } catch {
            /* not in Tauri */
          }
        });
        // Widget opened late misses earlier status broadcasts — resend on handshake.
        unlistenReady = await listen("widget-ready", async () => {
          try {
            await emit("widget-status", statusRef.current);
          } catch {
            /* not in Tauri */
          }
        });
      } catch {
        /* not in Tauri */
      }
    })();
    return () => {
      unlistenToggle?.();
      unlistenShowMain?.();
      unlistenReady?.();
    };
  }, [connectedRef, statusRef, startRef, stopRef]);

  // --- Tray actions + global push-to-talk shortcut ---
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let registeredShortcut: string | null = null;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<string>("tray-action", (event) => {
          if (event.payload === "start" && !connectedRef.current) {
            startRef.current(undefined, "Tray");
          } else if (event.payload === "stop" && connectedRef.current) {
            stopRef.current();
          }
        });
      } catch {
        /* not in Tauri */
      }
      // Register global shortcut for push-to-talk
      try {
        const { register, unregister } = await import("@tauri-apps/plugin-global-shortcut");
        // Unregister any previous shortcut first (handles StrictMode re-run)
        if (registeredShortcut) {
          try {
            await unregister(registeredShortcut);
          } catch {
            /* ok */
          }
        }
        const savedHotkey = getStoredHotkey();
        const mode = getPttMode();
        // Hold: press starts, release commits (with a 300ms re-press grace
        // window). Toggle: each press flips; releases are ignored.
        let pendingStop: number | null = null;
        await register(savedHotkey, (event) => {
          if (event.state === "Pressed") {
            if (mode === "toggle") {
              if (connectedRef.current) {
                console.log("[PTT] Toggle — stopping");
                stopRef.current();
              } else {
                console.log("[PTT] Toggle — starting");
                startRef.current(undefined, "Hotkey");
              }
              return;
            }
            if (pendingStop !== null) {
              window.clearTimeout(pendingStop);
              pendingStop = null;
              console.log("[PTT] Re-press — scheduled stop cancelled");
              return;
            }
            if (connectedRef.current) {
              console.log("[PTT] Ignored — already recording");
              return;
            }
            console.log("[PTT] Hotkey pressed — starting recording");
            // start()'s overrideSettings parameter is unused (the backend reads
            // its config from disk), so there is nothing to thread through here.
            startRef.current(undefined, "Hotkey");
          } else if (event.state === "Released") {
            if (mode === "toggle") return;
            console.log("[PTT] Hotkey released — committing text");
            if (!connectedRef.current) {
              console.log("[PTT] Not recording — nothing to commit");
              return;
            }
            // Wait briefly for in-flight transcription to complete, then stop+commit.
            // ponytail: fixed 300ms heuristic; no backend "segment fully
            // typed" signal exists to wait on instead.
            if (pendingStop !== null) window.clearTimeout(pendingStop);
            pendingStop = window.setTimeout(() => {
              pendingStop = null;
              stopRef.current();
            }, 300);
          }
        });
        registeredShortcut = savedHotkey;
        console.log(`[PTT] Global shortcut registered: ${savedHotkey} (${mode})`);
      } catch (e) {
        console.warn("[PTT] Failed to register global shortcut:", e);
        // On Wayland (and any compositor that refuses the binding) the shortcut
        // silently never fires. Point at the control server rather than leaving
        // the user with a hotkey that does nothing.
        toast.info(
          "Global hotkey unavailable — bind a compositor key to localhost:17833/toggle, or use the tray",
        );
      }
    })();
    return () => {
      unlisten?.();
      // Unregister global shortcut on cleanup
      if (registeredShortcut) {
        import("@tauri-apps/plugin-global-shortcut")
          .then(({ unregister }) => unregister(registeredShortcut!))
          .catch(() => {});
      }
    };
  }, [hotkey, pttMode, connectedRef, startRef, stopRef]);

  // --- Bare Ctrl+Win hold-to-talk (Windows native hook, backend emits) ---
  // Reuses the same start/stop refs, so overlay, sounds, widget, and guards
  // apply. Coexists with the registered shortcut; both funnel here.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<string>("ptt-hook", (event) => {
          if (event.payload === "pressed") {
            if (!connectedRef.current) startRef.current(undefined, "Hook");
          } else if (connectedRef.current) {
            stopRef.current();
          }
        });
      } catch {
        /* not in Tauri */
      }
    })();
    return () => {
      unlisten?.();
    };
  }, [connectedRef, startRef, stopRef]);

  const copyText = async (text: string, label: string) => {
    const { copyToClipboard } = await import("@/lib/clipboard");
    const ok = await copyToClipboard(text);
    if (ok) {
      toast.success(label);
    } else {
      toast.error("Copy failed");
    }
  };

  const copyLatest = async () => {
    const latest = lines[lines.length - 1];
    if (!latest) return;
    await copyText(latest.processed || latest.raw, "Copied latest!");
  };

  // Stable identity: TranscriptRow memo relies on onCopyLine not changing
  // across the per-token App re-renders during streaming.
  const copyLine = useCallback(async (line: TranscriptLine) => {
    await copyText(line.processed || line.raw, "Copied!");
  }, []);

  const handleHotkeyChange = useCallback((next: string) => {
    setHotkey(next);
    toast.success(`Hotkey set to ${next}`);
  }, []);

  const handlePttModeChange = useCallback((mode: PttMode) => {
    setPttMode(mode);
    setPttModeState(mode);
  }, []);

  const handleToggleOutput = useCallback(
    (patch: { typing?: boolean; clipboard?: boolean }) => {
      setSettings((s) => ({ ...s, ...patch }));
      setSettingsVersion((v) => v + 1);
    },
    [setSettings],
  );

  const handleSoundChange = useCallback((patch: { enabled?: boolean; volume?: number }) => {
    if (patch.enabled !== undefined) {
      setSoundEnabled(patch.enabled);
      setSoundOn(patch.enabled);
    }
    if (patch.volume !== undefined) {
      setSoundVolume(patch.volume);
      setSoundVolumeState(patch.volume);
    }
  }, []);

  const handleOnboardingComplete = () => {
    localStorage.setItem("onboarding_completed", "true");
    setView("main");
  };

  if (view === "onboarding") {
    return (
      <>
        <Toaster />
        <ErrorBanner
          errors={errors}
          onDismiss={dismissError}
          onRetry={(id) => {
            dismissError(id);
          }}
          visible={showErrors}
          onClose={() => setShowErrors(false)}
        />
        <OnboardingWizard onFinished={handleOnboardingComplete} />
      </>
    );
  }

  const handleNavigate = (item: string) => {
    setActiveItem(item);
  };

  const modelLabel = resolvedModel
    ? `${resolvedModel.model} · ${resolvedModel.device}`
    : settings.asrProfile;

  const content = (() => {
    switch (activeItem) {
      case "Config":
      case "Settings":
        return (
          <ErrorBoundary context="Settings">
            <Suspense
              fallback={<p className="p-6 text-[13px] text-text-muted">Loading settings…</p>}
            >
              <SettingsPanel
                settings={settings}
                onSave={async (s) => {
                  setSettings(s);
                  setSettingsVersion((v) => v + 1); // Trigger engine respawn with new CLI args
                  if (connectedRef.current) {
                    stopRef.current();
                  }
                }}
              />
            </Suspense>
          </ErrorBoundary>
        );
      case "Insights":
        return (
          <ErrorBoundary context="Insights">
            <Suspense fallback={<p className="p-6 text-[13px] text-text-muted">Loading…</p>}>
              <InsightsPage />
            </Suspense>
          </ErrorBoundary>
        );
      case "Dictionary":
        return (
          <ErrorBoundary context="Dictionary">
            <Suspense fallback={<p className="p-6 text-[13px] text-text-muted">Loading…</p>}>
              <DictionaryPage />
            </Suspense>
          </ErrorBoundary>
        );
      case "History":
        return (
          <ErrorBoundary context="History">
            <Suspense fallback={<p className="p-6 text-[13px] text-text-muted">Loading…</p>}>
              <HistoryPage onBack={() => setActiveItem("Home")} />
            </Suspense>
          </ErrorBoundary>
        );
      case "Models":
        return (
          <ErrorBoundary context="Models">
            <Suspense fallback={<p className="p-6 text-[13px] text-text-muted">Loading…</p>}>
              <ModelsPage />
            </Suspense>
          </ErrorBoundary>
        );
      default:
        return (
          <ErrorBoundary context="Home">
            <FeedView
              connected={connected}
              status={status}
              lines={lines}
              historyItems={history.items}
              historyLoading={history.loading}
              hasMoreHistory={history.hasMore}
              onFeedScroll={history.onScroll}
              start={start}
              stop={stop}
              cancel={cancel}
              copyLatest={copyLatest}
              copyLine={copyLine}
              clearLines={clearLines}
              feedRef={feedRef}
              errors={errors}
              showErrors={showErrors}
              setShowErrors={setShowErrors}
              dismissError={dismissError}
              onRequestMicPermission={() => setShowMicModal(true)}
              asrProfile={settings.asrProfile}
              resolvedModel={resolvedModel}
              hotkey={hotkey}
              onHotkeyChange={handleHotkeyChange}
              pttMode={pttMode}
              onPttModeChange={handlePttModeChange}
              typing={settings.typing}
              clipboard={settings.clipboard}
              onToggleOutput={handleToggleOutput}
              soundEnabled={soundOn}
              soundVolume={soundVolume}
              onSoundChange={handleSoundChange}
            />
          </ErrorBoundary>
        );
    }
  })();

  return (
    <>
      <AppShell
        activeItem={activeItem}
        onNavigate={handleNavigate}
        footer={
          <Footer
            status={status}
            modelLabel={modelLabel}
            onOpenModels={() => setActiveItem("Models")}
          />
        }
      >
        <PermissionBanner onOpenSettings={() => setActiveItem("Settings")} />
        {content}
      </AppShell>

      <Toaster />
      <WhatsNewGate />
      <MicPermissionModal
        visible={showMicModal}
        onOpenConfig={() => {
          setShowMicModal(false);
          setActiveItem("Settings");
        }}
        onClose={() => setShowMicModal(false)}
      />
      <PttOverlay visible={pttActive} />
    </>
  );
}

export default App;
