import React from "react";
import { quitAtoll } from "./tauri";

interface State {
  failed: boolean;
}

export class AppErrorBoundary extends React.Component<React.PropsWithChildren, State> {
  state: State = { failed: false };

  static getDerivedStateFromError(): State {
    return { failed: true };
  }

  componentDidCatch(error: unknown) {
    console.error("[Atoll] render failure", error);
  }

  render() {
    if (!this.state.failed) {
      return this.props.children;
    }

    return (
      <main className="atoll-recovery" role="alert">
        <strong>Atoll needs to reload</strong>
        <span>The interface stopped unexpectedly. Pending approvals remain in the agent.</span>
        <div>
          <button type="button" onClick={() => window.location.reload()}>
            Reload
          </button>
          <button type="button" onClick={() => void quitAtoll()}>
            Quit
          </button>
        </div>
      </main>
    );
  }
}
