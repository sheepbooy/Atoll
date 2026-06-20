import { describe, expect, it } from "vitest";
import {
  analyzeHookHealth,
  deriveHeaderLogoDisplay,
  hookAttentionTitle,
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

describe("hookHealth", () => {
  it("detects first-time setup when no agents are connected", () => {
    const analysis = analyzeHookHealth({
      claude: missing,
      codex: missing,
    });

    expect(analysis.needsFirstTimeSetup).toBe(true);
    expect(analysis.needsReconnect).toBe(false);
    expect(analysis.summary).toBe("Not connected");
  });

  it("detects partial drift when one agent disconnects", () => {
    const analysis = analyzeHookHealth({
      claude: missing,
      codex: ready,
    });

    expect(analysis.needsFirstTimeSetup).toBe(false);
    expect(analysis.needsReconnect).toBe(true);
    expect(analysis.connectedCount).toBe(1);
    expect(analysis.disconnectedAgents.map((agent) => agent.key)).toEqual(["claude"]);
    expect(hookAttentionTitle(analysis)).toContain("Claude Code");
  });

  it("treats missing scripts as not ready", () => {
    expect(
      isHookReady({
        ...ready,
        scriptFound: false,
      }),
    ).toBe(false);
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

  it("derives dead agent logo for a single disconnected agent", () => {
    const analysis = analyzeHookHealth({ claude: missing, codex: ready });
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
