import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import { expect, vi } from "vitest";
import { clearConfiguredHookAgentsForTests } from "../hookAgentsConfigured";
import {
  PANEL_EXIT_MS,
  PRESENTATION_SETTLE_FALLBACK_MS,
} from "../islandPresentation";
import type {
  IslandSnapshot,
  SessionSummary,
  SubagentSummary,
} from "../tauri";

export async function flushPanelExit() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(PANEL_EXIT_MS);
  });
}

/** Emit the native settled event and flush the microtask chain it kicks off
 *  (the listener awaits the mocked `setIslandPresentation` promise). */
export async function emitSettledPhase(mode: string) {
  await act(async () => {
    await emitPresentationSettled?.(mode);
  });
}

export async function flushCollapseAnimation(mode: string = "compact") {
  await flushPanelExit();
  await emitSettledPhase(mode);
  // Drain the 2s fallback timer — it no-ops because the settled event already
  // ran and nulled the pending closure.
  await act(async () => {
    await vi.advanceTimersByTimeAsync(PRESENTATION_SETTLE_FALLBACK_MS);
  });
}

export const connectedHookHealth = {
  claude: {
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  },
  codex: {
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  },
  cursor: {
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  },
  zcode: {
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  },
  gemini: {
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  },
};

export const emptyHookHealth = {
  claude: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  },
  codex: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  },
  cursor: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  },
  zcode: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  },
  gemini: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  },
};

export const emptySnapshot = {
  online: false,
  pendingCount: 0,
  archivedCount: 0,
  activeRequest: null,
  recent: [],
  sessions: [],
  dailyTokens: {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
  },
  activeSessionTokens: {
    inputTokens: 0,
    outputTokens: 0,
    cacheReadTokens: 0,
    cacheCreationTokens: 0,
  },
  hookHealth: emptyHookHealth,
};

export async function waitForExpandedPanel(container: HTMLElement) {
  const island = screen.getByLabelText("Atoll");
  fireEvent.pointerEnter(island);
  await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
  await emitSettledPhase("expanded");
  await waitFor(() => expect(container.querySelector(".island-panel")).not.toBeNull(), {
    timeout: 1500,
  });
}

export const request = {
  id: "request-1",
  agent: "claude" as const,
  session: "session-1",
  command: "Bash: npm install --save-dev a-very-long-package-name",
  detail: "Install development dependencies.",
  cwd: "/tmp/project",
  requestedAt: "2026-06-10T08:00:00Z",
  status: "pending" as const,
};

export const planQuestionRequest = {
  ...request,
  id: "plan-question-1",
  command: "AskUserQuestion",
  detail: "Agent needs your input to continue planning.",
  toolInput: {
    questions: [
      {
        header: "Scope",
        question: "Which areas should we focus on first?",
        multiSelect: true,
        options: [
          { label: "Hook bridge", description: "Permission events and local HTTP bridge" },
          { label: "Plan mode UI", description: "Questions card and build approval preview" },
        ],
      },
    ],
  },
};

export const planSingleQuestionRequest = {
  ...request,
  id: "plan-question-2",
  command: "AskUserQuestion",
  detail: "Agent needs your input to continue planning.",
  toolInput: {
    questions: [
      {
        header: "Approach",
        question: "Which library should we use?",
        options: [
          { label: "rusqlite", description: "Bundled SQLite bindings" },
          { label: "sqlx", description: "Async queries with compile-time checks" },
        ],
      },
    ],
  },
};

export function makeSubagent(
  index: number,
  overrides: Partial<SubagentSummary> = {},
): SubagentSummary {
  const startedAt = new Date(
    Date.UTC(2026, 5, 10, 8, 0, 0) + index * 60_000,
  ).toISOString();
  const agentId = overrides.agentId ?? `subagent-${String(index).padStart(3, "0")}`;
  return {
    agentId,
    agentType:
      overrides.agentType ?? `worker-${String(index).padStart(3, "0")}`,
    startedAt,
    agentTranscriptPath: null,
    completedAt: null,
    archived: false,
    lastMessage: null,
    ...overrides,
  };
}

export function makeSession(
  activeSubagents: SubagentSummary[],
  overrides: Partial<SessionSummary> = {},
): SessionSummary {
  return {
    sessionId: "session-subagents",
    agent: "claude",
    cwd: "/tmp/subagent-project",
    pendingCount: 0,
    totalCount: 0,
    lastActivity: "2026-06-10T08:00:00Z",
    transcriptPath: null,
    pinned: false,
    sessionHost: "unknown",
    activeSubagents,
    ...overrides,
  };
}

export const bridge = {
  getSnapshot: vi.fn(),
  onSnapshotChanged: vi.fn(),
  onIslandHoverChanged: vi.fn(),
  onIslandOpenRequested: vi.fn(),
  onIslandPresentationSettled: vi.fn(),
  onCaptureCollapseRequested: vi.fn(),
  onCaptureOpenHooksRequested: vi.fn(),
  onCaptureScreenshotRequested: vi.fn(),
  captureProvideScreenshot: vi.fn(),
  quitAtoll: vi.fn(),
  deactivateAtoll: vi.fn(),
  resolvePermissionRequest: vi.fn(),
  resolvePermissionWithInput: vi.fn(),
  setIslandPresentation: vi.fn(),
  setImeActive: vi.fn(),
  setCompactLayout: vi.fn(),
  usesMicroIsland: vi.fn(),
  getClaudeHookStatus: vi.fn(),
  installClaudeHooks: vi.fn(),
  uninstallClaudeHooks: vi.fn(),
  getCodexHookStatus: vi.fn(),
  installCodexHooks: vi.fn(),
  uninstallCodexHooks: vi.fn(),
  getCursorHookStatus: vi.fn(),
  installCursorHooks: vi.fn(),
  uninstallCursorHooks: vi.fn(),
  setSessionAutoApprove: vi.fn(),
  archiveAllResolved: vi.fn(),
  archiveRequest: vi.fn(),
  archiveSubagent: vi.fn(),
  archiveCompletedSubagents: vi.fn(),
  getSessionRequests: vi.fn(),
  getSessionTranscript: vi.fn(),
  getSessionChat: vi.fn(),
  getNotchMetrics: vi.fn(),
  getSessionRetention: vi.fn(),
  setSessionRetention: vi.fn(),
  openAgentApp: vi.fn(),
  isAutostartEnabled: vi.fn(),
  enableAutostart: vi.fn(),
  disableAutostart: vi.fn(),
};

