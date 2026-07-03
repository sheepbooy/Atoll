import type { AtollActivity } from "./AtollLogo";
import type { HookHealthSnapshot, HookStatus } from "./tauri";
import { EMPTY_HOOK_HEALTH } from "./tauri";

export const HOOK_AGENT_LABELS = {
  claude: "Claude Code",
  codex: "Codex",
  cursor: "Cursor",
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
  /** Ready agents whose hook script changed since they were last trusted — the
   * plumbing still works locally, but the agent app itself may be silently
   * ignoring the hook until the user re-confirms trust for it. */
  retrustAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
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
    status.scriptPath.includes("/target/release/") ||
    status.scriptPath.includes("\\target\\debug\\") ||
    status.scriptPath.includes("\\target\\release\\")
  ) {
    return "Hook points to a dev build path. Reinstall hooks from Atoll.app, then trust again in Codex.";
  }
  return null;
}

/** Keep the most connected hook status when overlapping snapshot loads race. */
export function preferHookStatus(
  a: HookStatus | undefined,
  b: HookStatus | undefined,
): HookStatus {
  const empty: HookStatus = {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  };
  if (!a) return b ?? empty;
  if (!b) return a;
  if (isHookReady(a)) {
    if (isHookDrifted(b)) return b;
    return a;
  }
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
  const baseCursor = base.cursor ?? EMPTY_HOOK_HEALTH.cursor;
  const upgradeCursor = upgrade.cursor ?? baseCursor;
  return {
    claude: preferHookStatus(base.claude, upgrade.claude),
    codex: preferHookStatus(base.codex, upgrade.codex),
    cursor: preferHookStatus(baseCursor, upgradeCursor),
  };
}

/** Hook was installed via Atoll but is now broken (e.g. shim missing after update). */
export function isHookDrifted(status: HookStatus | null | undefined): boolean {
  return Boolean(status?.installed && !isHookReady(status));
}

/** Agent is absent while at least one other agent remains connected (uninstall or drift).

 * Drifted hooks always count as disconnected. An agent that was never configured
 * is simply absent — only agents the user previously installed (tracked in
 * localStorage) count as disconnected when uninstalled or broken. */
export function isHookDisconnected(
  status: HookStatus,
  agentKey: HookAgentKey,
  otherAgentReady: boolean,
  configuredAgents: ReadonlySet<HookAgentKey>,
): boolean {
  if (isHookReady(status)) return false;
  if (isHookDrifted(status)) return true;
  if (!otherAgentReady) return false;
  return configuredAgents.has(agentKey);
}

export function analyzeHookHealth(
  health: HookHealthSnapshot | undefined,
  options?: { configuredAgents?: ReadonlySet<HookAgentKey> },
): HookHealthAnalysis {
  const configuredAgents = options?.configuredAgents ?? new Set<HookAgentKey>();
  const agentEntries: Array<{
    key: HookAgentKey;
    label: string;
    status: HookStatus | undefined;
  }> = [
    { key: "claude", label: HOOK_AGENT_LABELS.claude, status: health?.claude },
    { key: "codex", label: HOOK_AGENT_LABELS.codex, status: health?.codex },
    { key: "cursor", label: HOOK_AGENT_LABELS.cursor, status: health?.cursor },
  ];
  const agents = agentEntries.filter(
    (agent): agent is { key: HookAgentKey; label: string; status: HookStatus } =>
      agent.status != null,
  );

  const readyAgents = agents.filter((agent) => isHookReady(agent.status));
  const disconnectedAgents = agents.filter((agent) => {
    const otherAgentReady = readyAgents.some((ready) => ready.key !== agent.key);
    return isHookDisconnected(
      agent.status,
      agent.key,
      otherAgentReady,
      configuredAgents,
    );
  });
  const retrustAgents = readyAgents.filter((agent) => agent.status.needsRetrust);
  const connectedCount = readyAgents.length;
  const totalCount = agents.length;
  const anyHookInstalled = agents.some((agent) => agent.status.installed);
  const needsReconnect =
    connectedCount > 0 && (disconnectedAgents.length > 0 || retrustAgents.length > 0);

  let summary = "Not connected";
  if (connectedCount > 0 && disconnectedAgents.length === 0 && retrustAgents.length === 0) {
    summary = "All agents connected";
  } else if (connectedCount > 0 && disconnectedAgents.length === 0) {
    summary = `${connectedCount} of ${totalCount} connected — re-trust needed`;
  } else if (connectedCount > 0) {
    summary = `${connectedCount} of ${totalCount} connected`;
  }

  return {
    connectedCount,
    totalCount,
    anyConnected: connectedCount > 0,
    allConnected: connectedCount > 0 && disconnectedAgents.length === 0 && retrustAgents.length === 0,
    needsFirstTimeSetup: connectedCount === 0 && !anyHookInstalled,
    needsReconnect,
    summary,
    disconnectedAgents,
    retrustAgents,
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
    const disconnectedNames = analysis.disconnectedAgents.map((agent) => agent.label);
    const retrustNames = analysis.retrustAgents.map((agent) => agent.label);
    if (disconnectedNames.length > 0 && retrustNames.length === 0) {
      return `${disconnectedNames.join(", ")} hook${disconnectedNames.length > 1 ? "s" : ""} missing or outdated — reconnect in Agent hooks`;
    }
    if (disconnectedNames.length === 0 && retrustNames.length > 0) {
      return `${retrustNames.join(", ")} hook${retrustNames.length > 1 ? "s" : ""} updated by Atoll — re-trust in Agent hooks`;
    }
    const allNames = [...disconnectedNames, ...retrustNames].join(", ");
    return `${allNames} hooks need attention — check Agent hooks`;
  }
  return "All agent hooks connected";
}

