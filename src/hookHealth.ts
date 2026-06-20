import type { AtollActivity } from "./AtollLogo";
import type { HookHealthSnapshot, HookStatus } from "./tauri";

export const HOOK_AGENT_LABELS = {
  claude: "Claude Code",
  codex: "Codex",
} as const;

export type HookAgentKey = keyof typeof HOOK_AGENT_LABELS;

export interface HookHealthAnalysis {
  connectedCount: number;
  totalCount: number;
  anyConnected: boolean;
  allConnected: boolean;
  needsFirstTimeSetup: boolean;
  needsReconnect: boolean;
  summary: string;
  disconnectedAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
}

export function isHookReady(status: HookStatus | null | undefined): boolean {
  return Boolean(status?.installed && status?.scriptFound);
}

export function analyzeHookHealth(
  health: HookHealthSnapshot | undefined,
): HookHealthAnalysis {
  const agents = (
    [
      { key: "claude" as const, label: HOOK_AGENT_LABELS.claude, status: health?.claude },
      { key: "codex" as const, label: HOOK_AGENT_LABELS.codex, status: health?.codex },
    ] satisfies Array<{ key: HookAgentKey; label: string; status: HookStatus | undefined }>
  ).filter((agent): agent is { key: HookAgentKey; label: string; status: HookStatus } =>
    Boolean(agent.status),
  );

  const readyAgents = agents.filter((agent) => isHookReady(agent.status));
  const disconnectedAgents = agents.filter((agent) => !isHookReady(agent.status));
  const connectedCount = readyAgents.length;
  const totalCount = agents.length;

  let summary = "Not connected";
  if (connectedCount === totalCount && totalCount > 0) {
    summary = "All agents connected";
  } else if (connectedCount > 0) {
    summary = `${connectedCount} of ${totalCount} connected`;
  }

  return {
    connectedCount,
    totalCount,
    anyConnected: connectedCount > 0,
    allConnected: connectedCount === totalCount && totalCount > 0,
    needsFirstTimeSetup: connectedCount === 0,
    needsReconnect: connectedCount > 0 && disconnectedAgents.length > 0,
    summary,
    disconnectedAgents,
  };
}

export function hookAttentionTitle(analysis: HookHealthAnalysis): string {
  if (analysis.needsFirstTimeSetup) {
    return "Agent hooks are not installed";
  }
  if (analysis.needsReconnect) {
    const names = analysis.disconnectedAgents.map((agent) => agent.label).join(", ");
    return `${names} hook${analysis.disconnectedAgents.length > 1 ? "s" : ""} missing or outdated — reconnect in Agent hooks`;
  }
  return "All agent hooks connected";
}

export type HeaderLogoDisplay =
  | { kind: "atoll"; activity: AtollActivity }
  | { kind: "agent"; agent: HookAgentKey; mood: "dead" };

export function deriveHeaderLogoDisplay(
  analysis: HookHealthAnalysis,
  activity: AtollActivity,
): HeaderLogoDisplay {
  if (analysis.disconnectedAgents.length === 0) {
    return { kind: "atoll", activity };
  }
  if (analysis.needsFirstTimeSetup || analysis.disconnectedAgents.length >= 2) {
    return { kind: "atoll", activity: "dead" };
  }
  return {
    kind: "agent",
    agent: analysis.disconnectedAgents[0].key,
    mood: "dead",
  };
}