export const windowBridge = {
  startDragging: vi.fn(),
};

export const appUpdateBridge = {
  checkAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(),
  getAppVersion: vi.fn(),
};

// Live bindings: `resetAppTestBridge` (same module) re-wires these to each
// test's mock listeners; test files import and read them directly.
export let emitIslandHover:
  | ((state: { hovering: boolean; cursorOverWindow: boolean }) => void)
  | null = null;
export let emitIslandOpen: ((source: "summon" | "focus") => void) | null = null;
export let emitSnapshot: ((snapshot: IslandSnapshot) => void) | null = null;
export let emitPresentationSettled: ((mode: string) => void) | null = null;

/** The shared `beforeEach` body for every App test file: real timers, clean
 *  localStorage, fresh emit wiring, and the default mock responses. */
export function resetAppTestBridge() {
  vi.useRealTimers();
  window.localStorage.clear();
  clearConfiguredHookAgentsForTests();
  emitIslandHover = null;
  emitIslandOpen = null;
  emitSnapshot = null;
  emitPresentationSettled = null;
  bridge.getSnapshot.mockResolvedValue({
    online: true,
    pendingCount: 1,
    activeRequest: request,
    recent: [request],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  bridge.onSnapshotChanged.mockImplementation(async (callback) => {
    emitSnapshot = callback;
    return () => undefined;
  });
  bridge.onIslandHoverChanged.mockImplementation(async (callback) => {
    emitIslandHover = callback;
    return () => undefined;
  });
  bridge.onIslandOpenRequested.mockImplementation(async (callback) => {
    emitIslandOpen = callback;
    return () => undefined;
  });
  bridge.onIslandPresentationSettled.mockImplementation(async (callback) => {
    emitPresentationSettled = callback;
    return () => undefined;
  });
  bridge.onCaptureCollapseRequested.mockResolvedValue(() => undefined);
  bridge.onCaptureOpenHooksRequested.mockResolvedValue(() => undefined);
  bridge.onCaptureScreenshotRequested.mockResolvedValue(() => undefined);
  bridge.setIslandPresentation.mockResolvedValue(undefined);
  bridge.setImeActive.mockResolvedValue(undefined);
  bridge.setCompactLayout.mockResolvedValue(undefined);
  bridge.usesMicroIsland.mockResolvedValue(false);
  bridge.quitAtoll.mockResolvedValue(undefined);
  bridge.deactivateAtoll.mockResolvedValue(undefined);
  bridge.resolvePermissionRequest.mockResolvedValue({
    online: true,
    pendingCount: 0,
    activeRequest: null,
    recent: [{ ...request, status: "approved" }],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  bridge.resolvePermissionWithInput.mockResolvedValue({
    online: true,
    pendingCount: 0,
    activeRequest: null,
    recent: [{ ...request, status: "approved" }],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  bridge.getClaudeHookStatus.mockResolvedValue({
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.getCodexHookStatus.mockResolvedValue({
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.getCursorHookStatus.mockResolvedValue({
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.getSessionRetention.mockResolvedValue(300);
  bridge.getSessionChat.mockResolvedValue([]);
  bridge.openAgentApp.mockResolvedValue(undefined);
  bridge.getNotchMetrics.mockResolvedValue({
    hasNotch: false,
    width: 0,
    height: 0,
  });
  bridge.setSessionRetention.mockResolvedValue(300);
  bridge.isAutostartEnabled.mockResolvedValue(false);
  bridge.enableAutostart.mockResolvedValue(undefined);
  bridge.disableAutostart.mockResolvedValue(undefined);
  bridge.installClaudeHooks.mockResolvedValue({
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.uninstallClaudeHooks.mockResolvedValue({
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.installCodexHooks.mockResolvedValue({
    installed: true,
    scriptFound: true,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.uninstallCodexHooks.mockResolvedValue({
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
  });
  bridge.setSessionAutoApprove.mockResolvedValue(undefined);
  bridge.archiveAllResolved.mockResolvedValue({
    online: true,
    pendingCount: 0,
    activeRequest: null,
    recent: [],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  bridge.archiveSubagent.mockResolvedValue({
    online: true,
    pendingCount: 0,
    activeRequest: null,
    recent: [],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  bridge.archiveCompletedSubagents.mockResolvedValue({
    online: true,
    pendingCount: 0,
    activeRequest: null,
    recent: [],
    sessions: [],
    hookHealth: connectedHookHealth,
  });
  windowBridge.startDragging.mockResolvedValue(undefined);
  appUpdateBridge.checkAppUpdate.mockResolvedValue({ status: "idle" });
  appUpdateBridge.installAppUpdate.mockResolvedValue(undefined);
  appUpdateBridge.getAppVersion.mockResolvedValue("0.1.21");
}