export const CLAUDE_DESKTOP_HOOK_NOTE =
  "Works with Claude Code CLI and Desktop. After install: use Ask permissions in Claude Desktop, restart Claude, then trigger a Bash permission once.";

export const CODEX_DESKTOP_HOOK_NOTE =
  "Works with Codex CLI and Desktop. After install: trust the Atoll hook in Codex Desktop or via /hooks, restart Codex, then trigger one shell permission.";

export const CURSOR_HOOK_NOTE =
  "Works with Cursor IDE Agent and Ask modes. After install: confirm hooks in Cursor Settings, restart Cursor, then send a message in Agent or Ask mode to verify.";

/** Guidance shown when an agent's hook script changed after Atoll updated, so
 * the agent's previously cached trust decision is stale. */
export function hookRetrustNote(agentKey: HookAgentKey): string {
  switch (agentKey) {
    case "codex":
      return "Codex may still be using an older cached copy of the Atoll hook script. Click Reinstall hooks in Atoll, or open /hooks in Codex and re-approve the Atoll hook, then restart Codex.";
    case "claude":
      return "Atoll updated the Claude hook script since it was last approved. Reopen Claude Code / Claude Desktop permissions and re-allow the Atoll hook, then restart Claude.";
    case "cursor":
      return "Atoll updated the Cursor hook script since it was last loaded. Reopen Cursor Settings → Hooks to reload it, then restart Cursor.";
    default:
      return "Atoll updated this hook script since it was last trusted. Re-confirm it in the agent app, then restart.";
  }
}

export type HeaderLogoDisplay =
  | { kind: "atoll"; activity: AtollActivity }
  | { kind: "agent"; agent: HookAgentKey; mood: "dead" };

export function deriveHeaderLogoDisplay(
  analysis: HookHealthAnalysis,
  activity: AtollActivity,
  options?: { hookHealthKnown?: boolean },
): HeaderLogoDisplay {
  if (options?.hookHealthKnown === false) {
    return { kind: "atoll", activity: "idle" };
  }
  if (analysis.needsFirstTimeSetup) {
    return { kind: "atoll", activity: "dead" };
  }
  if (analysis.disconnectedAgents.length === 0) {
    return { kind: "atoll", activity };
  }
  // Only show a fully "dead" Atoll logo when no agent is connected at all. With
  // at least one connected agent the app is alive, so surface the single broken
  // agent instead of nuking the whole logo. This stops single-agent users from
  // seeing a false "dead" state just because other agents aren't installed.
  if (analysis.connectedCount === 0 && analysis.disconnectedAgents.length >= 2) {
    return { kind: "atoll", activity: "dead" };
  }
  return {
    kind: "agent",
    agent: analysis.disconnectedAgents[0].key,
    mood: "dead",
  };
}
