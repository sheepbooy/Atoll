import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getApprovalRules,
  getRiskGuardEnabled,
  saveApprovalRules,
  setRiskGuardEnabled,
} from "./tauri";
import type { ApprovalRule, ApprovalRuleDecision } from "./tauri";
import { SettingsToggle } from "./SettingsControls";

// Agents a rule can be scoped to. Labels are product names, not translated.
const AGENT_OPTIONS: { key: string; label: string }[] = [
  { key: "claude", label: "Claude Code" },
  { key: "codex", label: "Codex" },
  { key: "cursor", label: "Cursor" },
  { key: "zcode", label: "ZCode" },
  { key: "gemini", label: "Gemini CLI" },
  { key: "opencode", label: "OpenCode" },
];

interface RuleDraft {
  name: string;
  decision: ApprovalRuleDecision;
  agent: string;
  tool: string;
  pattern: string;
  projectPath: string;
  note: string;
}

function emptyDraft(): RuleDraft {
  return {
    name: "",
    decision: "allow",
    agent: "",
    tool: "",
    pattern: "",
    projectPath: "",
    note: "",
  };
}

function draftFromRule(rule: ApprovalRule): RuleDraft {
  return {
    name: rule.name,
    decision: rule.decision,
    agent: rule.agent ?? "",
    tool: rule.tool ?? "",
    pattern: rule.pattern ?? "",
    projectPath: rule.projectPath ?? "",
    note: rule.note,
  };
}

function trimmed(value: string): string {
  return value.trim();
}

