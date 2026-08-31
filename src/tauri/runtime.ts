import { invoke } from "@tauri-apps/api/core";

import type { AgentKind } from "./types";

export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in window;
}

/** Matches `uses_micro_island` in src-tauri (Windows-only micro island). */
export function isWindowsTauriRuntime(): boolean {
  if (!("__TAURI_INTERNALS__" in window)) {
    return false;
  }
  return /Windows/i.test(navigator.userAgent);
}


export function isAllowedExternalUrl(url: string): boolean {
  try {
    const protocol = new URL(url, window.location.href).protocol;
    return protocol === "http:" || protocol === "https:";
  } catch {
    return false;
  }
}



export async function quitAtoll() {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("quit_atoll");
}

export async function deactivateAtoll(
  agent?: AgentKind,
  session?: string,
  cwd?: string,
) {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("deactivate_atoll", {
    agent: agent ?? null,
    session: session ?? null,
    cwd: cwd ?? null,
  });
}

export async function openAgentApp(
  agent: AgentKind,
  cwd: string,
  session?: string,
): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("open_agent_app", { agent, cwd, session: session ?? null });
}

export async function revealPath(path: string): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  return invoke<void>("reveal_path", { path });
}

export async function openInTerminal(cwd: string): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("open_in_terminal", { cwd });
}

export async function focusClaudeApp(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("focus_claude_app");
}

export async function openUrl(url: string): Promise<void> {
  if (!isAllowedExternalUrl(url)) {
    return;
  }

  if (!isTauriRuntime()) {
    window.open(url, "_blank");
    return;
  }

  return invoke<void>("open_url", { url });
}

export async function isAutostartEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  return invoke<boolean>("is_autostart_enabled");
}

export async function enableAutostart(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_autostart_enabled", { enabled: true });
}

export async function disableAutostart(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_autostart_enabled", { enabled: false });
}
