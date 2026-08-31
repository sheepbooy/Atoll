import {
  ArrowLeft,
  ExternalLink,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  type SessionHost,
} from "../tauri";
import i18n from "../i18n";
import {
  type AgentKind,
} from "../appTypes";

export interface SessionSubviewNavProps {
  cwd: string;
  agent?: AgentKind;
  sessionId?: string;
  sessionHost?: SessionHost;
  onBack: () => void;
  onOpenExternal: () => void;
}

export function sessionJumpLabel(agent?: AgentKind, sessionHost?: SessionHost): string {
  if (agent === "claude") {
    if (sessionHost === "claudeCli") return i18n.t("nav.terminal");
    return i18n.t("nav.openClaude");
  }
  if (agent === "codex") {
    if (sessionHost === "codexCli") return i18n.t("nav.terminal");
    return i18n.t("nav.openCodex");
  }
  if (agent === "cursor") {
    return i18n.t("nav.openCursor");
  }
  if (agent === "zcode") {
    if (sessionHost === "zcodeCli") return i18n.t("nav.terminal");
    return i18n.t("nav.openZcode");
  }
  return i18n.t("nav.terminal");
}

export function SessionSubviewNav({
  cwd,
  agent,
  sessionId,
  sessionHost,
  onBack,
  onOpenExternal,
}: SessionSubviewNavProps) {
  const { t } = useTranslation();

  return (
    <div className="session-detail-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>{t("nav.back")}</span>
      </button>
      <button
        type="button"
        className="open-terminal-button"
        onClick={onOpenExternal}
      >
        <ExternalLink size={13} />
        <span>{sessionJumpLabel(agent, sessionHost)}</span>
      </button>
    </div>
  );
}
