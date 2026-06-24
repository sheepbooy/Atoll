import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const checkMock = vi.fn();
const relaunchMock = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: (...args: unknown[]) => relaunchMock(...args),
}));

describe("appUpdate", () => {
  beforeEach(() => {
    checkMock.mockReset();
    relaunchMock.mockReset();
    vi.stubGlobal("__TAURI_INTERNALS__", {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.resetModules();
  });

  it("returns idle when no update is available", async () => {
    checkMock.mockResolvedValue(null);
    const { checkAppUpdate } = await import("./appUpdate");
    await expect(checkAppUpdate()).resolves.toEqual({ status: "idle" });
  });

  it("returns available when updater finds a newer version", async () => {
    checkMock.mockResolvedValue({
      version: "0.2.0",
      body: "Bug fixes",
      downloadAndInstall: vi.fn(),
    });
    const { checkAppUpdate } = await import("./appUpdate");
    await expect(checkAppUpdate()).resolves.toEqual({
      status: "available",
      version: "0.2.0",
      notes: "Bug fixes",
    });
  });

  it("returns idle on check errors", async () => {
    checkMock.mockRejectedValue(new Error("network down"));
    const { checkAppUpdate } = await import("./appUpdate");
    await expect(checkAppUpdate()).resolves.toEqual({
      status: "error",
      message: "network down",
    });
  });

  it("downloads, installs, and relaunches", async () => {
    const downloadAndInstall = vi.fn(async (onEvent) => {
      onEvent({ event: "Started", data: { contentLength: 100 } });
      onEvent({ event: "Progress", data: { chunkLength: 50 } });
      onEvent({ event: "Progress", data: { chunkLength: 50 } });
      onEvent({ event: "Finished" });
    });
    checkMock.mockResolvedValue({
      version: "0.2.0",
      downloadAndInstall,
    });

    const progress: number[] = [];
    const { checkAppUpdate, installAppUpdate } = await import("./appUpdate");
    await checkAppUpdate();
    await installAppUpdate((value) => progress.push(value));

    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(relaunchMock).toHaveBeenCalledOnce();
    expect(progress).toEqual([0, 0.5, 1, 1]);
  });

  it("returns idle outside Tauri runtime", async () => {
    vi.unstubAllGlobals();
    vi.resetModules();
    const { checkAppUpdate } = await import("./appUpdate");
    await expect(checkAppUpdate()).resolves.toEqual({ status: "idle" });
    expect(checkMock).not.toHaveBeenCalled();
  });
});
