import {
  useMemo,
  CSSProperties,
} from "react";
import {
  Check,
  CheckCheck,
  ChevronRight,
  FolderClosed,
  TriangleAlert,
  X,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  type PermissionRequest,
  type SessionSummary,
} from "../tauri";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  type ClawdMood,
} from "../ClawdMascot";
import {
  getSessionColor,
} from "../subagentIdentity";
import {
  type Decision,
} from "../appTypes";
import {
  agentLabels,
} from "../agents";
import {
  assessRisk,
  localizedRiskLabel,
} from "../riskAssess";
import {
  DECISION_SHORTCUTS,
} from "../platform";

export interface ApprovalCardProps {
  request: PermissionRequest;
  busyDecision: Decision | null;
  sessions: SessionSummary[];
  onApprove: () => void;
  onDeny: () => void;
  onAlwaysApprove: () => void;
  onViewSession: (sessionId: string) => void;
}

export function ApprovalCard({ request, busyDecision, sessions, onApprove, onDeny, onAlwaysApprove, onViewSession }: ApprovalCardProps) {
  const { t } = useTranslation();
  const session = sessions.find((s) => s.sessionId === request.session);
  const sessionColor = getSessionColor(request.session);
  const tone = sessionColor.tone;
  const risk = useMemo(() => assessRisk(request.command), [request.command]);
  const mascotMood: ClawdMood = risk === "danger" ? "worried" : "alert";
  const resolvingClass =
    busyDecision === "approved"
      ? " is-resolving-approve"
      : busyDecision === "denied"
        ? " is-resolving-deny"
        : "";

  return (
    <div
      className={`approval-view is-arrive ${risk ? `is-${risk}` : ""}${resolvingClass}`}
      style={
        {
          "--approval-glow": sessionColor.accent
            ? `${sessionColor.accent}59`
            : "rgba(111, 220, 255, 0.35)",
        } as CSSProperties
      }
    >
      <div className="request-main stagger-child" style={{ "--stagger-i": 0 } as CSSProperties}>
        <div className="request-kicker">
          <span className="kicker-label">
            <AgentMascot
              agent={request.agent}
              mood={mascotMood}
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={18}
            />
            {t("approval.commandRequest")}
          </span>
          <span className="kicker-tags">
            {risk ? (
              <span className={`risk-pill ${risk}`}>
                <TriangleAlert size={11} />
                {localizedRiskLabel(risk)}
              </span>
            ) : null}
            <span className={`agent-label ${tone}`}>{agentLabels[request.agent]}</span>
          </span>
        </div>
        <code className={`command-block ${risk ? `risk-${risk}` : ""}`}>{request.command}</code>
        {request.detail ? <p className="request-detail">{request.detail}</p> : null}
        <div className="cwd-line" title={request.cwd}>
          <FolderClosed size={11} />
          <span className="cwd-path">{request.cwd}</span>
        </div>
      </div>

      <div className="approval-footer stagger-child" style={{ "--stagger-i": 1 } as CSSProperties}>
        <div className={`decision-row ${request.supportsAlways ? "has-always" : ""}`}>
          <button
            className="decision-button deny"
            type="button"
            onClick={onDeny}
            disabled={busyDecision !== null}
          >
            <X size={16} />
            <span>{busyDecision === "denied" ? t("approval.denying") : t("approval.deny")}</span>
            <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.deny}</kbd>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={onApprove}
            disabled={busyDecision !== null}
          >
            <Check size={16} />
            <span>{busyDecision === "approved" ? t("approval.approving") : t("approval.approve")}</span>
            <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.approve}</kbd>
          </button>
          {request.supportsAlways ? (
            <button
              className="decision-button always-approve"
              type="button"
              onClick={onAlwaysApprove}
              disabled={busyDecision !== null}
              title={t("approval.alwaysTitle")}
            >
              <CheckCheck size={16} />
              <span>{t("approval.always")}</span>
              <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.always}</kbd>
            </button>
          ) : null}
        </div>
        {session ? (
          <button
            type="button"
            className="view-session-link"
            onClick={() => onViewSession(request.session)}
          >
            {t("approval.viewSession")}
            <ChevronRight size={12} />
          </button>
        ) : null}
      </div>
    </div>
  );
}

