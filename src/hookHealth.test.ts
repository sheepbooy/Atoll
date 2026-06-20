import { describe, expect, it } from "vitest";
import {
  analyzeHookHealth,
  deriveHeaderLogoDisplay,
  hookAttentionTitle,
  isHookDisconnected,
  isHookDrifted,
  isHookReady,
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

describe("hookHealth", () => {
  it("detects first-time setup when no agents are connected", () => {
    const analysis = analyzeHookHealth({
      claude: missing,
      codex: missing,
    });

    expect(analysis.needsFirstTimeSetup).toBe(true);
    expect(analysis.needsReconnect).toBe(false);
    expect(analysis.disconnectedAgents).toEqual([]);
    expect(analysis.summary).toBe("Not connected");
  });

  it("flags an uninstalled agent when another agent stays connected", () => {
    const analysis = analyzeHookHealth({
      claude: missing,
      codex: ready,
    });

    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(true);
    expect(analysis.connectedCount).toBe(1);
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["claude"]);
    expect(analysis.summary).toBe("1 of 2 connected");
    expect(hookAttentionTitle(analysis)).toContain("Claude Code");
  });

  it("detects partial drift when one installed agent loses its shim", () => {
    const analysis = analyzeHookHealth({
      claude: drifted,
      codex: ready,
    });

    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(true);
    expect(analysis.connectedCount).toBe(1);
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["claude"]);
    expect(hookAttentionTitle(analysis)).toContain("Claude Code");
  });

  it("treats missing scripts as drift only when hooks were installed", () => {
    expect(isHookReady(drifted)).toBe(false);
    expect(isHookDrifted(drifted)).toBe(true);
    expect(isHookDrifted(missing)).toBe(false);
    expect(isHookDisconnected(missing, true)).toBe(true);
    expect(isHookDisconnected(missing, false)).toBe(false);
  });

  it("handles undefined health gracefully", () => {
    const analysis = analyzeHookHealth(undefined as HookHealthSnapshot | undefined);
    expect(analysis.needsFirstTimeSetup).toBe(true);
    expect(analysis.totalCount).toBe(0);
  });

  it("derives normal atoll logo when all agents are connected", () => {
    const analysis = analyzeHookHealth({ claude: ready, codex: ready });
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "idle",
    });
  });

  it("derives dead agent logo when one agent is uninstalled", () => {
    const analysis = analyzeHookHealth({ claude: missing, codex: ready });
    expect(deriveHeaderLogoDisplay(analysis, "coding")).toEqual({
      kind: "agent",
      agent: "claude",
      mood: "dead",
    });
  });

  it("derives dead agent logo for a single drifted agent", () => {
    const analysis = analyzeHookHealth({ claude: drifted, codex: ready });
    expect(deriveHeaderLogoDisplay(analysis, "coding")).toEqual({
      kind: "agent",
      agent: "claude",
      mood: "dead",
    });
  });

  it("derives dead atoll logo when all agents are disconnected", () => {
    const analysis = analyzeHookHealth({ claude: missing, codex: missing });
    expect(deriveHeaderLogoDisplay(analysis, "idle")).toEqual({
      kind: "atoll",
      activity: "dead",
    });
  });
});
