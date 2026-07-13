import type { AtollActivity } from "./AtollLogo";
import i18n from "./i18n";
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
  return status.scriptFound === true;
}

export function hookStatusIssue(status: HookStatus | null | undefined): string | null {
  if (!status?.installed) return null;
  if (status.nodeFound === false) {
    return i18n.t("warning.nodeNotFound", { ns: "hooks" });
  }
  if (
    status.scriptPath.includes("/target/debug/") ||
    status.scriptPath.includes("/target/release/") ||
    status.scriptPath.includes("\\target\\debug\\") ||
    status.scriptPath.includes("\\target\\release\\")
  ) {
    return i18n.t("warning.devBuildPath", { ns: "hooks" });
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

  let summary = i18n.t("summary.notConnected", { ns: "hooks" });
  if (connectedCount > 0 && disconnectedAgents.length === 0 && retrustAgents.length === 0) {
    summary = i18n.t("summary.allConnected", { ns: "hooks" });
  } else if (connectedCount > 0 && disconnectedAgents.length === 0) {
    summary = i18n.t("summary.partialRetrust", {
      ns: "hooks",
      connected: connectedCount,
      total: totalCount,
    });
  } else if (connectedCount > 0) {
    summary = i18n.t("summary.partial", {
      ns: "hooks",
      connected: connectedCount,
      total: totalCount,
    });
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
    return i18n.t("attention.checking", { ns: "hooks" });
  }
  if (analysis.needsFirstTimeSetup) {
    return i18n.t("attention.notInstalled", { ns: "hooks" });
  }
  if (analysis.needsReconnect) {
    const disconnectedNames = analysis.disconnectedAgents.map((agent) => agent.label);
    const retrustNames = analysis.retrustAgents.map((agent) => agent.label);
    if (disconnectedNames.length > 0 && retrustNames.length === 0) {
      return i18n.t("attention.disconnected", {
        ns: "hooks",
        count: disconnectedNames.length,
        agents: disconnectedNames.join(", "),
      });
    }
    if (disconnectedNames.length === 0 && retrustNames.length > 0) {
      return i18n.t("attention.retrust", {
        ns: "hooks",
        count: retrustNames.length,
        agents: retrustNames.join(", "),
      });
    }
    const allNames = [...disconnectedNames, ...retrustNames].join(", ");
    return i18n.t("attention.mixed", { ns: "hooks", agents: allNames });
  }
  return i18n.t("attention.allConnected", { ns: "hooks" });
}

export function hookAgentNote(agentKey: HookAgentKey): string {
  return i18n.t(`note.${agentKey}`, { ns: "hooks" });
}

/** Guidance shown when an agent's hook script changed after Atoll updated, so
 * the agent's previously cached trust decision is stale. */
export function hookRetrustNote(agentKey: HookAgentKey): string {
  switch (agentKey) {
    case "codex":
      return i18n.t("retrust.codex", { ns: "hooks" });
    case "claude":
      return i18n.t("retrust.claude", { ns: "hooks" });
    case "cursor":
      return i18n.t("retrust.cursor", { ns: "hooks" });
    default:
      return i18n.t("retrust.default", { ns: "hooks" });
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
