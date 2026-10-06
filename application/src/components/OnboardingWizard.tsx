import { useEffect, useState } from "react";
import {
  TriangleAlert,
  Zap,
  Star,
  Check,
  X,
  ClipboardList,
  Keyboard,
  Mic,
  Copy,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useOnboarding } from "../hooks/useOnboarding";
import { MODEL_CATALOG } from "../store";
import { usePermissions } from "../hooks/usePermissions";
import { getStoredHotkey } from "../lib/settings";
import { copyToClipboard } from "../lib/clipboard";
import { isTauri } from "../lib/utils";

function StepIndicator({ step, total }: { step: number; total: number }) {
  return (
    <div className="mb-6 flex items-center justify-center gap-2">
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          className={cn(
            "h-2 w-2 rounded-full transition-colors duration-200",
            i <= step ? "bg-accent" : "border border-border bg-app-surface-secondary",
            i < step && "bg-accent/60",
          )}
        />
      ))}
    </div>
  );
}

const LOOPBACK_CMD = "curl -X POST localhost:17833/toggle";

function StepPermissions({
  clipboard,
  typing,
  onClipboard,
  onTyping,
  onNext,
}: {
  clipboard: boolean;
  typing: boolean;
  onClipboard: (v: boolean) => void;
  onTyping: (v: boolean) => void;
  onNext: () => void;
}) {
  const { permissions, requestMic } = usePermissions();
  const micGranted = permissions.microphone === "granted";
  return (
    <div className="flex flex-col items-center gap-6 text-center">
      <h2 className="text-balance text-heading text-text-primary">Permissions</h2>
      <p className="text-body text-text-secondary">
        Control where your transcribed text goes. System checks and the mic level test live in
        Settings → System & Microphone.
      </p>

      <div className="flex w-full flex-col gap-3">
        <label className="flex cursor-pointer items-center justify-between rounded-card border border-border bg-app-surface px-4 py-3">
          <div className="text-left">
            <strong className="flex items-center gap-2 text-body text-text-primary">
              <ClipboardList size={16} className="text-text-secondary" />
              Auto-copy to Clipboard
            </strong>
          </div>
          <span className="relative inline-flex h-5 w-9 items-center rounded-full border border-border bg-app-surface-secondary transition-colors">
            <input
              type="checkbox"
              checked={clipboard}
              onChange={(e) => onClipboard(e.target.checked)}
              className="peer sr-only"
            />
            <span className="ml-0.5 inline-block h-3.5 w-3.5 rounded-full bg-text-muted transition-transform peer-checked:translate-x-4 peer-checked:bg-accent" />
          </span>
        </label>

        <label className="flex cursor-pointer items-center justify-between rounded-card border border-border bg-app-surface px-4 py-3">
          <div className="text-left">
            <strong className="flex items-center gap-2 text-body text-text-primary">
              <Keyboard size={16} className="text-text-secondary" />
              Type into Focused Window
            </strong>
          </div>
          <span className="relative inline-flex h-5 w-9 items-center rounded-full border border-border bg-app-surface-secondary transition-colors">
            <input
              type="checkbox"
              checked={typing}
              onChange={(e) => onTyping(e.target.checked)}
              className="peer sr-only"
            />
            <span className="ml-0.5 inline-block h-3.5 w-3.5 rounded-full bg-text-muted transition-transform peer-checked:translate-x-4 peer-checked:bg-accent" />
          </span>
        </label>

        <div className="flex items-center justify-between rounded-card border border-border bg-app-surface px-4 py-3">
          <strong className="flex items-center gap-2 text-body text-text-primary">
            <Mic size={16} className="text-text-secondary" />
            Microphone {micGranted ? "ready" : "access"}
          </strong>
          {micGranted ? (
            <span className="inline-flex items-center gap-1 text-small text-green-600">
              <Check size={14} aria-hidden="true" /> Granted
            </span>
          ) : (
            <button
              onClick={() => void requestMic()}
              className="inline-flex h-8 items-center rounded-button bg-accent px-3 text-small font-medium text-white transition-colors hover:bg-accent-warm"
            >
              Enable mic
            </button>
          )}
        </div>
      </div>

      <div className="flex items-center justify-center">
        <button
          className={cn(
            "inline-flex h-11 items-center justify-center rounded-button px-4 py-2 text-body font-medium transition-colors duration-200",
            "bg-accent text-white shadow-accent-button hover:bg-accent-warm",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
          )}
          onClick={onNext}
        >
          Continue
        </button>
      </div>
    </div>
  );
}

