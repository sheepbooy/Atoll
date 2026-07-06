import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const checkMock = vi.fn();
const relaunchMock = vi.fn();
const getVersionMock = vi.fn();

vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => checkMock(...args),
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: (...args: unknown[]) => relaunchMock(...args),
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: (...args: unknown[]) => getVersionMock(...args),
}));

describe("appUpdate", () => {
  beforeEach(() => {
    checkMock.mockReset();
    relaunchMock.mockReset();
    getVersionMock.mockReset();
    getVersionMock.mockResolvedValue("0.1.21");
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

  it("refuses to install without a pending checked update", async () => {
    const { installAppUpdate } = await import("./appUpdate");

    await expect(installAppUpdate()).rejects.toThrow("No pending update");
    expect(checkMock).not.toHaveBeenCalled();
    expect(relaunchMock).not.toHaveBeenCalled();
  });

  it("clears pending updates", async () => {
    const downloadAndInstall = vi.fn();
    checkMock.mockResolvedValue({
      version: "0.2.0",
      downloadAndInstall,
    });

    const { checkAppUpdate, clearPendingUpdate, installAppUpdate } = await import("./appUpdate");
    await checkAppUpdate();
    clearPendingUpdate();

    await expect(installAppUpdate()).rejects.toThrow("No pending update");
    expect(downloadAndInstall).not.toHaveBeenCalled();
  });

  it("detects Tauri runtime after module import", async () => {
    vi.unstubAllGlobals();
    vi.resetModules();
    const { isTauriUpdateRuntime, checkAppUpdate } = await import("./appUpdate");

    expect(isTauriUpdateRuntime()).toBe(false);

    vi.stubGlobal("__TAURI_INTERNALS__", {});
    checkMock.mockResolvedValue(null);

    expect(isTauriUpdateRuntime()).toBe(true);
    await expect(checkAppUpdate()).resolves.toEqual({ status: "idle" });
    expect(checkMock).toHaveBeenCalledOnce();
  });

  it("returns idle outside Tauri runtime", async () => {
    vi.unstubAllGlobals();
    vi.resetModules();
    const { checkAppUpdate } = await import("./appUpdate");
    await expect(checkAppUpdate()).resolves.toEqual({ status: "idle" });
    expect(checkMock).not.toHaveBeenCalled();
  });

  it("returns the current app version in Tauri runtime", async () => {
    const { getAppVersion } = await import("./appUpdate");
    await expect(getAppVersion()).resolves.toBe("0.1.21");
    expect(getVersionMock).toHaveBeenCalledOnce();
  });
});
