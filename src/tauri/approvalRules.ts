import { invoke } from "@tauri-apps/api/core";

import { isTauriRuntime } from "./runtime";
import type {
  ApprovalRule,
  ApprovalRuleDecision,
  ApprovalRuleScope,
} from "./types";

const RULE_DECISIONS: ApprovalRuleDecision[] = ["allow", "deny"];

function normalizeRule(raw: unknown): ApprovalRule {
  const value = (raw ?? {}) as Record<string, unknown>;
  const readString = (key: string, camelKey: string): string => {
    const direct = value[camelKey];
    if (typeof direct === "string") return direct;
    const snake = value[key];
    if (typeof snake === "string") return snake;
    return "";
  };
  const readOptional = (key: string, camelKey: string): string | undefined => {
    const text = readString(key, camelKey);
    return text === "" ? undefined : text;
  };
  const decision = readString("decision", "decision");
  return {
    id: readString("id", "id"),
    name: readString("name", "name"),
    enabled: value.enabled !== false,
    decision: RULE_DECISIONS.includes(decision as ApprovalRuleDecision)
      ? (decision as ApprovalRuleDecision)
      : "allow",
    note: readString("note", "note"),
    agent: readOptional("agent", "agent"),
    tool: readOptional("tool", "tool"),
    pattern: readOptional("pattern", "pattern"),
    projectPath: readOptional("project_path", "projectPath"),
    createdAt: typeof value.createdAt === "number" ? value.createdAt : 0,
    matchCount: typeof value.matchCount === "number" ? value.matchCount : 0,
    lastMatchedAt: typeof value.lastMatchedAt === "number" ? value.lastMatchedAt : null,
  };
}

export function normalizeRules(raw: unknown): ApprovalRule[] {
  if (!Array.isArray(raw)) {
    return [];
  }
  return raw.map(normalizeRule).filter((rule) => rule.id !== "");
}

export async function getApprovalRules(): Promise<ApprovalRule[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return normalizeRules(await invoke<unknown>("get_approval_rules"));
}

/** Replace the whole rule list; the backend validates and persists, and the
 * returned list is the source of truth (mirrors the pricing overrides flow). */
export async function saveApprovalRules(rules: ApprovalRule[]): Promise<ApprovalRule[]> {
  if (!isTauriRuntime()) {
    return rules;
  }
  return normalizeRules(await invoke<unknown>("save_approval_rules", { rules }));
}

export async function createApprovalRuleFromRequest(
  requestId: string,
  scope: ApprovalRuleScope,
  decision: "approved" | "denied",
): Promise<ApprovalRule[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return normalizeRules(
    await invoke<unknown>("create_approval_rule_from_request", {
      id: requestId,
      scope,
      decision,
    }),
  );
}

export async function getRiskGuardEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return true;
  }
  return invoke<boolean>("get_risk_guard_enabled");
}

export async function setRiskGuardEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_risk_guard_enabled", { enabled });
}
