import { describe, expect, it } from "vitest";
import {
  analyzeHookHealth,
  deriveHeaderLogoDisplay,
  hookAttentionTitle,
  hookRetrustNote,
  hookStatusIssue,
  isHookDisconnected,
  isHookDrifted,
  isHookReady,
  mergeHookHealthPreferReady,
  preferHookStatus,
  type HookAgentKey,
} from "./hookHealth";
import type { HookHealthSnapshot } from "./tauri";

const ready = {
  installed: true,
  scriptFound: true,
  settingsPath: "/tmp/settings.json",
  scriptPath: "/tmp/atoll-claude-hook.mjs",
};

const missing = {
  installed: false,
  scriptFound: true,
  settingsPath: "/tmp/settings.json",
  scriptPath: "/tmp/atoll-claude-hook.mjs",
};

const drifted = {
  installed: true,
  scriptFound: false,
  settingsPath: "/tmp/settings.json",
  scriptPath: "",
};

function hookHealth(
  claude: typeof ready,
  codex: typeof ready,
  cursor: typeof ready = ready,
): HookHealthSnapshot {
  return { claude, codex, cursor };
}

describe("hookHealth", () => {
  it("detects first-time setup when no agents are connected", () => {
    const analysis = analyzeHookHealth(hookHealth(missing, missing, missing));

    expect(analysis.needsFirstTimeSetup).toBe(true);
    expect(analysis.needsReconnect).toBe(false);
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.summary).toBe("Not connected");
  });

  it("does not flag an uninstalled agent as disconnected when others stay connected", () => {
    const analysis = analyzeHookHealth(hookHealth(missing, ready, ready));

    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(false);
    expect(analysis.connectedCount).toBe(2);
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.summary).toBe("All agents connected");
  });

  it("detects partial drift when one installed agent loses its shim", () => {
    const analysis = analyzeHookHealth(hookHealth(drifted, ready, ready));

    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(true);
    expect(analysis.connectedCount).toBe(2);
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["claude"]);
    expect(hookAttentionTitle(analysis)).toContain("Claude Code");
  });

  it("treats missing node executable as not ready", () => {
    const nodeMissing = {
      ...ready,
      nodeFound: false,
      nodePath: "/missing/node",
    };
    expect(isHookReady(nodeMissing)).toBe(false);
    expect(hookStatusIssue(nodeMissing)).toContain("Node.js not found");
  });

  it("warns when hook script points at a dev build path", () => {
    const devPath = {
      ...ready,
      scriptPath: "/Users/test/code/Atoll/src-tauri/target/debug/scripts/atoll-codex-hook.mjs",
    };
    expect(hookStatusIssue(devPath)).toContain("dev build path");
  });

  it("treats missing scripts as drift only when hooks were installed", () => {
    const emptyConfigured = new Set<HookAgentKey>();
    const configuredClaude = new Set<HookAgentKey>(["claude"]);
    expect(isHookReady(drifted)).toBe(false);
    expect(isHookDrifted(drifted)).toBe(true);
    expect(isHookDrifted(missing)).toBe(false);
    // Never-installed agents are absent, not disconnected.
    expect(isHookDisconnected(missing, "claude", true, emptyConfigured)).toBe(false);
    // Configured but uninstalled agents are disconnected when others stay live.
    expect(isHookDisconnected(missing, "claude", true, configuredClaude)).toBe(true);
    // Drifted (installed but broken) agents are disconnected.
    expect(isHookDisconnected(drifted, "claude", true, emptyConfigured)).toBe(true);
  });

  it("treats installed hooks with a script path as ready even without scriptFound", () => {
    const pathOnly = {
      installed: true,
      scriptFound: false,
      settingsPath: "/tmp/settings.json",
      scriptPath: "/Applications/Atoll.app/Contents/Resources/scripts/atoll-claude-hook.mjs",
    };
    expect(isHookReady(pathOnly)).toBe(true);
    const analysis = analyzeHookHealth(hookHealth(pathOnly, ready, ready));
    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.allConnected).toBe(true);
  });

  it("does not treat first-time setup when hooks are installed but not ready", () => {
    const analysis = analyzeHookHealth(hookHealth(drifted, drifted, drifted));
    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(false);
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "dead",
    });
  });

  it("prefers a ready hook status when snapshot loads race", () => {
    const merged = mergeHookHealthPreferReady(
      hookHealth(drifted, missing, missing),
      hookHealth(ready, ready, ready),
    );
    expect(merged.claude).toEqual(ready);
    expect(merged.codex).toEqual(ready);
    expect(preferHookStatus(drifted, ready)).toEqual(ready);
    expect(preferHookStatus(ready, drifted)).toEqual(drifted);
  });

  it("downgrades ready hook health when a later snapshot reports drift", () => {
    const merged = mergeHookHealthPreferReady(
      hookHealth(ready, ready, ready),
      hookHealth(drifted, ready, ready),
    );
    expect(merged.claude).toEqual(drifted);
    expect(merged.codex).toEqual(ready);
  });

  it("does not downgrade ready hook health to an empty startup snapshot", () => {
    const merged = mergeHookHealthPreferReady(
      hookHealth(ready, ready, ready),
      hookHealth(missing, missing, missing),
    );
    expect(merged).toEqual(hookHealth(ready, ready, ready));
    const analysis = analyzeHookHealth(merged);
    expect(analysis.connectedCount).toBe(3);
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.allConnected).toBe(true);
  });

  it("handles undefined health gracefully", () => {
    const analysis = analyzeHookHealth(undefined as HookHealthSnapshot | undefined);
    expect(analysis.needsFirstTimeSetup).toBe(true);
    expect(analysis.totalCount).toBe(0);
  });

  it("derives normal atoll logo when all agents are connected", () => {
    const analysis = analyzeHookHealth(hookHealth(ready, ready, ready));
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "idle",
    });
  });

  it("derives normal atoll logo when one agent is uninstalled but others are connected", () => {
    const analysis = analyzeHookHealth(hookHealth(missing, ready, ready));
    expect(deriveHeaderLogoDisplay(analysis, "coding")).toEqual({
      kind: "atoll",
      activity: "coding",
    });
  });

  it("derives dead agent logo for a single drifted agent", () => {
    const analysis = analyzeHookHealth(hookHealth(drifted, ready, ready));
    expect(deriveHeaderLogoDisplay(analysis, "coding")).toEqual({
      kind: "agent",
      agent: "claude",
      mood: "dead",
    });
  });

  it("derives dead cursor logo when cursor hook drifts", () => {
    const cursorDrifted = {
      installed: true,
      scriptFound: false,
      settingsPath: "/tmp/hooks.json",
      scriptPath: "",
    };
    const analysis = analyzeHookHealth({
      claude: ready,
      codex: ready,
      cursor: cursorDrifted,
    });
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["cursor"]);
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "agent",
      agent: "cursor",
      mood: "dead",
    });
  });

  it("flags configured but uninstalled agents as disconnected", () => {
    const configuredCursor = new Set<HookAgentKey>(["cursor"]);
    const analysis = analyzeHookHealth(
      {
        claude: ready,
        codex: ready,
        cursor: missing,
      },
      { configuredAgents: configuredCursor },
    );
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["cursor"]);
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "agent",
      agent: "cursor",
      mood: "dead",
    });
  });

  it("does not flag uninstalled cursor as disconnected when other agents stay connected", () => {
    const analysis = analyzeHookHealth({
      claude: ready,
      codex: ready,
      cursor: missing,
    });
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.needsReconnect).toBe(false);
    expect(analysis.summary).toBe("All agents connected");
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "idle",
    });
  });

  it("derives dead atoll logo when all agents are disconnected", () => {
    const analysis = analyzeHookHealth(hookHealth(missing, missing, missing));
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "dead",
    });
  });

  it("flags a ready agent that needs re-trust without treating it as disconnected", () => {
    const codexNeedsRetrust = { ...ready, needsRetrust: true };
    const analysis = analyzeHookHealth(hookHealth(ready, codexNeedsRetrust, ready));

    expect(analysis.connectedCount).toBe(3);
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.retrustAgents.map((agent) => agent.key)).toEqual(["codex"]);
    expect(analysis.needsReconnect).toBe(true);
    expect(analysis.allConnected).toBe(false);
    expect(analysis.summary).toContain("re-trust needed");
    expect(hookAttentionTitle(analysis)).toContain("Codex");
    expect(hookAttentionTitle(analysis)).toContain("re-trust");
  });

  it("does not treat a needs-retrust agent as dead in the header logo", () => {
    const codexNeedsRetrust = { ...ready, needsRetrust: true };
    const analysis = analyzeHookHealth(hookHealth(ready, codexNeedsRetrust, ready));
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "idle",
    });
  });

  it("combines disconnected and retrust agents in the attention title", () => {
    const cursorNeedsRetrust = { ...ready, needsRetrust: true };
    const analysis = analyzeHookHealth(hookHealth(drifted, ready, cursorNeedsRetrust));
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["claude"]);
    expect(analysis.retrustAgents.map((agent) => agent.key)).toEqual(["cursor"]);
    const title = hookAttentionTitle(analysis);
    expect(title).toContain("Claude Code");
    expect(title).toContain("Cursor");
  });

  it("provides agent-specific re-trust guidance", () => {
    expect(hookRetrustNote("codex")).toContain("/hooks");
    expect(hookRetrustNote("codex")).toContain("cached");
    expect(hookRetrustNote("claude")).toContain("Claude");
    expect(hookRetrustNote("cursor")).toContain("Cursor");
  });

  it("derives idle atoll logo before hook health is known", () => {
    const analysis = analyzeHookHealth(undefined as HookHealthSnapshot | undefined);
    expect(deriveHeaderLogoDisplay(analysis, "idle", { hookHealthKnown: false })).toEqual({
      kind: "atoll",
      activity: "idle",
    });
    expect(deriveHeaderLogoDisplay(analysis, "coding", { hookHealthKnown: false })).toEqual({
      kind: "atoll",
      activity: "idle",
    });
    expect(deriveHeaderLogoDisplay(analysis, "dead", { hookHealthKnown: false })).toEqual({
      kind: "atoll",
      activity: "idle",
    });
  });
});
