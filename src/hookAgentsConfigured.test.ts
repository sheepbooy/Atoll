import { afterEach, describe, expect, it, vi } from "vitest";
import {
  clearConfiguredHookAgentsForTests,
  markHookAgentConfigured,
  readConfiguredHookAgents,
  seedConfiguredFromHookHealth,
} from "./hookAgentsConfigured";

const STORAGE_KEY = "atoll-hook-agents-configured";

describe("hookAgentsConfigured", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    clearConfiguredHookAgentsForTests();
  });

  it("reads only known hook agent keys from localStorage", () => {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(["claude", "unknown", "cursor"]),
    );

    expect([...readConfiguredHookAgents()].sort()).toEqual(["claude", "cursor"]);
  });

  it("treats malformed localStorage JSON as empty", () => {
    window.localStorage.setItem(STORAGE_KEY, "{not-json");

    expect(readConfiguredHookAgents()).toEqual(new Set());
  });

  it("ignores quota/private-mode write failures", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("quota exceeded");
    });

    expect(() => markHookAgentConfigured("codex")).not.toThrow();
    expect([...readConfiguredHookAgents()]).toEqual([]);
  });

  it("seeds installed hook agents from health", () => {
    const configured = seedConfiguredFromHookHealth({
      claude: {
        installed: true,
        scriptFound: true,
        settingsPath: "/tmp/claude.json",
        scriptPath: "/tmp/claude.mjs",
      },
      codex: {
        installed: false,
        scriptFound: false,
        settingsPath: "/tmp/codex.json",
        scriptPath: "",
      },
      cursor: {
        installed: true,
        scriptFound: true,
        settingsPath: "/tmp/cursor.json",
        scriptPath: "/tmp/cursor.mjs",
      },
    });

    expect([...configured].sort()).toEqual(["claude", "cursor"]);
  });
});
