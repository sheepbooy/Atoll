import { invoke } from "@tauri-apps/api/core";

import {
  getDemoCodexHookStatus,
  getDemoCursorHookStatus,
  getDemoGeminiHookStatus,
  getDemoHookStatus,
  getDemoMode,
  getDemoZcodeHookStatus,
} from "../demoSnapshot";
import { EMPTY_HOOK_HEALTH, type HookHealthSnapshot, type HookStatus } from "./types";

import type { AgentKind } from "./types";
import { isTauriRuntime } from "./runtime";

export function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as Record<string, unknown>;
}

function readBool(record: Record<string, unknown>, camel: string, snake: string): boolean {
  const value = record[camel] ?? record[snake];
  return Boolean(value);
}

function readString(record: Record<string, unknown>, camel: string, snake: string): string {
  const value = record[camel] ?? record[snake];
  return typeof value === "string" ? value : "";
}

export function normalizeHookStatus(raw: unknown): HookStatus {
  const record = asRecord(raw);
  if (!record) {
    return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
  }
  const nodeFoundRaw = record.nodeFound ?? record.node_found;
  const competingHooksRaw = record.competingHooks ?? record.competing_hooks;
  return {
    installed: readBool(record, "installed", "installed"),
    scriptFound: readBool(record, "scriptFound", "script_found"),
    settingsPath: readString(record, "settingsPath", "settings_path"),
    scriptPath: readString(record, "scriptPath", "script_path"),
    nodePath: readString(record, "nodePath", "node_path"),
    nodeFound: nodeFoundRaw === undefined ? true : Boolean(nodeFoundRaw),
    needsRetrust: readBool(record, "needsRetrust", "needs_retrust"),
    competingHooks: Array.isArray(competingHooksRaw)
      ? competingHooksRaw
          .map((entry) => asRecord(entry))
          .filter((entry): entry is Record<string, unknown> => entry !== null)
          .map((entry) => ({
            event: readString(entry, "event", "event"),
            command: readString(entry, "command", "command"),
            binaryExists: readBool(entry, "binaryExists", "binary_exists"),
          }))
      : [],
  };
}

export function normalizeHookHealth(raw: unknown): HookHealthSnapshot {
  const record = asRecord(raw);
  if (!record) {
    return EMPTY_HOOK_HEALTH;
  }
  return {
    claude: normalizeHookStatus(record.claude),
    codex: normalizeHookStatus(record.codex),
    cursor: normalizeHookStatus(record.cursor ?? EMPTY_HOOK_HEALTH.cursor),
    zcode: normalizeHookStatus(record.zcode ?? EMPTY_HOOK_HEALTH.zcode),
    gemini: normalizeHookStatus(record.gemini ?? EMPTY_HOOK_HEALTH.gemini),
  };
}

export async function getClaudeHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("get_claude_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("install_claude_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_claude_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

/** Remove non-Atoll hooks whose binaries are missing from `~/.claude/settings.json`.
 * Dead competitor hooks can veto Atoll's permission decisions under Claude Code's
 * most-restrictive-wins merge. No-op outside Tauri. */
export async function removeCompetingClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(
      await invoke<HookStatus>("remove_competing_claude_hooks"),
    );
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getCodexHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("get_codex_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoCodexHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installCodexHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("install_codex_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallCodexHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_codex_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getCursorHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("get_cursor_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoCursorHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installCursorHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("install_cursor_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallCursorHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_cursor_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getZcodeHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("get_zcode_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoZcodeHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installZcodeHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("install_zcode_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallZcodeHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_zcode_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getGeminiHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("get_gemini_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoGeminiHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installGeminiHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("install_gemini_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallGeminiHooks(): Promise<HookStatus> {
  if (isTauriRuntime()) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_gemini_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}
