import { invoke } from "@tauri-apps/api/core";

import { DEFAULT_GLOBAL_SHORTCUTS } from "../shortcuts";
import { isTauriRuntime } from "./runtime";
export type ApprovalNoticeMode = "interrupt" | "notify";

const APPROVAL_NOTICE_MODES: ApprovalNoticeMode[] = ["interrupt", "notify"];

export function normalizeApprovalNoticeMode(value: unknown): ApprovalNoticeMode {
  return APPROVAL_NOTICE_MODES.includes(value as ApprovalNoticeMode)
    ? (value as ApprovalNoticeMode)
    : "interrupt";
}

export async function getApprovalNoticeMode(): Promise<ApprovalNoticeMode> {
  if (!isTauriRuntime()) {
    return "interrupt";
  }
  return normalizeApprovalNoticeMode(await invoke<string>("get_approval_notice_mode"));
}

export async function setApprovalNoticeMode(
  mode: ApprovalNoticeMode,
): Promise<ApprovalNoticeMode> {
  if (!isTauriRuntime()) {
    return mode;
  }
  return normalizeApprovalNoticeMode(
    await invoke<string>("set_approval_notice_mode", { mode }),
  );
}

export async function setNotificationLanguage(language: string): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  await invoke("set_notification_language", { language });
}

export type ShortcutAction = "summon" | "approve" | "deny";

export interface GlobalShortcutConfig {
  enabled: boolean;
  summon: string;
  approve: string;
  deny: string;
}

/** Per-action error text from the last registration attempt; null/undefined = OK. */
export interface GlobalShortcutErrors {
  summon?: string | null;
  approve?: string | null;
  deny?: string | null;
}

export interface GlobalShortcutView {
  config: GlobalShortcutConfig;
  errors: GlobalShortcutErrors;
}

export async function getGlobalShortcutConfig(): Promise<GlobalShortcutView> {
  if (!isTauriRuntime()) {
    return { config: { ...DEFAULT_GLOBAL_SHORTCUTS }, errors: {} };
  }
  return invoke<GlobalShortcutView>("get_global_shortcut_config");
}

export async function setGlobalShortcutConfig(
  config: GlobalShortcutConfig,
): Promise<GlobalShortcutView> {
  if (!isTauriRuntime()) {
    return { config, errors: {} };
  }
  return invoke<GlobalShortcutView>("set_global_shortcut_config", { config });
}

export async function getSessionRetention(): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>("get_session_retention");
  }
  return 900;
}

export async function setSessionRetention(minutes: number): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>("set_session_retention", { minutes });
  }
  return minutes * 60;
}
