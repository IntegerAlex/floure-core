import { Component, type ReactNode } from "react";

interface Props {
  context: string;
  children: ReactNode;
}

interface State {
  failed: boolean;
  message: string;
}

/// Per-section boundary so one crashing page (Insights, Dictionary, …) shows
/// a card instead of blanking the whole shell. Mounted around each destination
/// in App.tsx, mirroring Handy's per-area ErrorBoundary usage.
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { failed: false, message: "" };

  static getDerivedStateFromError(e: unknown): State {
    return { failed: true, message: e instanceof Error ? e.message : String(e) };
  }

  componentDidCatch() {
    /* rendered as fallback UI; console keeps the stack */
  }

  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <div
        className="m-6 rounded-card border border-red-500/20 bg-red-500/10 p-6"
        role="alert"
        aria-label={`${this.props.context} failed to render`}
      >
        <p className="text-[15px] font-semibold text-text-primary">
          {this.props.context} ran into a problem
        </p>
        <p className="mt-1 text-[13px] text-text-secondary">
          {this.state.message || "Unexpected render error."} Switching sections and coming back
          usually recovers.
        </p>
        <button
          className="mt-3 inline-flex h-9 items-center rounded-button border border-border bg-app-surface px-4 text-[13px] font-medium text-text-primary transition-colors hover:bg-app-hover"
          onClick={() => this.setState({ failed: false, message: "" })}
        >
          Try again
        </button>
      </div>
    );
  }
}
