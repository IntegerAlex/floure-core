import { useEffect, useState } from "react";
import { Sun, Moon, Monitor } from "lucide-react";
import { cn } from "@/lib/utils";

export type Theme = "light" | "dark" | "system";

const THEME_KEY = "stt-theme";

export function getStoredTheme(): Theme {
  if (typeof window === "undefined") return "light";
  const v = localStorage.getItem(THEME_KEY);
  return v === "dark" || v === "system" ? v : "light";
}

/// Apply the theme as a `data-theme` hook. Surfaces stay light-styled via
/// Tailwind tokens; dark only flips the page chrome + native controls until
/// the token set is migrated to CSS vars (see globals.css).
export function applyTheme(theme: Theme) {
  if (typeof document === "undefined") return;
  const dark =
    theme === "dark" ||
    (theme === "system" &&
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

export function initTheme() {
  applyTheme(getStoredTheme());
}

const OPTIONS: { value: Theme; label: string; icon: React.ReactNode }[] = [
  { value: "light", label: "Light", icon: <Sun size={13} aria-hidden="true" /> },
  { value: "dark", label: "Dark", icon: <Moon size={13} aria-hidden="true" /> },
  { value: "system", label: "System", icon: <Monitor size={13} aria-hidden="true" /> },
];

export default function ThemeSelector({ compact = false }: { compact?: boolean }) {
  const [theme, setTheme] = useState<Theme>(() => getStoredTheme());

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  useEffect(() => {
    if (theme !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => applyTheme("system");
    mq.addEventListener?.("change", onChange);
    return () => mq.removeEventListener?.("change", onChange);
  }, [theme]);

  const pick = (t: Theme) => {
    localStorage.setItem(THEME_KEY, t);
    setTheme(t);
  };

  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className={cn("flex items-center gap-1", compact && "ml-1")}
    >
      {OPTIONS.map((o) => (
        <button
          key={o.value}
          role="radio"
          aria-checked={theme === o.value}
          title={`${o.label} theme`}
          aria-label={`${o.label} theme`}
          onClick={() => pick(o.value)}
          className={cn(
            "flex h-7 items-center gap-1 rounded-[8px] px-2 text-[12px] font-medium transition-colors",
            theme === o.value
              ? "bg-accent-surface text-accent"
              : "text-text-muted hover:bg-accent-hover-surface hover:text-text-secondary",
            compact && "px-1.5",
          )}
        >
          {o.icon}
          {!compact && o.label}
        </button>
      ))}
    </div>
  );
}
