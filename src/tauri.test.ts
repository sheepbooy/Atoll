import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

function setTauriRuntime(enabled: boolean) {
  if (enabled) {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    return;
  }

  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
}

describe("Tauri bridge", () => {
  beforeEach(() => {
    vi.resetModules();
    invoke.mockReset();
    setTauriRuntime(false);
  });

  afterEach(() => {
    setTauriRuntime(false);
  });

  it("starts with an empty browser fallback snapshot", async () => {
    const { EMPTY_HOOK_HEALTH, getSnapshot } = await import("./tauri");

    await expect(getSnapshot()).resolves.toEqual({
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      dailyTokens: {
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
      },
      activeSessionTokens: {
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
      },
      hookHealth: EMPTY_HOOK_HEALTH,
    });
  });

  it("invokes quit_atoll in the Tauri runtime", async () => {
    setTauriRuntime(true);
    const { quitAtoll } = await import("./tauri");

    await quitAtoll();

    expect(invoke).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("quit_atoll");
  });

  it("normalizes snake_case hook health from IPC payloads", async () => {
    setTauriRuntime(true);
    invoke.mockResolvedValueOnce({
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      dailyTokens: {
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
      },
      activeSessionTokens: {
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
      },
      hook_health: {
        claude: {
          installed: true,
          script_found: true,
          settings_path: "/tmp/claude.json",
          script_path: "/tmp/atoll-claude-hook.mjs",
        },
        codex: {
          installed: true,
          script_found: true,
          settings_path: "/tmp/codex.json",
          script_path: "/tmp/atoll-codex-hook.mjs",
        },
      },
    });

    const { getSnapshot } = await import("./tauri");
    const snapshot = await getSnapshot();

    expect(snapshot.hookHealth.claude.scriptFound).toBe(true);
    expect(snapshot.hookHealth.codex.scriptPath).toBe("/tmp/atoll-codex-hook.mjs");
  });

  it("detects Windows micro island synchronously", async () => {
    setTauriRuntime(true);
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });

    const { usesMicroIslandSync } = await import("./tauri");
    expect(usesMicroIslandSync()).toBe(true);

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
  });

  it("returns false for autostart outside the Tauri runtime", async () => {
    const { isAutostartEnabled, enableAutostart, disableAutostart } = await import("./tauri");

    await expect(isAutostartEnabled()).resolves.toBe(false);
    await enableAutostart();
    await disableAutostart();

    expect(invoke).not.toHaveBeenCalled();
  });

  it("delegates autostart controls to Tauri commands in the Tauri runtime", async () => {
    setTauriRuntime(true);
    invoke.mockResolvedValueOnce(true);
    invoke.mockResolvedValueOnce(undefined);
    invoke.mockResolvedValueOnce(undefined);

    const { isAutostartEnabled, enableAutostart, disableAutostart } = await import("./tauri");

    await expect(isAutostartEnabled()).resolves.toBe(true);
    await enableAutostart();
    await disableAutostart();

    expect(invoke).toHaveBeenNthCalledWith(1, "is_autostart_enabled");
    expect(invoke).toHaveBeenNthCalledWith(2, "set_autostart_enabled", { enabled: true });
    expect(invoke).toHaveBeenNthCalledWith(3, "set_autostart_enabled", { enabled: false });
  });
});
