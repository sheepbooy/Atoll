import type { HookStatus, IslandSnapshot, PermissionRequest } from "./tauri";

export type DemoMode =
  | "compact"
  | "approval"
  | "idle"
  | "sessions"
  | "gif"
  | "plan-question"
  | "plan-approval";

export function getDemoMode(): DemoMode | null {
  if ("__TAURI_INTERNALS__" in window) return null;
  const mode = new URLSearchParams(window.location.search).get("demo");
  if (
    mode === "compact" ||
    mode === "approval" ||
    mode === "idle" ||
    mode === "sessions" ||
    mode === "gif" ||
    mode === "plan-question" ||
    mode === "plan-approval"
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

const planQuestionRequest: PermissionRequest = {
  id: "demo-plan-question",
  toolUseId: "tool-plan-q",
  agent: "claude",
  session: "session-atoll",
  command: "AskUserQuestion",
  detail: "Agent needs your input to continue planning.",
  cwd: "~/code/Atoll",
  requestedAt: new Date().toISOString(),
  status: "pending",
  supportsAlways: false,
  toolInput: {
    questions: [
      {
        header: "Scope",
        question: "Which areas should we focus on first?",
        multiSelect: true,
        options: [
          {
            label: "Hook bridge",
            description: "Permission events and local HTTP bridge",
          },
          {
            label: "Plan mode UI",
            description: "Questions card and build approval preview",
          },
          {
            label: "Token tracking",
            description: "Heatmap and session usage metrics",
          },
        ],
      },
    ],
  },
};

const planApprovalRequest: PermissionRequest = {
  id: "demo-plan-approval",
  toolUseId: "tool-plan-a",
  agent: "claude",
  session: "session-atoll",
  command: "ExitPlanMode",
  detail: "Agent is ready to start building.",
  cwd: "~/code/Atoll",
  requestedAt: new Date().toISOString(),
  status: "pending",
  supportsAlways: false,
  toolInput: {
    plan: `# Plan Mode Integration

## Overview
Add Claude Code plan-mode hooks to the Atoll floating island.

## Steps
1. **Question card** — render \`AskUserQuestion\` with multi-select options
2. **Build approval** — preview plan Markdown from \`ExitPlanMode\`
3. **Keyboard flow** — Submit / Deny without leaving the menu bar

## Files
- \`src/App.tsx\` — PlanQuestionCard, PlanApprovalCard
- \`src/styles.css\` — plan-* styles
`,
  },
};

const planSessions = [
  {
    sessionId: "session-atoll",
    agent: "claude" as const,
    cwd: "~/code/Atoll",
    pendingCount: 1,
    totalCount: 6,
    lastActivity: new Date().toISOString(),
    transcriptPath: null,
    pinned: true,
  },
];

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

const demoCursorHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.cursor/hooks.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-cursor-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoCursorHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.cursor/hooks.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-cursor-hook.mjs",
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
      cursor: demoCursorHookInstalled,
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
    case "plan-question":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: planQuestionRequest,
        recent: [planQuestionRequest],
        sessions: planSessions,
      };
    case "plan-approval":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: planApprovalRequest,
        recent: [planApprovalRequest],
        sessions: planSessions,
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

export function getDemoCursorHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoCursorHookMissing : demoCursorHookInstalled;
}

export function shouldAutoExpandDemo(mode: DemoMode): boolean {
  return (
    mode === "approval" ||
    mode === "sessions" ||
    mode === "idle" ||
    mode === "plan-question" ||
    mode === "plan-approval"
  );
}

export function isGifCaptureMode(): boolean {
  return getDemoMode() === "gif";
}