function newRuleId(): string {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  return `rule-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

function draftToRule(draft: RuleDraft, existing: ApprovalRule | null): ApprovalRule {
  const optional = (value: string) => {
    const text = trimmed(value);
    return text === "" ? undefined : text;
  };
  return {
    id: existing?.id ?? newRuleId(),
    name: trimmed(draft.name),
    enabled: existing?.enabled ?? true,
    decision: draft.decision,
    note: trimmed(draft.note),
    agent: optional(draft.agent),
    tool: optional(draft.tool),
    pattern: optional(draft.pattern),
    projectPath: optional(draft.projectPath),
    createdAt: existing?.createdAt ?? Math.floor(Date.now() / 1000),
    matchCount: existing?.matchCount ?? 0,
    lastMatchedAt: existing?.lastMatchedAt ?? null,
  };
}

function hasMatcher(draft: RuleDraft): boolean {
  return (
    trimmed(draft.agent) !== "" ||
    trimmed(draft.tool) !== "" ||
    trimmed(draft.pattern) !== "" ||
    trimmed(draft.projectPath) !== ""
  );
}

function agentLabel(key: string | undefined): string {
  if (!key) return "";
  return AGENT_OPTIONS.find((option) => option.key === key)?.label ?? key;
}

function formatAge(secs: number, t: (key: string, options?: Record<string, unknown>) => string): string {
  const ageMs = Date.now() - secs * 1000;
  if (!Number.isFinite(ageMs) || ageMs < 0) return t("rules.justNow");
  if (ageMs < 60_000) return t("rules.justNow");
  const hours = Math.floor(ageMs / 3_600_000);
  if (hours < 24) return t("rules.minutesAgo", { minutes: Math.floor(ageMs / 60_000) });
  const days = Math.floor(hours / 24);
  return t("rules.daysAgo", { days });
}

function RuleEditor({
  draft,
  busy,
  error,
  onDraftChange,
  onCancel,
  onSave,
}: {
  draft: RuleDraft;
  busy: boolean;
  error: string | null;
  onDraftChange: (draft: RuleDraft) => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="pricing-editor rules-editor">
      <div className="pricing-rate-grid">
        <label className="pricing-rate-field">
          <span>{t("rules.fieldName")}</span>
          <input
            type="text"
            value={draft.name}
            placeholder={t("rules.fieldNamePlaceholder")}
            onChange={(event) => onDraftChange({ ...draft, name: event.target.value })}
            data-no-drag
          />
        </label>
        <label className="pricing-rate-field">
          <span>{t("rules.fieldAgent")}</span>
          <select
            className="settings-select"
            value={draft.agent}
            onChange={(event) => onDraftChange({ ...draft, agent: event.target.value })}
            data-no-drag
          >
            <option value="">{t("rules.agentAny")}</option>
            {AGENT_OPTIONS.map((option) => (
              <option key={option.key} value={option.key}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label className="pricing-rate-field">
          <span>{t("rules.fieldTool")}</span>
          <input
            type="text"
            value={draft.tool}
            placeholder="Bash"
            onChange={(event) => onDraftChange({ ...draft, tool: event.target.value })}
            data-no-drag
          />
        </label>
        <label className="pricing-rate-field">
          <span>{t("rules.fieldPattern")}</span>
          <input
            type="text"
            value={draft.pattern}
            placeholder="Bash: npm *"
            onChange={(event) => onDraftChange({ ...draft, pattern: event.target.value })}
            data-no-drag
          />
        </label>
        <label className="pricing-rate-field">
          <span>{t("rules.fieldProject")}</span>
          <input
            type="text"
            value={draft.projectPath}
            placeholder="/Users/me/project"
            onChange={(event) => onDraftChange({ ...draft, projectPath: event.target.value })}
            data-no-drag
          />
        </label>
        <label className="pricing-rate-field">
          <span>{t("rules.fieldNote")}</span>
          <input
            type="text"
            value={draft.note}
            placeholder={t("rules.fieldNotePlaceholder")}
            onChange={(event) => onDraftChange({ ...draft, note: event.target.value })}
            data-no-drag
          />
        </label>
      </div>
      <span className="settings-card-desc">{t("rules.globHint")}</span>
      <div className="pricing-editor-actions">
        <div className="settings-segmented" role="group" aria-label={t("rules.fieldDecision")}>
          {(["allow", "deny"] as ApprovalRuleDecision[]).map((decision) => (
            <button
              key={decision}
              type="button"
              className={`settings-segment${draft.decision === decision ? " is-active" : ""}`}
              onClick={() => onDraftChange({ ...draft, decision })}
              data-no-drag
            >
              {t(decision === "allow" ? "rules.decisionAllow" : "rules.decisionDeny")}
            </button>
          ))}
        </div>
        <div className="pricing-model-meta">
          {error ? <span className="settings-shortcut-error">{error}</span> : null}
          <button
            type="button"
            className="settings-inline-button"
            disabled={busy}
            onClick={onCancel}
            data-no-drag
          >
            {t("rules.cancel")}
          </button>
          <button
            type="button"
            className="settings-inline-button is-primary"
            disabled={busy}
            onClick={onSave}
            data-no-drag
          >
            {t("rules.save")}
          </button>
        </div>
      </div>
    </div>
  );
}

export function RulesSettingsView() {
  const { t } = useTranslation("settings");
  const [rules, setRules] = useState<ApprovalRule[] | null>(null);
  const [riskGuardEnabled, setRiskGuardEnabledState] = useState(true);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [draft, setDraft] = useState<RuleDraft>(emptyDraft());
  const [draftError, setDraftError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loadError, setLoadError] = useState(false);
  const [confirmingDeleteId, setConfirmingDeleteId] = useState<string | null>(null);

  useEffect(() => {
    getApprovalRules()
      .then(setRules)
      .catch(() => setLoadError(true));
    getRiskGuardEnabled()
      .then(setRiskGuardEnabledState)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    if (confirmingDeleteId === null) return;
    const timer = window.setTimeout(() => setConfirmingDeleteId(null), 2500);
    return () => window.clearTimeout(timer);
  }, [confirmingDeleteId]);

  async function persist(next: ApprovalRule[]) {
    setBusy(true);
    try {
      const saved = await saveApprovalRules(next);
      setRules(saved);
    } finally {
      setBusy(false);
    }
  }

  function beginCreate() {
    setDraft(emptyDraft());
    setDraftError(null);
    setCreating(true);
    setEditingId(null);
  }

  function beginEdit(rule: ApprovalRule) {
    setDraft(draftFromRule(rule));
    setDraftError(null);
    setEditingId(rule.id);
    setCreating(false);
  }

  function cancelEditor() {
    setCreating(false);
    setEditingId(null);
    setDraftError(null);
  }

  async function handleSave() {
    if (!hasMatcher(draft)) {
      setDraftError(t("rules.needsMatcher"));
      return;
    }
    const existing = editingId ? rules?.find((rule) => rule.id === editingId) ?? null : null;
    const nextRule = draftToRule(draft, existing);
    const current = rules ?? [];
    // Edits stay at their list position (order = priority); creates go on top.
    const next = editingId
      ? current.map((rule) => (rule.id === nextRule.id ? nextRule : rule))
      : [nextRule, ...current];
    await persist(next);
    cancelEditor();
  }

  async function handleToggleEnabled(rule: ApprovalRule, enabled: boolean) {
    await persist((rules ?? []).map((entry) =>
      entry.id === rule.id ? { ...entry, enabled } : entry,
    ));
  }

  async function handleDelete(ruleId: string) {
    if (confirmingDeleteId !== ruleId) {
      setConfirmingDeleteId(ruleId);
      return;
    }
    setConfirmingDeleteId(null);
    await persist((rules ?? []).filter((rule) => rule.id !== ruleId));
    if (editingId === ruleId) {
      cancelEditor();
    }
  }

  async function handleMove(ruleId: string, offset: -1 | 1) {
    const list = [...(rules ?? [])];
    const index = list.findIndex((rule) => rule.id === ruleId);
    const target = index + offset;
    if (index < 0 || target < 0 || target >= list.length) return;
    [list[index], list[target]] = [list[target], list[index]];
    await persist(list);
  }

  async function handleChangeRiskGuard(enabled: boolean) {
    setRiskGuardEnabledState(enabled);
    setRiskGuardEnabled(enabled).catch(() => undefined);
  }

  const list = rules ?? [];

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.rules")}</span>
          <SettingsToggle
            label={t("rules.riskGuardLabel")}
            desc={t("rules.riskGuardDesc")}
            checked={riskGuardEnabled}
            onChange={handleChangeRiskGuard}
          />

          <p className="settings-card-desc">{t("rules.intro")}</p>

          {loadError ? (
            <span className="settings-card-desc">{t("rules.loadFailed")}</span>
          ) : null}

          <div className="pricing-model-list">
            {creating ? (
              <RuleEditor
                draft={draft}
                busy={busy}
                error={draftError}
                onDraftChange={setDraft}
                onCancel={cancelEditor}
                onSave={handleSave}
              />
            ) : null}
            {list.map((rule, index) => {
              const isEditing = editingId === rule.id;
              return (
                <div
                  key={rule.id}
                  className={`pricing-model-row${isEditing ? " is-editing" : ""}${rule.enabled ? "" : " is-disabled"}`}
                >
                  <div className="pricing-model-row-main">
                    <div className="pricing-model-copy">
                      <span className="settings-card-title">{rule.name || rule.id}</span>
                      <span className="settings-card-desc">
                        {[
                          rule.pattern ?? undefined,
                          rule.tool ? `tool: ${rule.tool}` : undefined,
                          agentLabel(rule.agent) || undefined,
                          rule.projectPath ?? undefined,
                        ]
                          .filter(Boolean)
                          .join(" · ") || t("rules.noMatchers")}
                      </span>
                      <span className="settings-card-desc">
                        {rule.matchCount > 0
                          ? t("rules.matched", {
                              count: rule.matchCount,
                              last: rule.lastMatchedAt
                                ? formatAge(rule.lastMatchedAt, t)
                                : t("rules.justNow"),
                            })
                          : t("rules.neverMatched")}
                      </span>
                    </div>
                    <div className="pricing-model-meta">
                      <span
                        className={`settings-hook-badge is-summary ${
                          rule.decision === "allow" ? "is-installed" : "is-missing"
                        }`}
                      >
                        {t(rule.decision === "allow" ? "rules.decisionAllow" : "rules.decisionDeny")}
                      </span>
                      {isEditing ? null : (
                        <>
                          <button
                            type="button"
                            className="settings-inline-button"
                            disabled={busy || index === 0}
                            onClick={() => handleMove(rule.id, -1)}
                            aria-label={t("rules.moveUp")}
                            data-no-drag
                          >
                            ↑
                          </button>
                          <button
                            type="button"
                            className="settings-inline-button"
                            disabled={busy || index === list.length - 1}
                            onClick={() => handleMove(rule.id, 1)}
                            aria-label={t("rules.moveDown")}
                            data-no-drag
                          >
                            ↓
                          </button>
                          <button
                            type="button"
                            className="settings-inline-button"
                            disabled={busy}
                            onClick={() =>
                              handleToggleEnabled(rule, !rule.enabled).catch(() => undefined)
                            }
                            data-no-drag
                          >
                            {rule.enabled ? t("rules.disable") : t("rules.enable")}
                          </button>
                          <button
                            type="button"
                            className="settings-inline-button"
                            disabled={busy}
                            onClick={() => beginEdit(rule)}
                            data-no-drag
                          >
                            {t("rules.edit")}
                          </button>
                          <button
                            type="button"
                            className={`settings-inline-button is-danger${
                              confirmingDeleteId === rule.id ? " is-confirming" : ""
                            }`}
                            disabled={busy}
                            onClick={() => handleDelete(rule.id)}
                            data-no-drag
                          >
                            {confirmingDeleteId === rule.id ? t("rules.deleteConfirm") : t("rules.delete")}
                          </button>
                        </>
                      )}
                    </div>
                  </div>
                  {isEditing ? (
                    <RuleEditor
                      draft={draft}
                      busy={busy}
                      error={draftError}
                      onDraftChange={setDraft}
                      onCancel={cancelEditor}
                      onSave={handleSave}
                    />
                  ) : null}
                </div>
              );
            })}
          </div>

          {!creating ? (
            <div className="pricing-editor-actions">
              <button
                type="button"
                className="settings-inline-button is-primary"
                disabled={busy}
                onClick={beginCreate}
                data-no-drag
              >
                {t("rules.add")}
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