function StepModelDownload({
  progress,
  onDownload,
  onDone,
}: {
  progress: Record<string, { percent: number; status: string }>;
  onDownload: (models: string[]) => void;
  onDone: () => void;
}) {
  const [selected, setSelected] = useState<Set<string>>(
    new Set(MODEL_CATALOG.filter((m) => m.recommended).map((m) => m.name)),
  );
  const [hasExisting, setHasExisting] = useState(false);
  const hasDownloads = Object.values(progress).some(
    (p) => p.status === "done" || p.status === "downloading",
  );

  // Returning installs already have models on disk — say so and let them skip.
  useEffect(() => {
    if (!isTauri()) return;
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const statuses = await invoke<Array<{ downloaded: boolean }>>("check_model_status");
        if (statuses.some((s) => s.downloaded)) setHasExisting(true);
      } catch {
        /* offline — selection UI still works */
      }
    })();
  }, []);

  const toggle = (name: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  return (
    <div className="flex flex-col items-center gap-6 text-center">
      <h2 className="text-balance text-heading text-text-primary">Download Models</h2>
      <p className="text-body text-text-secondary">
        Choose which speech recognition models to install. Smaller = faster, larger = more accurate.
      </p>
      {hasExisting && !hasDownloads && (
        <p className="w-full rounded-card border border-green-500/20 bg-green-500/10 px-4 py-2.5 text-small text-green-700">
          Models already on disk — you can continue without downloading.
        </p>
      )}

      <div className="grid w-full grid-cols-1 gap-3 sm:grid-cols-2">
        {MODEL_CATALOG.map((model) => (
          <label
            key={model.name}
            className={cn(
              "flex cursor-pointer flex-col gap-2 rounded-card border p-4 text-left transition-colors duration-200",
              selected.has(model.name)
                ? "border-[rgba(255,59,86,0.15)] bg-accent-surface"
                : "border-border bg-app-surface-card hover:border-border-hover",
            )}
          >
            <input
              type="checkbox"
              checked={selected.has(model.name)}
              onChange={() => toggle(model.name)}
              disabled={progress[model.name]?.status === "downloading"}
              className="sr-only"
            />
            <div className="flex items-center justify-between">
              <strong className="text-body text-text-primary">{model.name}</strong>
              <span
                className={cn(
                  "inline-flex items-center rounded-badge px-2 py-0.5 text-label font-semibold",
                  model.recommended
                    ? "border border-accent-muted-border bg-accent-muted text-accent-active"
                    : "border border-border bg-app-surface text-text-secondary",
                )}
              >
                {model.profile}
              </span>
            </div>
            <div className="flex items-center gap-3 text-small text-text-muted">
              <span>{model.size}</span>
              <span className="inline-flex items-center gap-1">
                <Zap size={11} aria-hidden="true" />
                {model.speed}
              </span>
              <span className="inline-flex items-center gap-1">
                <Star size={11} aria-hidden="true" />
                {model.accuracy}
              </span>
            </div>
            <p className="text-small text-text-secondary">{model.bestFor}</p>
            {progress[model.name] && (
              <div className="flex flex-col gap-1.5">
                <div className="h-1.5 overflow-hidden rounded-full bg-app-surface-secondary">
                  <div
                    className="h-full rounded-full bg-accent transition-[width] duration-150"
                    style={{ width: `${progress[model.name].percent}%` }}
                  />
                </div>
                <span className="inline-flex items-center gap-1 text-small text-text-secondary">
                  {progress[model.name].status === "done" ? (
                    <>
                      <Check size={13} aria-hidden="true" /> Done
                    </>
                  ) : (
                    `${progress[model.name].percent}%`
                  )}
                </span>
              </div>
            )}
          </label>
        ))}
      </div>

      <div className="flex items-center justify-center gap-3">
        <button
          className={cn(
            "inline-flex h-11 items-center justify-center rounded-button px-4 py-2 text-body font-medium transition-colors duration-200",
            "bg-accent text-white shadow-accent-button hover:bg-accent-warm",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
            "disabled:pointer-events-none disabled:opacity-50",
          )}
          onClick={() => onDownload(Array.from(selected))}
          disabled={selected.size === 0}
        >
          Download Selected
        </button>
        <button
          className={cn(
            "inline-flex h-11 items-center justify-center rounded-button px-4 py-2 text-body font-medium transition-colors duration-200",
            "border border-border bg-app-surface text-text-primary hover:bg-app-hover",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
          )}
          onClick={onDone}
        >
          {hasDownloads || hasExisting ? "Continue" : "Skip"}
        </button>
      </div>
    </div>
  );
}

