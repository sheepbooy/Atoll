import type { HookStatus, IslandSnapshot, PermissionRequest, StagedFile } from "./tauri";

export type DemoMode =
  | "compact"
  | "approval"
  | "idle"
  | "sessions"
  | "gif"
  | "plan-question"
  | "plan-approval"
  | "fileStation";

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
    mode === "plan-approval" ||
    mode === "fileStation"
  ) {
    return mode;
  }
  return null;
}

/** File-station demo rows (one lost) for `?demo=fileStation` in the browser. */
export function getDemoStagedFiles(): StagedFile[] {
  const now = Date.now();
  return [
    {
      id: "demo-1",
      path: "/Users/demo/Desktop/design-v3.png",
      fileName: "design-v3.png",
      byteSize: 2_411_520,
      isDir: false,
      stagedAt: now - 42_000,
      lost: false,
    },
    {
      id: "demo-2",
      path: "/Users/demo/Downloads/annual-report.pdf",
      fileName: "annual-report.pdf",
      byteSize: 18_644_992,
      isDir: false,
      stagedAt: now - 900_000,
      lost: false,
    },
    {
      id: "demo-3",
      path: "/Users/demo/Documents/invoices",
      fileName: "invoices",
      byteSize: 0,
      isDir: true,
      stagedAt: now - 3_600_000,
      lost: false,
    },
    {
      id: "demo-4",
      path: "/private/tmp/old-mock.sketch",
      fileName: "old-mock.sketch",
      byteSize: 8_388_608,
      isDir: false,
      stagedAt: now - 86_400_000,
      lost: true,
    },
  ];
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

const demoZcodeHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.zcode/cli/config.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-zcode-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoZcodeHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.zcode/cli/config.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-zcode-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoGeminiHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.gemini/settings.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-gemini-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoGeminiHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.gemini/settings.json",
  scriptPath: "/Applications/Atoll.app/.../atoll-gemini-hook.mjs",
  nodePath: "/opt/homebrew/bin/node",
  nodeFound: true,
};

const demoOpencodeHookInstalled: HookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "~/.config/opencode/plugins",
  scriptPath: "/Applications/Atoll.app/.../atoll-opencode-bridge.mjs",
  nodePath: "",
  nodeFound: true,
};

const demoOpencodeHookMissing: HookStatus = {
  installed: false,
  scriptFound: true,
  settingsPath: "~/.config/opencode/plugins",
  scriptPath: "/Applications/Atoll.app/.../atoll-opencode-bridge.mjs",
  nodePath: "",
  nodeFound: true,
};

function refreshRequestTimestamp(request: PermissionRequest): PermissionRequest {
  return { ...request, requestedAt: new Date().toISOString() };
}

function refreshSessionActivity<T extends { lastActivity: string }>(session: T): T {
  return { ...session, lastActivity: new Date().toISOString() };
}

export function getDemoSnapshot(mode: DemoMode): IslandSnapshot {
  const pending = refreshRequestTimestamp(pendingRequest);
  const planQuestion = refreshRequestTimestamp(planQuestionRequest);
  const planApproval = refreshRequestTimestamp(planApprovalRequest);
  const demoSessions = sessions.map(refreshSessionActivity);
  const demoPlanSessions = planSessions.map(refreshSessionActivity);
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
    dailyTokensByModel: {
      "claude-sonnet-4-20250514": {
        inputTokens: 128_400,
        outputTokens: 42_180,
        cacheReadTokens: 890_000,
        cacheCreationTokens: 12_400,
      },
    },
    activeSessionTokensByModel: {
      "claude-sonnet-4-20250514": {
        inputTokens: 128_400,
        outputTokens: 42_180,
        cacheReadTokens: 890_000,
        cacheCreationTokens: 12_400,
      },
    },
    hookHealth: {
      claude: demoHookInstalled,
      codex: demoCodexHookInstalled,
      cursor: demoCursorHookInstalled,
      zcode: demoZcodeHookInstalled,
      gemini: demoGeminiHookInstalled,
      opencode: demoOpencodeHookInstalled,
    },
  };

  switch (mode) {
    case "approval":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: pending,
        recent: [pending],
        sessions: demoSessions,
      };
    case "compact":
    case "gif":
    case "sessions":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: pending,
        recent: [pending],
        sessions: demoSessions,
      };
    case "plan-question":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: planQuestion,
        recent: [planQuestion],
        sessions: demoPlanSessions,
      };
    case "plan-approval":
      return {
        ...base,
        pendingCount: 1,
        activeRequest: planApproval,
        recent: [planApproval],
        sessions: demoPlanSessions,
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

export function getDemoZcodeHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoZcodeHookMissing : demoZcodeHookInstalled;
}

export function getDemoGeminiHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoGeminiHookMissing : demoGeminiHookInstalled;
}

export function getDemoOpencodeHookStatus(mode: DemoMode): HookStatus {
  return mode === "idle" ? demoOpencodeHookMissing : demoOpencodeHookInstalled;
}

export function shouldAutoExpandDemo(mode: DemoMode): boolean {
  return (
    mode === "approval" ||
    mode === "sessions" ||
    mode === "idle" ||
    mode === "plan-question" ||
    mode === "plan-approval" ||
    mode === "fileStation"
  );
}

export function isFileStationDemoMode(): boolean {
  return getDemoMode() === "fileStation";
}

export function isGifCaptureMode(): boolean {
  return getDemoMode() === "gif";
}
