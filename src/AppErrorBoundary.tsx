import React from "react";
import { withTranslation, type WithTranslation } from "react-i18next";
import { quitAtoll } from "./tauri";

interface State {
  failed: boolean;
}

class AppErrorBoundaryBase extends React.Component<React.PropsWithChildren<WithTranslation>, State> {
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

    const { t } = this.props;

    return (
      <main className="atoll-recovery" role="alert">
        <strong>{t("boundary.title", { ns: "errors" })}</strong>
        <span>{t("boundary.description", { ns: "errors" })}</span>
        <div>
          <button type="button" onClick={() => window.location.reload()}>
            {t("boundary.reload", { ns: "errors" })}
          </button>
          <button type="button" onClick={() => void quitAtoll()}>
            {t("boundary.quit", { ns: "errors" })}
          </button>
        </div>
      </main>
    );
  }
}

export const AppErrorBoundary = withTranslation()(AppErrorBoundaryBase);
