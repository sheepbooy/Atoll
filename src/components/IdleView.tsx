import {
  CSSProperties,
} from "react";
import {
  Download,
  TriangleAlert,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  type HookStatus,
} from "../tauri";
import {
  type HookAgentKey,
} from "../hookHealth";

export interface IdleViewProps {
  needsHookSetup: boolean;
  needsReconnect: boolean;
  disconnectedAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
  retrustAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
  onOpenHooks: () => void;
}

export function IdleView({
  needsHookSetup,
  needsReconnect,
  disconnectedAgents,
  retrustAgents,
  onOpenHooks,
}: IdleViewProps) {
  const { t } = useTranslation();

  if (!needsHookSetup) {
    const hasDisconnected = disconnectedAgents.length > 0;
    const hasRetrust = retrustAgents.length > 0;
    const reconnectTitle =
      hasDisconnected && hasRetrust
        ? t("idle.reconnectTitleNeedAttention", {
            agents: [...disconnectedAgents, ...retrustAgents]
              .map((agent) => agent.label)
              .join(", "),
          })
        : hasDisconnected
          ? t("idle.reconnectTitleDisconnected", {
              agents: disconnectedAgents.map((agent) => agent.label).join(", "),
            })
          : t("idle.reconnectTitleRetrust", {
              agents: retrustAgents.map((agent) => agent.label).join(", "),
            });
    const reconnectDetail = hasDisconnected
      ? t("idle.reconnectDisconnectedDetail")
      : hasRetrust
        ? t("idle.reconnectRetrustDetail")
        : "";
    return (
      <div className={`idle-view${needsReconnect ? " idle-view--alert" : ""}`}>
        <div className="idle-stack">
          {needsReconnect ? (
            <div className="idle-reconnect-banner">
              <div className="idle-reconnect-icon" aria-hidden="true">
                <TriangleAlert size={14} />
              </div>
              <div className="idle-reconnect-copy">
                <strong>{reconnectTitle}</strong>
                <span>{reconnectDetail}</span>
              </div>
              <button
                type="button"
                className="install-button is-compact"
                onClick={onOpenHooks}
                data-no-drag
              >
                {t("idle.reconnect")}
              </button>
            </div>
          ) : null}
          <div className="idle-content stagger-child" style={{ "--stagger-i": 0 } as CSSProperties}>
            <span className="idle-dot" />
            <span className="idle-text">{t("idle.waiting")}</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="idle-view setup-view">
      <div className="setup-card stagger-child" style={{ "--stagger-i": 0 } as CSSProperties}>
        <div className="setup-head">
          <div className="idle-icon setup-icon">
            <Download size={16} />
          </div>
          <div className="setup-copy">
            <h2>{t("idle.setupTitle")}</h2>
            <p>{t("idle.setupDescription")}</p>
          </div>
        </div>
        <button
          type="button"
          className="install-button stagger-child"
          style={{ "--stagger-i": 1 } as CSSProperties}
          onClick={onOpenHooks}
          data-no-drag
        >
          <Download size={14} />
          <span>{t("idle.openHooks")}</span>
        </button>
      </div>
    </div>
  );
}
