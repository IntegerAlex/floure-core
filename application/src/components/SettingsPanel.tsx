import { useState, useEffect, useRef } from "react";
import {
  Settings,
  Bot,
  KeyRound,
  Mic,
  ShieldCheck,
  SlidersHorizontal,
  Search,
  Stethoscope,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { usePermissions } from "@/hooks/usePermissions";
import type { RuntimeSettings } from "../lib/settings";
import { getStoredHotkey, HOTKEY_STORAGE_KEY } from "../lib/settings";

interface Props {
  settings: RuntimeSettings;
  onSave: (s: RuntimeSettings) => void;
}

const HOTKEY_OPTIONS = [
  { value: "CommandOrControl+Shift+F12", label: "Ctrl + Shift + F12" },
  { value: "CommandOrControl+Shift+K", label: "Ctrl + Shift + K" },
  { value: "CommandOrControl+Shift+Space", label: "Ctrl + Shift + Space" },
  { value: "CommandOrControl+Alt+Space", label: "Ctrl + Alt + Space" },
  { value: "Alt+Space", label: "Alt + Space" },
  { value: "Super+Space", label: "Super + Space" },
];

const TOGGLES = [
  { key: "typing", label: "Type to Input", hint: "Automatically type into focused field" },
  { key: "clipboard", label: "Clipboard", hint: "Copy transcript to clipboard" },
] as const;

// Sections with search keywords. Anchors + search both drive off this — MeasuringU
// [G]: search is a precision tool for known intent; anchors cover browsing.
const SECTIONS = [
  {
    id: "speech",
    title: "Speech Recognition",
    keywords: "profile model language parakeet whisper custom vocabulary hotwords accuracy",
  },
  {
    id: "output",
    title: "Output",
    keywords: "llm mode cleanup bullet list email commit message typing clipboard type",
  },
  { id: "provider", title: "LLM Provider", keywords: "provider model local openrouter cloud" },
  { id: "api-keys", title: "API Keys", keywords: "openrouter api key secret token" },
  { id: "ptt", title: "Push-to-Talk", keywords: "hotkey shortcut keyboard binding key" },
  {
    id: "permissions",
    title: "Permissions",
    keywords: "clipboard microphone mic access permission",
  },
  {
    id: "diagnostics",
    title: "Diagnostics",
    keywords:
      "diagnostics environment debug report copy audio server os wayland pipewire microphone format",
  },
] as const;

export default function SettingsPanel({ settings, onSave }: Props) {
  const [local, setLocal] = useState<RuntimeSettings>({ ...settings });
  const [showKeys, setShowKeys] = useState(false);
  const [saved, setSaved] = useState(false);
  const [hotkey, setHotkey] = useState(() => getStoredHotkey());
  const [searchQuery, setSearchQuery] = useState("");
  const [copiedDiag, setCopiedDiag] = useState(false);
  const [copiedLoopback, setCopiedLoopback] = useState(false);
  // Wayland has no core key-grab, so a registered global shortcut often never
  // fires. The loopback control server is the reliable trigger there.
  const isLinux =
    typeof navigator !== "undefined" && navigator.platform.toLowerCase().includes("linux");
  const sectionRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const { permissions, requestClipboard, requestMic, isCapturingMic, stopMic } = usePermissions();

  useEffect(() => {
    setLocal({ ...settings });
  }, [settings]);

  // Re-read the stored hotkey whenever the page is (re)opened, so a change
  // made elsewhere is reflected rather than showing a stale binding.
  useEffect(() => {
    setHotkey(getStoredHotkey());
  }, []);

  const dirty = JSON.stringify(local) !== JSON.stringify(settings);
  const handleSave = () => {
    localStorage.setItem(HOTKEY_STORAGE_KEY, hotkey);
    onSave(local);
    setSaved(true);
    window.setTimeout(() => setSaved(false), 2000);
  };

  const update = (patch: Partial<RuntimeSettings>) => setLocal((s) => ({ ...s, ...patch }));

  // A section matches an empty query (browse mode); otherwise match title or keywords.
  const q = searchQuery.trim().toLowerCase();
  const sectionMatches = (s: (typeof SECTIONS)[number]) =>
    !q || s.title.toLowerCase().includes(q) || s.keywords.includes(q);
  const visibleSections = SECTIONS.filter(
    (s) => sectionMatches(s) && !(s.id === "api-keys" && local.llmProvider === "local"),
  );

  // Anchors: scroll the matched section into view inside the scrollable body.
  const scrollToSection = (id: string) => {
    const el = sectionRefs.current[id];
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const inputClass = cn(
    "w-full rounded-input bg-app-surface-secondary border border-border px-3 py-2 text-body text-text-primary",
    "placeholder:text-text-muted focus:outline-none focus:ring-2 focus:ring-accent/30 transition-colors",
  );

  const checkRow = "flex items-center justify-between gap-3";
  const checkLabel = "text-body text-text-primary";
  const checkHint = "text-small text-text-muted";

  return (
    <div className="flex flex-1 flex-col overflow-hidden p-6">
      {/* Page header: same shape as History/Models, so Settings reads as a
          destination rather than an overlay. */}
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <h2 className="flex items-center gap-2 text-balance text-[32px] font-semibold text-text-primary">
            <Settings size={26} className="text-text-secondary" />
            Settings
          </h2>
          {dirty && (
            <span className="rounded-badge border border-accent-muted-border bg-accent-muted px-2 py-0.5 text-[11px] font-semibold text-accent-active">
              Unsaved changes
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {dirty && (
            <button
              onClick={() => setLocal({ ...settings })}
              className="inline-flex h-9 items-center rounded-button border border-border bg-app-surface px-4 text-small font-medium text-text-secondary transition-colors hover:bg-app-hover hover:text-text-primary"
            >
              Discard
            </button>
          )}
          <button
            onClick={handleSave}
            disabled={!dirty}
            className="inline-flex h-9 items-center rounded-button bg-accent px-4 text-small font-medium text-white shadow-accent-button transition-colors hover:bg-accent-warm disabled:pointer-events-none disabled:opacity-50"
          >
            {saved ? "Saved" : "Save & Apply"}
          </button>
        </div>
      </div>
      {/* Search + section anchors */}
      <div className="mb-3 flex items-center gap-2">
        <Search size={15} className="shrink-0 text-text-muted" />
        <input
          type="text"
          name="settings-search"
          autoComplete="off"
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          placeholder="Search settings…"
          aria-label="Search settings"
          className="h-9 flex-1 rounded-input border border-border bg-app-surface-secondary px-3 text-body text-text-primary placeholder:text-text-muted focus:border-accent focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
        />
        {searchQuery && (
          <button
            onClick={() => setSearchQuery("")}
            className="text-small text-text-muted hover:text-text-primary"
          >
            Clear
          </button>
        )}
      </div>
      <div className="mb-4 flex flex-wrap gap-1.5">
        {visibleSections.map((s) => (
          <button
            key={s.id}
            onClick={() => scrollToSection(s.id)}
            className="h-7 rounded-full border border-border bg-app-surface-secondary px-3 text-[12px] font-medium text-text-secondary transition-colors hover:border-accent hover:text-text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent/30"
          >
            {s.title}
          </button>
        ))}
        {visibleSections.length === 0 && (
          <p className="text-small text-text-muted">No settings match “{searchQuery}”.</p>
        )}
      </div>
      <div className="flex flex-1 flex-col gap-6 overflow-y-auto overscroll-contain pr-1">
        <div
          ref={(el) => {
            sectionRefs.current.speech = el;
          }}
          style={{ display: sectionMatches(SECTIONS[0]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <Mic size={15} className="text-text-secondary" />
            Speech Recognition
          </h3>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-asr-profile" className="text-label text-text-secondary">
              Profile
            </label>
            <select
              id="settings-asr-profile"
              className={inputClass}
              value={local.asrProfile}
              onChange={(e) =>
                update({ asrProfile: e.target.value as RuntimeSettings["asrProfile"] })
              }
            >
              <option value="parakeet">Parakeet TDT (English)</option>
              <option value="whisper-turbo">Whisper large-v3-turbo (Multilingual)</option>
              <option value="whisper-base">Whisper base (Lightweight)</option>
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-language" className="text-label text-text-secondary">
              Language
            </label>
            <select
              id="settings-language"
              className={inputClass}
              value={local.language}
              onChange={(e) => update({ language: e.target.value })}
            >
              <option value="">Auto-detect</option>
              <option value="en">English</option>
              <option value="hi">Hindi</option>
              <option value="es">Spanish</option>
              <option value="fr">French</option>
              <option value="de">German</option>
              <option value="pt">Portuguese</option>
              <option value="ja">Japanese</option>
              <option value="ko">Korean</option>
              <option value="zh">Chinese</option>
              <option value="ar">Arabic</option>
              <option value="ru">Russian</option>
            </select>
          </div>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-hotwords" className="text-label text-text-secondary">
              Custom Vocabulary
            </label>
            <input
              id="settings-hotwords"
              name="hotwords"
              autoComplete="off"
              className={inputClass}
              value={local.hotwords}
              onChange={(e) => update({ hotwords: e.target.value })}
              placeholder="e.g. WhisperFlow, Tauri, PyTorch"
            />
            <p className="text-small text-text-muted">
              Comma-separated words to boost recognition accuracy. Applies to the Parakeet profile;
              Whisper ignores it.
            </p>
          </div>
        </div>

        <div
          ref={(el) => {
            sectionRefs.current.output = el;
          }}
          style={{ display: sectionMatches(SECTIONS[1]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <SlidersHorizontal size={15} className="text-text-secondary" />
            Output
          </h3>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-llm-mode" className="text-label text-text-secondary">
              LLM Mode
            </label>
            <select
              id="settings-llm-mode"
              className={inputClass}
              value={local.llmMode}
              onChange={(e) => update({ llmMode: e.target.value as RuntimeSettings["llmMode"] })}
            >
              <option value="off">Off</option>
              <option value="cleanup">Cleanup</option>
              <option value="bullet_list">Bullet List</option>
              <option value="email">Email</option>
              <option value="commit_message">Commit Message</option>
            </select>
          </div>
          {TOGGLES.map((t) => (
            <label key={t.key} className={checkRow}>
              <span>
                <span className={checkLabel}>{t.label}</span>
                <span className={checkHint}> — {t.hint}</span>
              </span>
              <input
                type="checkbox"
                checked={local[t.key]}
                onChange={(e) => update({ [t.key]: e.target.checked })}
                aria-label={t.label}
                className="h-5 w-5 accent"
              />
            </label>
          ))}
        </div>

        <div
          ref={(el) => {
            sectionRefs.current.provider = el;
          }}
          style={{ display: sectionMatches(SECTIONS[2]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <Bot size={15} className="text-text-secondary" />
            LLM Provider
          </h3>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-provider" className="text-label text-text-secondary">
              Provider
            </label>
            <select
              id="settings-provider"
              className={inputClass}
              value={local.llmProvider}
              onChange={(e) =>
                update({ llmProvider: e.target.value as RuntimeSettings["llmProvider"] })
              }
            >
              <option value="local">Local</option>
              <option value="openrouter">OpenRouter</option>
            </select>
          </div>
          {local.llmProvider === "local" ? (
            <div className="flex flex-col gap-1.5">
              <label htmlFor="settings-local-model" className="text-label text-text-secondary">
                Model
              </label>
              <select
                id="settings-local-model"
                className={inputClass}
                value={local.llmModel || "s1-mini-q4_k_m"}
                onChange={(e) => update({ llmModel: e.target.value })}
              >
                <option value="s1-mini-q4_k_m">S1-Mini (462 MB)</option>
                <option value="gemma-3-1b-it-q4_k_m">Gemma 3 1B (806 MB)</option>
              </select>
            </div>
          ) : (
            <div className="flex flex-col gap-1.5">
              <label htmlFor="settings-model" className="text-label text-text-secondary">
                Model
              </label>
              <input
                id="settings-model"
                name="llm-model"
                autoComplete="off"
                spellCheck={false}
                className={inputClass}
                value={local.llmModel}
                onChange={(e) => update({ llmModel: e.target.value })}
                placeholder="openai/gpt-4o-mini"
              />
            </div>
          )}
        </div>

        {local.llmProvider !== "local" && (
          <div
            ref={(el) => {
              sectionRefs.current["api-keys"] = el;
            }}
            style={{ display: sectionMatches(SECTIONS[3]) ? undefined : "none" }}
            className="flex flex-col gap-3"
          >
            <h3 className="flex items-center gap-2 text-subheading text-text-primary">
              <KeyRound size={15} className="text-text-secondary" />
              API Keys
            </h3>
            <p className="text-small text-text-muted">
              Keys stay in memory only and are never written to disk — re-enter them after a
              restart.
            </p>
            <div className="flex flex-col gap-1.5">
              <label htmlFor="settings-openrouter-key" className="text-label text-text-secondary">
                OpenRouter API Key
              </label>
              <input
                id="settings-openrouter-key"
                name="openrouter-api-key"
                spellCheck={false}
                className={cn(inputClass, "font-mono")}
                type={showKeys ? "text" : "password"}
                value={local.openrouterApiKey}
                onChange={(e) => update({ openrouterApiKey: e.target.value })}
                placeholder={local.openrouterApiKey ? "••••••••" : "sk-or-…"}
                autoComplete="off"
              />
            </div>
            <label className="flex cursor-pointer items-center gap-2 text-body text-text-secondary">
              <span className="relative inline-flex h-5 w-9 items-center rounded-full border border-border bg-app-surface-secondary transition-colors">
                <input
                  type="checkbox"
                  checked={showKeys}
                  onChange={(e) => setShowKeys(e.target.checked)}
                  aria-label="Show API keys"
                  className="peer sr-only"
                />
                <span className="ml-0.5 inline-block h-3.5 w-3.5 rounded-full bg-text-muted transition-transform peer-checked:translate-x-4 peer-checked:bg-accent" />
              </span>
              Show keys
            </label>
          </div>
        )}

        <div
          ref={(el) => {
            sectionRefs.current.ptt = el;
          }}
          style={{ display: sectionMatches(SECTIONS[4]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <Mic size={15} className="text-text-secondary" />
            Push-to-Talk
          </h3>
          <div className="flex flex-col gap-1.5">
            <label htmlFor="settings-hotkey" className="text-label text-text-secondary">
              Push-to-Talk Hotkey
            </label>
            <select
              id="settings-hotkey"
              className={inputClass}
              value={hotkey}
              onChange={(e) => setHotkey(e.target.value)}
            >
              {HOTKEY_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
            <p className="text-small text-text-muted">Hold to record, release to commit text.</p>
          </div>
          {isLinux && (
            <div className="flex flex-col gap-2 rounded-card border border-border bg-app-surface-secondary p-3">
              <p className="text-small text-text-secondary">
                On Wayland a compositor may ignore global hotkeys entirely. Floure runs a local
                control server that always works — bind a compositor key to it:
              </p>
              <div className="flex items-center gap-2">
                <code className="flex-1 overflow-x-auto whitespace-nowrap rounded-[6px] bg-app-surface px-2 py-1.5 text-[12px] text-text-primary">
                  curl -X POST localhost:17833/toggle
                </code>
                <button
                  onClick={async () => {
                    const { copyToClipboard } = await import("@/lib/clipboard");
                    setCopiedLoopback(await copyToClipboard("curl -X POST localhost:17833/toggle"));
                  }}
                  className="inline-flex h-8 shrink-0 items-center rounded-button border border-border bg-app-surface px-3 text-small font-medium text-text-primary transition-colors hover:bg-app-hover"
                >
                  {copiedLoopback ? "Copied!" : "Copy"}
                </button>
              </div>
              <p className="text-small text-text-muted">
                Sway/Hyprland:{" "}
                <code className="rounded-[4px] bg-app-surface px-1.5 py-0.5">
                  bindsym $mod+d exec "curl -X POST localhost:17833/toggle"
                </code>
              </p>
            </div>
          )}
        </div>

        <div
          ref={(el) => {
            sectionRefs.current.permissions = el;
          }}
          style={{ display: sectionMatches(SECTIONS[5]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <ShieldCheck size={15} className="text-text-secondary" />
            Permissions
          </h3>
          <label className={checkRow}>
            <span>
              <span className={checkLabel}>Clipboard</span>
              <span className={checkHint}> — copy transcripts to clipboard</span>
            </span>
            <input
              type="checkbox"
              checked={permissions.clipboard === "granted"}
              onChange={(e) => {
                if (e.target.checked) void requestClipboard();
              }}
              aria-label="Clipboard permission"
              className="h-5 w-5 accent"
            />
          </label>
          <div className={checkRow}>
            <span>
              <span className={checkLabel}>Microphone</span>
              <span className={checkHint}> — capture audio for recognition</span>
            </span>
            <span className="flex items-center gap-2">
              <input
                type="checkbox"
                checked={permissions.microphone === "granted"}
                onChange={(e) => {
                  if (e.target.checked) void requestMic();
                  else if (isCapturingMic) stopMic();
                }}
                aria-label="Microphone permission"
                className="h-5 w-5 accent"
              />
              {permissions.microphone === "granted" && (
                <button
                  onClick={isCapturingMic ? stopMic : () => void requestMic()}
                  className="h-[26px] rounded-[6px] border border-border bg-app-surface-secondary px-2.5 text-[11px] font-medium text-text-secondary transition-colors hover:bg-app-hover"
                >
                  {isCapturingMic ? "Stop" : "Test"}
                </button>
              )}
            </span>
          </div>
        </div>
        <div
          ref={(el) => {
            sectionRefs.current.diagnostics = el;
          }}
          style={{ display: sectionMatches(SECTIONS[6]) ? undefined : "none" }}
          className="flex flex-col gap-3"
        >
          <h3 className="flex items-center gap-2 text-subheading text-text-primary">
            <Stethoscope size={15} className="text-text-secondary" />
            Diagnostics
          </h3>
          <p className="text-small text-text-muted">
            A local record of what Floure ran against — OS, display and audio server, and the
            microphone format it negotiated. It is never sent anywhere; copying it is your choice.
          </p>
          <div>
            <button
              onClick={async () => {
                try {
                  const { invoke } = await import("@tauri-apps/api/core");
                  const text = await invoke<string>("get_diagnostics");
                  const { copyToClipboard } = await import("@/lib/clipboard");
                  setCopiedDiag(await copyToClipboard(text));
                } catch {
                  setCopiedDiag(false);
                }
              }}
              className="inline-flex h-9 items-center gap-1.5 rounded-button border border-border bg-app-surface px-3 text-small font-medium text-text-primary transition-colors hover:bg-app-hover"
            >
              {copiedDiag ? "Copied!" : "Copy diagnostics"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
