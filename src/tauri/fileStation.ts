import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { isTauriRuntime } from "./runtime";

export interface StagedFile {
  id: string;
  /** Canonical absolute path of the referenced file. */
  path: string;
  fileName: string;
  byteSize: number;
  isDir: boolean;
  /** ms since the Unix epoch when the file was staged. */
  stagedAt: number;
  /** True when the referenced file no longer exists on disk. */
  lost: boolean;
}

export interface StageFilesResult {
  files: StagedFile[];
  added: number;
  skipped: number;
  evicted: number;
}

/** Matches STAGED_FILES_LIMIT in src-tauri/src/file_station.rs. */
export const STAGED_FILES_LIMIT = 30;

export async function getStagedFiles(): Promise<StagedFile[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invoke<StagedFile[]>("get_staged_files");
}

export async function stageFiles(paths: string[]): Promise<StageFilesResult | null> {
  if (!isTauriRuntime() || paths.length === 0) {
    return null;
  }
  return invoke<StageFilesResult>("stage_files", { paths });
}

export async function removeStagedFile(id: string): Promise<StagedFile[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invoke<StagedFile[]>("remove_staged_file", { id });
}

export async function clearStagedFiles(): Promise<StagedFile[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invoke<StagedFile[]>("clear_staged_files");
}

/** Copy staged files (by id) to the clipboard as a file list; returns how many. */
export async function copyStagedFilesToClipboard(ids: string[]): Promise<number> {
  if (!isTauriRuntime() || ids.length === 0) {
    return 0;
  }
  return invoke<number>("copy_staged_files_to_clipboard", { ids });
}

/**
 * Begin a native drag-out of staged files (row gesture on macOS/Windows).
 * Resolves false when the platform could not anchor a drag session — callers
 * stay silent and the click-to-copy path remains the fallback.
 */
export async function beginStagedFilesDrag(ids: string[]): Promise<boolean> {
  if (!isTauriRuntime() || ids.length === 0) {
    return false;
  }
  return invoke<boolean>("begin_staged_files_drag", { ids });
}

export async function onFileStationChanged(callback: (files: StagedFile[]) => void) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<StagedFile[]>("file-station-changed", (event) =>
    callback(event.payload),
  );
}

/**
 * Native drag-drop events for the island webview. Tauri v2 keeps
 * `dragDropEnabled` at its default (true), which routes file drops through
 * this channel instead of HTML5 drop events.
 */
export async function onIslandDragDropEvent(
  callback: (kind: "enter" | "over" | "leave" | "drop", paths: string[]) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return getCurrentWebview().onDragDropEvent((event) => {
    const payload = event.payload;
    switch (payload.type) {
      case "enter":
      case "drop":
        callback(payload.type, payload.paths);
        break;
      case "leave":
        callback("leave", []);
        break;
      default:
        callback("over", []);
        break;
    }
  });
}
