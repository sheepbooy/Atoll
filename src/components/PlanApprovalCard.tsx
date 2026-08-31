import {
  useMemo,
  useState,
  MouseEvent,
} from "react";
import {
  Hammer,
  HelpCircle,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  resolvePermissionRequest,
  openUrl,
} from "../tauri";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  getSessionColor,
} from "../subagentIdentity";
import {
  agentLabels,
} from "../agents";
import {
  parsePlanContent,
} from "../planMode";
import { type PlanApprovalCardProps } from "./PlanQuestionCard";

export function PlanApprovalCard({ request, onResolve }: PlanApprovalCardProps) {
  const { t } = useTranslation();
  const sessionColor = getSessionColor(request.session);
  const [busy, setBusy] = useState(false);
  const planContent = useMemo(() => parsePlanContent(request.toolInput), [request.toolInput]);

  function handlePlanPreviewClick(event: MouseEvent<HTMLDivElement>) {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.href) {
      event.preventDefault();
      openUrl(anchor.href);
    }
  }

  async function handleApprove() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(request.id, "approved");
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  async function handleContinuePlanning() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(
        request.id,
        "denied",
        "Continue planning",
      );
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="approval-view plan-approval-view">
      <div className="request-main">
        <div className="request-kicker">
          <span className="kicker-label">
            <AgentMascot
              agent={request.agent}
              mood="alert"
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={18}
            />
            {t("plan.readyToBuild")}
          </span>
          <span className={`agent-label ${sessionColor.tone}`}>{agentLabels[request.agent]}</span>
        </div>
        <p className="plan-approval-message">{t("plan.readyMessage")}</p>
        {planContent ? (
          <div className="plan-preview" onClick={handlePlanPreviewClick}>
            <div className="plan-preview-md">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{planContent}</ReactMarkdown>
            </div>
          </div>
        ) : null}
      </div>
      <div className="approval-footer">
        <div className="decision-row">
          <button
            className="decision-button deny"
            type="button"
            onClick={handleContinuePlanning}
            disabled={busy}
          >
            <HelpCircle size={16} />
            <span>{busy ? t("plan.sending") : t("plan.continuePlanning")}</span>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={handleApprove}
            disabled={busy}
          >
            <Hammer size={16} />
            <span>{busy ? t("approval.approving") : t("plan.agreeToBuild")}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
