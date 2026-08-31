import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";
export type ClipboardEntryKind = "text" | "image" | "files";

export interface ClipboardEntry {
  id: string;
  kind: ClipboardEntryKind;
  content: string;
  preview: string;
  copiedAt: number;
  byteSize?: number;
  favorited?: boolean;
}

export const CLIPBOARD_HISTORY_EXPIRY_SECS = 24 * 60 * 60;

export async function getClipboardHistory(): Promise<ClipboardEntry[]> {
  if (!isTauriRuntime()) {
    return [];
  }
  return invoke<ClipboardEntry[]>("get_clipboard_history");
}

export async function copyClipboardEntry(id: string): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("copy_clipboard_entry", { id });
}

export async function clearClipboardHistory(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  return invoke<void>("clear_clipboard_history");
}

export async function getClipboardHistoryEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("get_clipboard_history_enabled");
}

export async function setClipboardHistoryEnabled(enabled: boolean): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_clipboard_history_enabled", { enabled });
}

export async function getClipboardHistoryLimit(): Promise<number> {
  if (!isTauriRuntime()) {
    return 50;
  }
  return invoke<number>("get_clipboard_history_limit");
}

export async function setClipboardHistoryLimit(limit: number): Promise<number> {
  if (!isTauriRuntime()) {
    return limit;
  }
  return invoke<number>("set_clipboard_history_limit", { limit });
}

export async function getClipboardEntryThumbnail(id: string): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  return invoke<string | null>("get_clipboard_entry_thumbnail", { id });
}

export async function toggleClipboardFavorite(id: string): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }
  return invoke<boolean>("toggle_clipboard_favorite", { id });
}

export async function onClipboardHistoryChanged(
  callback: (entries: ClipboardEntry[]) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<ClipboardEntry[]>("clipboard-history-changed", (event) =>
    callback(event.payload),
  );
}
