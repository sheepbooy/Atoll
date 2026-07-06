import { check, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type AppUpdateState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "available"; version: string; notes?: string }
  | { status: "downloading"; version: string; progress: number }
  | { status: "error"; message: string };

export const UPDATE_RECHECK_MS = 6 * 60 * 60 * 1000;
export const UPDATE_INITIAL_DELAY_MS = 3_000;

function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

let pendingUpdate: Awaited<ReturnType<typeof check>> | null = null;

export function isTauriUpdateRuntime(): boolean {
  return isTauriRuntime();
}

export async function checkAppUpdate(): Promise<AppUpdateState> {
  if (!isTauriRuntime()) {
    return { status: "idle" };
  }

  try {
    const update = await check();
    pendingUpdate = update;
    if (!update) {
      return { status: "idle" };
    }
    return {
      status: "available",
      version: update.version,
      notes: update.body ?? undefined,
    };
  } catch (error) {
    pendingUpdate = null;
    return {
      status: "error",
      message: error instanceof Error ? error.message : String(error),
    };
  }
}

export type InstallProgressCallback = (progress: number) => void;

export async function installAppUpdate(
  onProgress?: InstallProgressCallback,
): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  const update = pendingUpdate;
  if (!update) {
    throw new Error("No pending update. Check for updates again before installing.");
  }

  pendingUpdate = update;
  let downloaded = 0;
  let contentLength = 0;

  await update.downloadAndInstall((event: DownloadEvent) => {
    switch (event.event) {
      case "Started":
        contentLength = event.data.contentLength ?? 0;
        downloaded = 0;
        onProgress?.(0);
        break;
      case "Progress":
        downloaded += event.data.chunkLength;
        if (contentLength > 0) {
          onProgress?.(Math.min(1, downloaded / contentLength));
        }
        break;
      case "Finished":
        onProgress?.(1);
        break;
    }
  });

  await relaunch();
}

export function clearPendingUpdate(): void {
  pendingUpdate = null;
}

export async function getAppVersion(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  const { getVersion } = await import("@tauri-apps/api/app");
  return getVersion();
}
