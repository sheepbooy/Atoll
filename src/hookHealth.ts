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
  if (!status?.installed) return false;
  if (status.nodeFound === false) return false;
  return Boolean(status.scriptFound || status.scriptPath);
}

export function hookStatusIssue(status: HookStatus | null | undefined): string | null {
  if (!status?.installed) return null;
  if (status.nodeFound === false) {
    return "Node.js not found at the configured hook path. Install Node.js, then reinstall hooks.";
  }
  if (
    status.scriptPath.includes("/target/debug/") ||
    status.scriptPath.includes("/target/release/")
  ) {
    return "Hook points to a dev build path. Reinstall hooks from Atoll.app, then trust again in Codex.";
  }
  return null;
}

/** Keep the most connected hook status when overlapping snapshot loads race. */
export function preferHookStatus(a: HookStatus, b: HookStatus): HookStatus {
  if (isHookReady(a)) return a;
  if (isHookReady(b)) return b;
  return {
    installed: a.installed || b.installed,
    scriptFound: a.scriptFound || b.scriptFound,
    settingsPath: b.settingsPath || a.settingsPath,
    scriptPath: b.scriptPath || a.scriptPath,
    nodePath: b.nodePath || a.nodePath,
    nodeFound: a.nodeFound !== false && b.nodeFound !== false,
  };
}

export function mergeHookHealthPreferReady(
  base: HookHealthSnapshot,
  upgrade: HookHealthSnapshot,
): HookHealthSnapshot {
  return {
    claude: preferHookStatus(base.claude, upgrade.claude),
    codex: preferHookStatus(base.codex, upgrade.codex),
  };
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
  const anyHookInstalled = agents.some((agent) => agent.status.installed);

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
    needsFirstTimeSetup: connectedCount === 0 && !anyHookInstalled,
    needsReconnect: connectedCount > 0 && disconnectedAgents.length > 0,
    summary,
    disconnectedAgents,
  };
}

export function hookAttentionTitle(
  analysis: HookHealthAnalysis,
  hookHealthKnown = true,
): string {
  if (!hookHealthKnown) {
    return "Checking agent hooks";
  }
  if (analysis.needsFirstTimeSetup) {
    return "Agent hooks are not installed";
  }
  if (analysis.needsReconnect) {
    const names = analysis.disconnectedAgents.map((agent) => agent.label).join(", ");
    return `${names} hook${analysis.disconnectedAgents.length > 1 ? "s" : ""} missing or outdated — reconnect in Agent hooks`;
  }
  return "All agent hooks connected";
}

export const CLAUDE_DESKTOP_HOOK_NOTE =
  "Works with Claude Code CLI and Desktop. After install: use Ask permissions in Claude Desktop, restart Claude, then trigger a Bash permission once.";

export const CODEX_DESKTOP_HOOK_NOTE =
  "Works with Codex CLI and Desktop. After install: trust the Atoll hook in Codex Desktop or via /hooks, restart Codex, then trigger one shell permission.";

export type HeaderLogoDisplay =
  | { kind: "atoll"; activity: AtollActivity }
  | { kind: "agent"; agent: HookAgentKey; mood: "dead" };

export function deriveHeaderLogoDisplay(
  analysis: HookHealthAnalysis,
  activity: AtollActivity,
  options?: { hookHealthKnown?: boolean },
): HeaderLogoDisplay {
  if (options?.hookHealthKnown === false) {
    return { kind: "atoll", activity };
  }
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
