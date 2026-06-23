import type { HookStatus, IslandSnapshot, PermissionRequest } from "./tauri";

export type DemoMode = "compact" | "approval" | "idle" | "sessions" | "gif";

export function getDemoMode(): DemoMode | null {
  if ("__TAURI_INTERNALS__" in window) return null;
  const mode = new URLSearchParams(window.location.search).get("demo");
  if (
    mode === "compact" ||
    mode === "approval" ||
    mode === "idle" ||
    mode === "sessions" ||
    mode === "gif"
  ) {
    return mode;
  }
  return null;
}

const pendingRequest: PermissionRequest = {
  id: "demo-request-1",
  toolUseId: "tool-1",
  agent: "claude",
  session: "session-atoll",
  command: "Bash: npm test -- --run",
  detail: "Run the project test suite in the current workspace.",
  cwd: "~/code/my-app",
  requestedAt: new Date().toISOString(),
  status: "pending",
  supportsAlways: true,
};

const sessions = [
  {
    sessionId: "session-atoll",
    agent: "claude" as const,
    cwd: "~/code/my-app",
    pendingCount: 1,
    totalCount: 4,
    lastActivity: new Date().toISOString(),
    transcriptPath: null,
    pinned: true,
  },
  {
    sessionId: "session-api",
    agent: "codex" as const,
    cwd: "~/code/api-server",
    pendingCount: 0,
    totalCount: 12,
    lastActivity: new Date().toISOString(),
    transcriptPath: null,
  },
  {
    sessionId: "session-docs",
    agent: "gemini" as const,
    cwd: "~/code/docs-site",
    pendingCount: 0,
    totalCount: 3,
    lastActivity: new Date().toISOString(),
    transcriptPath: null,
  },
];

const demoHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.claude/settings.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-claude-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.claude/settings.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-claude-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoCodexHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.codex/hooks.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-codex-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoCodexHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.codex/hooks.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-codex-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

export function getDemoSnapshot(mode: DemoMode): IslandSnapshot {
  const base: IslandSnapshot = {
    online: true,
    pendingCount: 0,
    archivedCount: 0,
    activeRequest: null,
    recent: [],
    sessions: [],
    dailyTokens: {
      inputTokens: 128_400,
      outputTokens: 42_180,
      cacheReadTokens: 890_000,
      cacheCreationTokens: 12_400,
    },
    activeSessionTokens: {
      inputTokens: 128_400,
      outputTokens: 42_180,
      cacheReadTokens: 890_000,
      cacheCreationTokens: 12_400,
    },
    hookHealth: {
      claude: demoHookInstalled,
      codex: demoCodexHookInstalled,
    },
  };

  switch (mode) {
    case "approval":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: pendingRequest,
        recent: [pendingRequest],
        sessions,
      };
    case "compact":
    case "gif":
    case "sessions":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: pendingRequest,
        recent: [pendingRequest],
        sessions,
      };
    case "idle":
    default:
      return base;
  }
}

export function getDemoHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoHookMissing : demoHookInstalled;
}

export function getDemoCodexHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoCodexHookMissing : demoCodexHookInstalled;
}

export function shouldAutoExpandDemo(mode: DemoMode): boolean {
  return mode === "approval" || mode === "sessions" || mode === "idle";
}

export function isGifCaptureMode(): boolean {
  return getDemoMode() === "gif";
}
