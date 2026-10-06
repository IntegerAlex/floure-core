import { memo } from "react";
import ThemeSelector from "./ThemeSelector";

export const APP_VERSION = "0.2.0";

/// Bottom chrome: version, engine state, active model. Handy keeps a fixed
/// footer with update affordance; without an updater plugin this shows the
/// local version and directs to releases for updates.
const Footer = memo(function Footer({
  status,
  modelLabel,
  onOpenModels,
}: {
  status: string;
  modelLabel: string;
  onOpenModels: () => void;
}) {
  return (
    <footer className="flex items-center gap-3 border-t border-border bg-transparent px-4 py-2 text-[12px] text-text-muted">
      <span translate="no" className="font-medium text-text-secondary">
        Floure v{APP_VERSION}
      </span>
      <span aria-hidden="true" className="text-border-hover">
        ·
      </span>
      <span role="status" aria-label={`Engine ${status}`}>
        {status}
      </span>
      <span aria-hidden="true" className="text-border-hover">
        ·
      </span>
      <button
        onClick={onOpenModels}
        className="truncate transition-colors hover:text-text-primary"
        title="Open model management"
      >
        {modelLabel}
      </button>
      <span className="ml-auto hidden sm:inline">
        Local build — see GitHub releases for updates
      </span>
      <ThemeSelector compact />
    </footer>
  );
});

export default Footer;
