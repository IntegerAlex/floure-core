import { useEffect, useState } from "react";
import { X, CheckCircle2, AlertTriangle, Info } from "lucide-react";
import { cn } from "@/lib/utils";
import { subscribeToasts, dismiss, type Toast } from "@/lib/toast";

const ICONS = {
  success: <CheckCircle2 size={16} className="shrink-0 text-green-600" aria-hidden="true" />,
  error: <AlertTriangle size={16} className="shrink-0 text-red-600" aria-hidden="true" />,
  info: <Info size={16} className="shrink-0 text-accent" aria-hidden="true" />,
};

/// Stacked toasts, bottom-right. Rendered once at the App root (including
/// onboarding) so backend failures during first-run — e.g. a model download
/// failing because the host is unreachable — are never silently swallowed.
export default function Toaster() {
  const [toasts, setToasts] = useState<Toast[]>([]);

  useEffect(() => subscribeToasts(setToasts), []);

  if (toasts.length === 0) return null;

  return (
    <div
      className="fixed bottom-6 right-6 z-[10000] flex w-[min(360px,calc(100vw-3rem))] flex-col gap-2"
      role="region"
      aria-label="Notifications"
    >
      {toasts.map((t) => (
        <div
          key={t.id}
          role={t.kind === "error" ? "alert" : "status"}
          className={cn(
            "animate-panel-in flex items-start gap-2.5 rounded-card border border-border bg-app-surface-solid px-4 py-3 shadow-lg",
            t.kind === "error" && "border-red-500/25",
          )}
        >
          <span className="mt-0.5">{ICONS[t.kind]}</span>
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-medium text-text-primary">{t.title}</p>
            {t.description && (
              <p className="mt-0.5 break-words text-[12px] text-text-secondary">{t.description}</p>
            )}
          </div>
          <button
            onClick={() => dismiss(t.id)}
            aria-label={`Dismiss notification: ${t.title}`}
            className="shrink-0 rounded p-0.5 text-text-muted transition-colors hover:text-text-primary"
          >
            <X size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
