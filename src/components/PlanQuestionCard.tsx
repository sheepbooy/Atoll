import {
  useMemo,
  useState,
} from "react";
import {
  Check,
  HelpCircle,
  X,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  resolvePermissionRequest,
  resolvePermissionWithInput,
  type IslandSnapshot,
  type PermissionRequest,
} from "../tauri";
import {
  getSessionColor,
} from "../subagentIdentity";
import {
  agentLabels,
} from "../agents";
import {
  parsePlanQuestions,
  getOriginalQuestions,
  OTHER_SENTINEL,
  type PlanQuestion,
  type PlanQuestionCardProps,
} from "../planMode";

export function PlanQuestionCard({ request, onResolve }: PlanQuestionCardProps) {
  const { t } = useTranslation();
  const questions = useMemo(() => parsePlanQuestions(request.toolInput), [request.toolInput]);
  const [answers, setAnswers] = useState<Record<string, string | string[]>>({});
  const [otherActive, setOtherActive] = useState<Record<string, boolean>>({});
  const [otherText, setOtherText] = useState<Record<string, string>>({});
  const [freeResponse, setFreeResponse] = useState("");
  const [useFreeResponse, setUseFreeResponse] = useState(false);
  const [busy, setBusy] = useState(false);

  function toggleOption(question: PlanQuestion, label: string) {
    const key = question.question;
    if (label === OTHER_SENTINEL) {
      setOtherActive((c) => ({ ...c, [key]: !c[key] }));
      if (otherActive[key]) {
        setOtherText((c) => ({ ...c, [key]: "" }));
        setAnswers((current) => {
          if (question.multiSelect) {
            const existing = current[key];
            const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
            return { ...current, [key]: selected.filter((item) => item !== OTHER_SENTINEL) };
          }
          const { [key]: _, ...rest } = current;
          return rest;
        });
      }
      return;
    }
    setAnswers((current) => {
      if (question.multiSelect) {
        const existing = current[key];
        const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
        const next = selected.includes(label)
          ? selected.filter((item) => item !== label)
          : [...selected, label];
        return { ...current, [key]: next };
      }
      setOtherActive((c) => ({ ...c, [key]: false }));
      setOtherText((c) => ({ ...c, [key]: "" }));
      return { ...current, [key]: label };
    });
  }

  function isOptionSelected(question: PlanQuestion, label: string) {
    if (label === OTHER_SENTINEL) return !!otherActive[question.question];
    const key = question.question;
    const value = answers[key];
    if (Array.isArray(value)) return value.includes(label);
    return value === label;
  }

  function buildUpdatedInput(): Record<string, unknown> {
    const originalQuestions = getOriginalQuestions(request.toolInput);
    if (useFreeResponse && freeResponse.trim()) {
      const text = freeResponse.trim();
      const finalAnswers: Record<string, string> = {};
      for (const entry of originalQuestions) {
        const questionText = (entry as { question?: unknown })?.question;
        if (typeof questionText === "string" && questionText) {
          finalAnswers[questionText] = text;
        }
      }
      return { questions: originalQuestions, answers: finalAnswers };
    }
    // Agents require every answer to be a plain string; multi-select choices
    // (plus "Other" text) are joined the same way the official clients do.
    const finalAnswers: Record<string, string> = {};
    for (const q of questions) {
      const key = q.question;
      if (otherActive[key] && otherText[key]?.trim()) {
        if (q.multiSelect) {
          const existing = answers[key];
          const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
          const parts = selected.filter((s) => s !== OTHER_SENTINEL);
          finalAnswers[key] = [...parts, otherText[key].trim()].join(", ");
        } else {
          finalAnswers[key] = otherText[key].trim();
        }
      } else {
        const val = answers[key];
        if (val !== undefined) {
          finalAnswers[key] = Array.isArray(val) ? val.join(", ") : val;
        }
      }
    }
    return {
      questions: originalQuestions,
      answers: finalAnswers,
    };
  }

  async function handleSubmit() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionWithInput(
        request.id,
        "approved",
        "",
        buildUpdatedInput(),
      );
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  async function handleDeny() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(request.id, "denied");
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="approval-view plan-question-view">
      <div className="request-main">
        <div className="request-kicker">
          <span className="kicker-label">
            <HelpCircle size={14} />
            {t("plan.questionsKicker")}
          </span>
          <span className={`agent-label ${getSessionColor(request.session).tone}`}>
            {agentLabels[request.agent]}
          </span>
        </div>
        {useFreeResponse ? (
          <div className="plan-questions">
            <div className="plan-question-block">
              <p className="plan-question-text">{t("plan.typeResponse")}</p>
              <textarea
                className="plan-other-input plan-free-response"
                value={freeResponse}
                onChange={(e) => setFreeResponse((e.target as HTMLTextAreaElement).value)}
                placeholder={t("plan.placeholderFreeform")}
                rows={4}
                disabled={busy}
              />
            </div>
          </div>
        ) : (
          <div className="plan-questions">
            {questions.map((question) => (
              <div className="plan-question-block" key={question.question}>
                {question.header ? <p className="plan-question-header">{question.header}</p> : null}
                <p className="plan-question-text">{question.question}</p>
                <div className="plan-options">
                  {question.options.map((option) => {
                    const selected = isOptionSelected(question, option.label);
                    return (
                      <button
                        key={option.label}
                        type="button"
                        className={`plan-option ${selected ? "selected" : ""}`}
                        onClick={() => toggleOption(question, option.label)}
                        disabled={busy}
                      >
                        <span className="plan-option-label">{option.label}</span>
                        {option.description ? (
                          <span className="plan-option-description">{option.description}</span>
                        ) : null}
                      </button>
                    );
                  })}
                  <button
                    type="button"
                    className={`plan-option plan-option-other ${isOptionSelected(question, OTHER_SENTINEL) ? "selected" : ""}`}
                    onClick={() => toggleOption(question, OTHER_SENTINEL)}
                    disabled={busy}
                  >
                    <span className="plan-option-label">{t("plan.other")}</span>
                  </button>
                </div>
                {otherActive[question.question] && (
                  <input
                    type="text"
                    className="plan-other-input"
                    placeholder={t("plan.placeholderAnswer")}
                    value={otherText[question.question] || ""}
                    onChange={(e) =>
                      setOtherText((c) => ({
                        ...c,
                        [question.question]: (e.target as HTMLInputElement).value,
                      }))
                    }
                    disabled={busy}
                  />
                )}
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="approval-footer">
        <button
          type="button"
          className="plan-toggle-free"
          onClick={() => setUseFreeResponse((v) => !v)}
          disabled={busy}
        >
          {useFreeResponse ? t("plan.backToOptions") : t("plan.replyFreely")}
        </button>
        <div className="decision-row">
          <button
            className="decision-button deny"
            type="button"
            onClick={handleDeny}
            disabled={busy}
          >
            <X size={16} />
            <span>{busy ? t("approval.denying") : t("approval.deny")}</span>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={handleSubmit}
            disabled={busy}
          >
            <Check size={16} />
            <span>{busy ? t("plan.submitting") : t("plan.submit")}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

export interface PlanApprovalCardProps {
  request: PermissionRequest;
  onResolve: (snapshot: IslandSnapshot) => void;
}
