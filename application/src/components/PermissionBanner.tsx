import { memo } from "react";
import { TriangleAlert, Mic, ClipboardList } from "lucide-react";
import { usePermissions } from "@/hooks/usePermissions";

/// Persistent permission strip above the main content (Handy's
/// AccessibilityPermissions equivalent). The onboarding wizard asks once; this
/// is the always-visible reminder when mic/clipboard access is missing later.
const PermissionBanner = memo(function PermissionBanner({
  onOpenSettings,
}: {
  onOpenSettings: () => void;
}) {
  const { permissions, requestClipboard, requestMic } = usePermissions();
  const missing: string[] = [];
  if (permissions.microphone !== "granted") missing.push("microphone");
  if (permissions.clipboard !== "granted") missing.push("clipboard");
  if (missing.length === 0) return null;

  const grant = () => {
    if (permissions.microphone !== "granted") void requestMic();
    else void requestClipboard();
  };

  return (
    <div
      role="alert"
      className="mx-6 mt-4 flex flex-wrap items-center gap-3 rounded-card border border-yellow-500/25 bg-yellow-500/10 px-4 py-2.5"
    >
      <TriangleAlert size={15} className="shrink-0 text-yellow-700" aria-hidden="true" />
      <p className="min-w-0 flex-1 text-[13px] text-text-primary">
        {missing.includes("microphone") ? (
          <span className="inline-flex items-center gap-1">
            <Mic size={13} aria-hidden="true" /> Microphone access missing
          </span>
        ) : (
          <span className="inline-flex items-center gap-1">
            <ClipboardList size={13} aria-hidden="true" /> Clipboard access missing
          </span>
        )}
        <span className="text-text-secondary">
          {" "}
          — dictation output may not land where you expect.
        </span>
      </p>
      <button
        onClick={grant}
        className="inline-flex h-8 items-center rounded-button bg-accent px-3 text-[12px] font-medium text-white transition-colors hover:bg-accent-warm"
      >
        Enable
      </button>
      <button
        onClick={onOpenSettings}
        className="inline-flex h-8 items-center rounded-button border border-border bg-app-surface px-3 text-[12px] font-medium text-text-secondary transition-colors hover:bg-app-hover hover:text-text-primary"
      >
        Settings
      </button>
    </div>
  );
});

export default PermissionBanner;