function StepReady({ hotkey, onFinish }: { hotkey: string; onFinish: () => void }) {
  const [copied, setCopied] = useState(false);
  const isLinux =
    typeof navigator !== "undefined" && navigator.platform.toLowerCase().includes("linux");

  const copyCmd = async () => {
    setCopied(await copyToClipboard(LOOPBACK_CMD));
    window.setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="flex flex-col items-center gap-6 text-center">
      <div className="flex h-16 w-16 items-center justify-center rounded-full bg-accent/10 text-accent">
        <Mic size={30} strokeWidth={1.5} aria-hidden="true" />
      </div>
      <h2 className="text-balance text-heading text-text-primary">You&apos;re All Set!</h2>
      <p className="text-body text-text-secondary">
        Your audio never leaves this device — everything runs locally.
      </p>
      {isLinux ? (
        <div className="flex w-full flex-col gap-2 rounded-card border border-border bg-app-surface p-4 text-left">
          <p className="text-small text-text-muted">
            On Wayland, global hotkeys are compositor-dependent. The reliable trigger is the
            built-in control server — copy the command, then bind it in your desktop:
          </p>
          <div className="flex items-center gap-2">
            <code className="block w-full overflow-x-auto whitespace-nowrap rounded-[8px] bg-app-surface-secondary px-3 py-2 text-left text-[12px] text-text-primary">
              {LOOPBACK_CMD}
            </code>
            <button
              onClick={() => void copyCmd()}
              className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-button border border-border bg-app-surface-secondary px-3 text-small font-medium text-text-primary transition-colors hover:bg-app-hover"
            >
              <Copy size={12} aria-hidden="true" />
              {copied ? "Copied!" : "Copy"}
            </button>
          </div>
          <ul className="flex list-disc flex-col gap-1 pl-5 text-small text-text-secondary">
            <li>GNOME: Settings → Keyboard → Custom Shortcuts → command above</li>
            <li>KDE: System Settings → Shortcuts → Custom → command above</li>
            <li>
              Sway/i3: <code>bindsym $mod+d exec &quot;{LOOPBACK_CMD}&quot;</code>
            </li>
            <li>
              Hyprland: <code>bind = $mod, D, exec, {LOOPBACK_CMD}</code>
            </li>
          </ul>
          <p className="text-small text-text-muted">
            Or start/stop from the tray icon or the widget mic button — no keybind needed.
          </p>
        </div>
      ) : (
        <div className="flex w-full flex-col gap-2 rounded-card border border-border bg-app-surface p-4 text-left text-body text-text-secondary">
          <div>
            <kbd className="mr-2 inline-flex items-center rounded-badge border border-border bg-app-surface-secondary px-2 py-0.5 text-label font-semibold text-text-primary">
              {hotkey}
            </kbd>{" "}
            Start / Stop
          </div>
          <div>
            <kbd className="mr-2 inline-flex items-center rounded-badge border border-border bg-app-surface-secondary px-2 py-0.5 text-label font-semibold text-text-primary">
              {hotkey}
            </kbd>{" "}
            (hold) Talk, release to transcribe
          </div>
        </div>
      )}
      <div className="flex items-center justify-center">
        <button
          className={cn(
            "inline-flex h-11 items-center justify-center rounded-button px-4 py-2 text-body font-medium transition-colors duration-200",
            "bg-accent text-white shadow-accent-button hover:bg-accent-warm",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30",
          )}
          onClick={onFinish}
        >
          Start Dictating
        </button>
      </div>
    </div>
  );
}

interface Props {
  onFinished: () => void;
}

export default function OnboardingWizard({ onFinished }: Props) {
  const { state, dispatch, downloadModels, nextStep, finish } = useOnboarding(onFinished);
  const { step, clipboardEnabled, typingEnabled, modelDownloadProgress, error } = state;
  const totalSteps = 3;

  // Returning installs (flag already set, e.g. re-run after an update) skip
  // the model step — Handy re-checks permissions but never re-pushes models.
  const isReturning =
    typeof window !== "undefined" && localStorage.getItem("onboarding_completed") === "true";
  useEffect(() => {
    if (isReturning && step === 1) nextStep();
  }, [isReturning, step, nextStep]);

  return (
    <div className="animate-wizard-in flex min-h-screen flex-col items-center justify-center p-8">
      <div className="w-full max-w-lg">
        <StepIndicator step={step} total={totalSteps} />

        {error && (
          <div className="mb-4 flex items-center gap-2 rounded-card border border-red-500/20 bg-red-500/10 px-4 py-3 text-body text-red-600">
            <TriangleAlert size={16} className="shrink-0" /> {error}
            <button
              className="ml-auto flex items-center text-red-600 transition-colors hover:text-red-700"
              onClick={() => dispatch({ type: "CLEAR_ERROR" })}
              aria-label="Dismiss error"
            >
              <X size={16} />
            </button>
          </div>
        )}

        <div className="rounded-card border border-border bg-app-surface p-6">
          {/* Keyed by step so the CSS entrance animation replays on change. */}
          <div key={step} className="animate-step-in">
            {step === 0 && (
              <StepPermissions
                clipboard={clipboardEnabled}
                typing={typingEnabled}
                onClipboard={(v) => dispatch({ type: "SET_CLIPBOARD", enabled: v })}
                onTyping={(v) => dispatch({ type: "SET_TYPING", enabled: v })}
                onNext={() => nextStep()}
              />
            )}
            {step === 1 && (
              <StepModelDownload
                progress={modelDownloadProgress}
                onDownload={(models) => {
                  downloadModels(models);
                }}
                onDone={() => nextStep()}
              />
            )}
            {step >= 2 && <StepReady hotkey={getStoredHotkey()} onFinish={finish} />}
          </div>
        </div>
      </div>
    </div>
  );
}
