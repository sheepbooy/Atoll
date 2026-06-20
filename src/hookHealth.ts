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

/** Hook was installed via Atoll but is now broken (e.g. shim missing after update). */
export function isHookDrifted(status: HookStatus | null | undefined): boolean {
  return Boolean(status?.installed && !isHookReady(status));
}

/** Agent is absent while at least one other agent remains connected (uninstall or drift). */
export function isHookDisconnected(
  status: HookStatus,
  otherAgentReady: boolean,
): boolean {
  if (isHookReady(status)) return false;
  if (isHookDrifted(status)) return true;
  return otherAgentReady;
}

export function analyzeHookHealth(
  health: HookHealthSnapshot | undefined,
): HookHealthAnalysis {
  const agentEntries: Array<{
    key: HookAgentKey;
    label: string;
    status: HookStatus | undefined;
  }> = [
    { key: "claude", label: HOOK_AGENT_LABELS.claude, status: health?.claude },
    { key: "codex", label: HOOK_AGENT_LABELS.codex, status: health?.codex },
  ];
  const agents = agentEntries.filter(
    (agent): agent is { key: HookAgentKey; label: string; status: HookStatus } =>
      agent.status != null,
  );

  const readyAgents = agents.filter((agent) => isHookReady(agent.status));
  const disconnectedAgents = agents.filter((agent) => {
    const otherAgentReady = readyAgents.some((ready) => ready.key !== agent.key);
    return isHookDisconnected(agent.status, otherAgentReady);
  });
  const connectedCount = readyAgents.length;
  const totalCount = agents.length;

  let summary = "Not connected";
  if (connectedCount > 0 && disconnectedAgents.length === 0) {
    summary = "All agents connected";
  } else if (connectedCount > 0) {
    summary = `${connectedCount} of ${totalCount} connected`;
  }

  return {
    connectedCount,
    totalCount,
    anyConnected: connectedCount > 0,
    allConnected: connectedCount > 0 && disconnectedAgents.length === 0,
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
  if (analysis.needsFirstTimeSetup) {
    return { kind: "atoll", activity: "dead" };
  }
  if (analysis.disconnectedAgents.length === 0) {
    return { kind: "atoll", activity };
  }
  if (analysis.disconnectedAgents.length >= 2) {
    return { kind: "atoll", activity: "dead" };
  }
  return {
    kind: "agent",
    agent: analysis.disconnectedAgents[0].key,
    mood: "dead",
  };
}
