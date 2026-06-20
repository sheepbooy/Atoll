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

  it("does not invoke quit_atoll outside the Tauri runtime", async () => {
    const { quitAtoll } = await import("./tauri");

    await quitAtoll();

    expect(invoke).not.toHaveBeenCalled();
  });
});
