import type { HookAgentKey } from "./hookHealth";
import type { HookHealthSnapshot } from "./tauri";

const STORAGE_KEY = "atoll-hook-agents-configured";

const ALL_HOOK_AGENTS: HookAgentKey[] = ["claude", "codex", "cursor"];

function parseConfigured(raw: string | null): Set<HookAgentKey> {
  if (!raw) return new Set();
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return new Set();
    return new Set(
      parsed.filter(
        (entry): entry is HookAgentKey =>
          entry === "claude" || entry === "codex" || entry === "cursor",
      ),
    );
  } catch {
    return new Set();
  }
}

function writeConfigured(agents: Set<HookAgentKey>): void {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify([...agents]));
  } catch {
    // ignore quota / private mode
  }
}

export function readConfiguredHookAgents(): Set<HookAgentKey> {
  if (typeof window === "undefined") return new Set();
  return parseConfigured(window.localStorage.getItem(STORAGE_KEY));
}

export function markHookAgentConfigured(agent: HookAgentKey): Set<HookAgentKey> {
  const next = readConfiguredHookAgents();
  next.add(agent);
  writeConfigured(next);
  return next;
}

export function markAllHookAgentsConfigured(): Set<HookAgentKey> {
  const next = new Set(ALL_HOOK_AGENTS);
  writeConfigured(next);
  return next;
}

export function seedConfiguredFromHookHealth(
  health: HookHealthSnapshot | undefined,
): Set<HookAgentKey> {
  const next = readConfiguredHookAgents();
  let changed = false;
  for (const agent of ALL_HOOK_AGENTS) {
    if (health?.[agent]?.installed && !next.has(agent)) {
      next.add(agent);
      changed = true;
    }
  }
  if (changed) {
    writeConfigured(next);
  }
  return next;
}

export function clearConfiguredHookAgentsForTests(): void {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.removeItem(STORAGE_KEY);
  } catch {
    // ignore
  }
}
