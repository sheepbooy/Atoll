import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { computeCollapsedWindowWidth } from "./compactLayout";
import {
  clearConfiguredHookAgentsForTests,
  markHookAgentConfigured,
} from "./hookAgentsConfigured";
import {
  COLLAPSE_ANIMATION_MS,
  PANEL_EXIT_MS,
  RESOLVE_FEEDBACK_MS,
} from "./islandPresentation";
import type { SessionSummary, SubagentSummary } from "./tauri";

async function flushPanelExit() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(PANEL_EXIT_MS);
  });
}

async function flushCollapseAnimation() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(PANEL_EXIT_MS + COLLAPSE_ANIMATION_MS);
  });
}

const connectedHookHealth = {
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
};

const emptyHookHealth = {
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
};

const emptySnapshot = {
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

async function waitForExpandedPanel(container: HTMLElement) {
  const island = screen.getByLabelText("Atoll");
  fireEvent.pointerEnter(island);
  await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
  await waitFor(() => expect(container.querySelector(".island-panel")).not.toBeNull(), {
    timeout: 1500,
  });
}

const request = {
  id: "request-1",
  agent: "claude" as const,
  session: "session-1",
  command: "Bash: npm install --save-dev a-very-long-package-name",
  detail: "Install development dependencies.",
  cwd: "/tmp/project",
  requestedAt: "2026-06-10T08:00:00Z",
  status: "pending" as const,
};

const planQuestionRequest = {
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

function makeSubagent(
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

function makeSession(
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

const bridge = vi.hoisted(() => ({
  getSnapshot: vi.fn(),
  onSnapshotChanged: vi.fn(),
  onIslandHoverChanged: vi.fn(),
  onIslandOpenRequested: vi.fn(),
  onCaptureCollapseRequested: vi.fn(),
  onCaptureOpenHooksRequested: vi.fn(),
  onCaptureScreenshotRequested: vi.fn(),
  captureProvideScreenshot: vi.fn(),
  quitAtoll: vi.fn(),
  deactivateAtoll: vi.fn(),
  resolvePermissionRequest: vi.fn(),
  setIslandPresentation: vi.fn(),
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
}));

const windowBridge = vi.hoisted(() => ({
  startDragging: vi.fn(),
}));

const appUpdateBridge = vi.hoisted(() => ({
  checkAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(),
  getAppVersion: vi.fn(),
}));

vi.mock("./appUpdate", () => ({
  checkAppUpdate: (...args: unknown[]) => appUpdateBridge.checkAppUpdate(...args),
  installAppUpdate: (...args: unknown[]) => appUpdateBridge.installAppUpdate(...args),
  getAppVersion: (...args: unknown[]) => appUpdateBridge.getAppVersion(...args),
  UPDATE_INITIAL_DELAY_MS: 3_000,
  UPDATE_RECHECK_MS: 6 * 60 * 60 * 1000,
  isTauriUpdateRuntime: () => true,
}));

let emitIslandHover: ((state: { hovering: boolean; cursorOverWindow: boolean }) => void) | null = null;
let emitSnapshot: ((snapshot: import("./tauri").IslandSnapshot) => void) | null =
  null;

vi.mock("./tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./tauri")>();
  return {
    ...actual,
    ...bridge,
  };
});
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowBridge,
}));

describe("App", () => {
  beforeEach(() => {
    vi.useRealTimers();
    window.localStorage.clear();
    clearConfiguredHookAgentsForTests();
    emitIslandHover = null;
    emitSnapshot = null;
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
    bridge.onIslandOpenRequested.mockResolvedValue(() => undefined);
    bridge.onCaptureCollapseRequested.mockResolvedValue(() => undefined);
    bridge.onCaptureOpenHooksRequested.mockResolvedValue(() => undefined);
    bridge.onCaptureScreenshotRequested.mockResolvedValue(() => undefined);
    bridge.setIslandPresentation.mockResolvedValue(undefined);
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
  });

  it("renders the command as compact code and contains no demo control", async () => {
    render(<App />);

    expect(await screen.findByText(request.command)).toHaveProperty("tagName", "CODE");
    expect(screen.queryByText("Demo")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/demo/i)).not.toBeInTheDocument();
  });

  it("expands taller for plan mode permission requests", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planQuestionRequest,
      recent: [planQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    expect(container.querySelector(".is-plan")).not.toBeNull();
    expect(screen.getByText("Plan questions")).toBeInTheDocument();
    expect(
      bridge.setIslandPresentation.mock.calls.some(
        (call) => call[0] === "expanded" && call[6] === true,
      ),
    ).toBe(true);
  });

  it("collapses to a persistent capsule that can be reopened", async () => {
    const { container } = render(<App />);
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });

    vi.useFakeTimers();
    expect(fireEvent.mouseDown(collapseButton)).toBe(false);
    fireEvent.click(collapseButton);
    expect(collapseButton).not.toHaveFocus();
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    });
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "compact",
      expect.any(Number),
      undefined,
      expect.any(Number),
      false,
      true,
    );
    expect(container.querySelector(".is-compact")).not.toBeNull();

    fireEvent.click(screen.getByLabelText("Atoll"));
    await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      expect.any(Number),
      false,
      expect.any(Number),
      true,
      false,
      false,
      false,
    );
    expect(container.querySelector(".is-expanded")).not.toBeNull();
    vi.useRealTimers();
  });

  it("still auto-collapses after the more button is clicked with a pointer", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() =>
      expect(container.querySelector(".is-expanded:not(.is-opening)")).not.toBeNull(),
    );

    const moreButton = screen.getByRole("button", { name: "More options" });
    await user.click(moreButton);
    expect(moreButton).not.toHaveFocus();

    fireEvent.pointerLeave(island);
    await waitFor(
      () => expect(container.querySelector(".is-compact")).not.toBeNull(),
      { timeout: 1500 },
    );
  });

  it("auto-collapses after leaving a session opened from the list", async () => {
    const session = {
      sessionId: "session-1",
      agent: "claude" as const,
      cwd: "/tmp/project",
      pendingCount: 0,
      totalCount: 2,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [session],
      hookHealth: connectedHookHealth,
    });
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);
    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument(),
    );

    fireEvent.pointerLeave(island);
    await waitFor(
      () => expect(container.querySelector(".is-compact")).not.toBeNull(),
      { timeout: 1500 },
    );
  });

  it("loads Cursor session detail from a known transcript path without resolving by session", async () => {
    const session = {
      sessionId: "cursor-session-1",
      agent: "cursor" as const,
      cwd: "/tmp/cursor-project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: "/tmp/cursor-project/transcript.jsonl",
      pinned: false,
      sessionHost: "cursorIde" as const,
      activeSubagents: [],
    };
    bridge.getSnapshot.mockResolvedValue({
      ...emptySnapshot,
      online: true,
      sessions: [session],
      hookHealth: connectedHookHealth,
    });
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([
      { role: "user", content: "hello cursor" },
    ]);
    bridge.getSessionChat.mockClear();
    bridge.getSessionTranscript.mockClear();
    const user = userEvent.setup();
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    await user.click(await screen.findByRole("button", { name: /cursor-project/i }));

    await waitFor(() =>
      expect(bridge.getSessionTranscript).toHaveBeenCalledWith(
        "/tmp/cursor-project/transcript.jsonl",
      ),
    );
    expect(bridge.getSessionChat).not.toHaveBeenCalled();
    expect(screen.getByText("hello cursor")).toBeInTheDocument();
  });

  it("opens the subagent list with counts, archive action, and detail navigation", async () => {
    const subagents = [
      makeSubagent(1, { agentType: "worker-alpha" }),
      makeSubagent(2, {
        agentType: "worker-beta",
        completedAt: "2026-06-10T08:04:00Z",
        lastMessage: "done",
      }),
      makeSubagent(3, { agentType: "worker-gamma" }),
    ];
    const session = makeSession(subagents);
    const snapshot = {
      ...emptySnapshot,
      online: true,
      sessions: [session],
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(snapshot);
    bridge.archiveCompletedSubagents.mockResolvedValue(snapshot);
    bridge.getSessionTranscript.mockResolvedValue([]);
    const user = userEvent.setup();
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    await user.click(screen.getByTitle("View all subagents"));

    expect(screen.getByText("Subagents (3)")).toBeInTheDocument();
    expect(screen.getByText("2 running")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /Archive completed/i }));
    expect(bridge.archiveCompletedSubagents).toHaveBeenCalledWith(
      "session-subagents",
    );

    await user.click(screen.getByRole("button", { name: /worker-alpha/i }));
    expect(
      screen.getByRole("heading", { name: "worker-alpha" }),
    ).toBeInTheDocument();
  });

  it("virtualizes large subagent lists while preserving scroll navigation", async () => {
    const subagents = Array.from({ length: 80 }, (_, index) =>
      makeSubagent(index),
    );
    const session = makeSession(subagents);
    bridge.getSnapshot.mockResolvedValue({
      ...emptySnapshot,
      online: true,
      sessions: [session],
      hookHealth: connectedHookHealth,
    });
    bridge.getSessionTranscript.mockResolvedValue([]);
    const user = userEvent.setup();
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    await user.click(screen.getByTitle("View all subagents"));

    expect(screen.getByText("Subagents (80)")).toBeInTheDocument();
    expect(
      container.querySelectorAll(".subagent-list-item").length,
    ).toBeLessThan(80);
    expect(screen.queryByText("worker-000")).not.toBeInTheDocument();

    const listBody = container.querySelector(".subagent-list-body");
    expect(listBody).not.toBeNull();
    fireEvent.scroll(listBody!, {
      target: { scrollTop: 80 * 52 },
    });

    await waitFor(() =>
      expect(screen.getByText("worker-000")).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: /worker-000/i }));
    expect(
      screen.getByRole("heading", { name: "worker-000" }),
    ).toBeInTheDocument();
  });

  it("collapses once after returning from a session even if tokens update mid-animation", async () => {
    const session = {
      sessionId: "session-1",
      agent: "claude" as const,
      cwd: "/tmp/project",
      pendingCount: 0,
      totalCount: 2,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    const lowTokens = {
      inputTokens: 100,
      outputTokens: 50,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const highTokens = {
      inputTokens: 50_000_000,
      outputTokens: 50_000_000,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const noNotch = { hasNotch: false, width: 0, height: 0 };
    const expectedCompactWidth = computeCollapsedWindowWidth(
      noNotch,
      1,
      3,
      lowTokens.inputTokens + lowTokens.outputTokens,
      0,
    );

    const baseSnapshot = {
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [session],
      dailyTokens: lowTokens,
      activeSessionTokens: lowTokens,
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(baseSnapshot);
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);

    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Back" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Collapse Atoll" })).toBeInTheDocument(),
    );

    await act(async () => {
      emitSnapshot?.({
        ...baseSnapshot,
        dailyTokens: highTokens,
        activeSessionTokens: lowTokens,
      });
    });

    bridge.setIslandPresentation.mockClear();
    bridge.setCompactLayout.mockClear();

    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await act(async () => {
      emitSnapshot?.({
        ...baseSnapshot,
        dailyTokens: {
          inputTokens: 99_000_000,
          outputTokens: 99_000_000,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
        activeSessionTokens: lowTokens,
      });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    });

    const compactAnimatedCalls = bridge.setIslandPresentation.mock.calls.filter(
      (call) => call[0] === "compact" && call[4] !== false,
    );
    expect(compactAnimatedCalls).toHaveLength(1);
    expect(compactAnimatedCalls[0]?.[1]).toBe(expectedCompactWidth);
    expect(container.querySelector(".is-compact")).not.toBeNull();
    vi.useRealTimers();
  });

  it("keeps compact width when opening Claude from a session subview", async () => {
    const session = {
      sessionId: "session-1",
      agent: "claude" as const,
      cwd: "/tmp/project",
      pendingCount: 0,
      totalCount: 2,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    const wideTokens = {
      inputTokens: 50_000_000,
      outputTokens: 50_000_000,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const noTokens = {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const noNotch = { hasNotch: false, width: 0, height: 0 };
    const expectedCompactWidth = computeCollapsedWindowWidth(
      noNotch,
      1,
      3,
      wideTokens.inputTokens + wideTokens.outputTokens,
      0,
    );

    const baseSnapshot = {
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [session],
      dailyTokens: wideTokens,
      activeSessionTokens: wideTokens,
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(baseSnapshot);
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);

    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open Claude" })).toBeInTheDocument(),
    );

    await act(async () => {
      emitSnapshot?.({
        ...baseSnapshot,
        activeSessionTokens: noTokens,
      });
    });

    bridge.setIslandPresentation.mockClear();
    bridge.setCompactLayout.mockClear();

    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Open Claude" }));
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();
    expect(bridge.openAgentApp).toHaveBeenCalledWith(
      "claude",
      "/tmp/project",
      "session-1",
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    });

    const compactAnimatedCalls = bridge.setIslandPresentation.mock.calls.filter(
      (call) => call[0] === "compact" && call[4] !== false,
    );
    expect(compactAnimatedCalls).toHaveLength(1);
    expect(compactAnimatedCalls[0]?.[1]).toBe(expectedCompactWidth);
    expect(container.querySelector(".is-compact")).not.toBeNull();
    vi.useRealTimers();
  });

  it("keeps compact when opening Cursor from a session subview on Windows micro island", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });

    const session = {
      sessionId: "session-cursor",
      agent: "cursor" as const,
      cwd: "/tmp/cursor-project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    const baseSnapshot = {
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [session],
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(baseSnapshot);
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);
    bridge.usesMicroIsland.mockResolvedValue(true);

    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

    await user.click(await screen.findByRole("button", { name: /cursor-project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open Cursor" })).toBeInTheDocument(),
    );

    bridge.setIslandPresentation.mockClear();

    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Open Cursor" }));
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();
    expect(bridge.openAgentApp).toHaveBeenCalledWith(
      "cursor",
      "/tmp/cursor-project",
      "session-cursor",
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    });

    expect(container.querySelector(".is-compact")).not.toBeNull();
    expect(container.querySelector(".is-micro")).toBeNull();
    expect(
      bridge.setIslandPresentation.mock.calls.some(
        (call) => call[0] === "compact" && call[5] === true,
      ),
    ).toBe(true);
    expect(
      bridge.setIslandPresentation.mock.calls.some((call) => call[0] === "micro"),
    ).toBe(false);

    bridge.setIslandPresentation.mockClear();
    emitIslandHover?.({ hovering: false, cursorOverWindow: false });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(container.querySelector(".is-compact")).not.toBeNull();
    expect(container.querySelector(".is-micro")).toBeNull();
    expect(
      bridge.setIslandPresentation.mock.calls.some((call) => call[0] === "micro"),
    ).toBe(false);

    vi.useRealTimers();
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("does not reopen from a stale hover event after manual collapse", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container } = render(<App />);

    await waitFor(() => expect(emitIslandHover).not.toBeNull());
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });
    await vi.advanceTimersByTimeAsync(420);

    fireEvent.click(collapseButton);
    emitIslandHover?.({ hovering: true, cursorOverWindow: true });
    await flushCollapseAnimation();

    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "compact",
      expect.any(Number),
      undefined,
      expect.any(Number),
      false,
      true,
    );
    expect(container.querySelector(".is-compact")).not.toBeNull();

    emitIslandHover?.({ hovering: false, cursorOverWindow: false });
    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      expect.any(Number),
      false,
      expect.any(Number),
      true,
      false,
      false,
      false,
    );
    vi.useRealTimers();
  });

  it("puts Quit Atoll in the more menu", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Quit Atoll" }));

    expect(bridge.quitAtoll).toHaveBeenCalledOnce();
  });

  it("keeps the full window-control surfaces out of the drag handler", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    render(<App />);

    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });
    const moreButton = screen.getByRole("button", { name: "More options" });

    expect(fireEvent.mouseDown(collapseButton, { button: 0 })).toBe(false);
    expect(fireEvent.mouseDown(moreButton, { button: 0 })).toBe(false);
    fireEvent.click(moreButton);

    const quitButton = screen.getByRole("menuitem", { name: "Quit Atoll" });
    expect(fireEvent.mouseDown(quitButton, { button: 0 })).toBe(false);
    expect(windowBridge.startDragging).not.toHaveBeenCalled();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("closes the more menu with Escape", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the more menu on an outside pointer press", async () => {
    render(<App />);

    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.pointerDown(document.body);

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("uses one shared header instead of duplicate close buttons", async () => {
    const { container } = render(<App />);

    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    expect(container.querySelectorAll(".island-header")).toHaveLength(1);
    expect(screen.queryByLabelText("Hide Atoll")).not.toBeInTheDocument();
  });

  it("automatically collapses after the final approval while still focused and hovered", async () => {
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");
    const approveButton = await screen.findByRole("button", { name: "Approve" });

    fireEvent.pointerEnter(island);
    fireEvent.focus(approveButton);
    bridge.setIslandPresentation.mockClear();

    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      fireEvent.click(approveButton);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(
          RESOLVE_FEEDBACK_MS + PANEL_EXIT_MS + COLLAPSE_ANIMATION_MS,
        );
      });
      expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
        "dormant",
        undefined,
        undefined,
        undefined,
        false,
        true,
      );
      expect(container.querySelector(".is-dormant")).not.toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not cancel opening when hover, focus, and click arrive together", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    fireEvent.focus(island);
    fireEvent.click(island);

    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
  });

  it("shows dead agent mascot in header when one installed hook drifts", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: {
        claude: {
          installed: true,
          scriptFound: false,
          settingsPath: "",
          scriptPath: "",
        },
        codex: connectedHookHealth.codex,
        cursor: connectedHookHealth.cursor,
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.clawd.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows dead cursor mascot in header when cursor hook drifts", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: {
        claude: connectedHookHealth.claude,
        codex: connectedHookHealth.codex,
        cursor: {
          installed: true,
          scriptFound: false,
          settingsPath: "",
          scriptPath: "",
        },
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.cursor-mascot.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows dead cursor mascot in header when cursor hook is not installed", async () => {
    markHookAgentConfigured("cursor");
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: {
        claude: connectedHookHealth.claude,
        codex: connectedHookHealth.codex,
        cursor: {
          installed: false,
          scriptFound: false,
          settingsPath: "",
          scriptPath: "",
        },
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.cursor-mascot.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows dead agent mascot in header when one hook is uninstalled", async () => {
    markHookAgentConfigured("claude");
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: {
        claude: {
          installed: false,
          scriptFound: true,
          settingsPath: "",
          scriptPath: "",
        },
        codex: connectedHookHealth.codex,
        cursor: connectedHookHealth.cursor,
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.clawd.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows dead cursor mascot when offline and cursor hook is missing", async () => {
    markHookAgentConfigured("cursor");
    bridge.getSnapshot.mockResolvedValue({
      online: false,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: {
        claude: connectedHookHealth.claude,
        codex: connectedHookHealth.codex,
        cursor: {
          installed: false,
          scriptFound: false,
          settingsPath: "",
          scriptPath: "",
        },
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.cursor-mascot.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-napping")).toBeNull();
  });

  it("shows dead atoll logo before first-time hook install", async () => {
    bridge.getSnapshot.mockResolvedValue(emptySnapshot);
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".atoll-logo.is-dead")).not.toBeNull();
    });
  });

  it("does not show dead atoll logo before hook health hydrates on startup", () => {
    bridge.getSnapshot.mockImplementation(
      () => new Promise(() => undefined),
    );
    const { container } = render(<App />);

    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows live atoll logo on startup when hooks are already connected", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);

    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
    await waitFor(() => {
      expect(container.querySelector(".atoll-logo.is-idle")).not.toBeNull();
    });
  });

  it("starts in micro mode on Windows without presenting dormant first", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    bridge.usesMicroIsland.mockResolvedValue(true);

    const { container } = render(<App />);

    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(bridge.setIslandPresentation.mock.calls[0]?.[0]).not.toBe("dormant");
    await waitFor(() =>
      expect(bridge.setIslandPresentation).toHaveBeenCalledWith(
        "micro",
        72,
        undefined,
        undefined,
        expect.any(Boolean),
        expect.any(Boolean),
        undefined,
        undefined,
      ),
    );

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("starts in compact mode on Windows when regular folded island is selected", async () => {
    window.localStorage.setItem("atoll.foldedIslandSize", "regular");
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    bridge.usesMicroIsland.mockResolvedValue(true);

    const { container } = render(<App />);

    expect(container.querySelector(".is-compact")).not.toBeNull();
    expect(container.querySelector(".is-micro")).toBeNull();
    expect(bridge.setIslandPresentation.mock.calls[0]?.[0]).not.toBe("dormant");

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("shows and persists the Windows small folded island setting", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    bridge.usesMicroIsland.mockResolvedValue(true);

    const { container } = render(<App />);
    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/i }));

    const toggle = await screen.findByRole("switch", {
      name: /Small folded island/i,
    });
    expect(toggle).toHaveAttribute("aria-checked", "true");

    fireEvent.click(toggle);

    await waitFor(() =>
      expect(window.localStorage.getItem("atoll.foldedIslandSize")).toBe(
        "regular",
      ),
    );
    expect(toggle).toHaveAttribute("aria-checked", "false");

    fireEvent.click(toggle);
    await waitFor(() =>
      expect(window.localStorage.getItem("atoll.foldedIslandSize")).toBe(
        "small",
      ),
    );
    expect(toggle).toHaveAttribute("aria-checked", "true");

    bridge.setIslandPresentation.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    await waitFor(() =>
      expect(bridge.setIslandPresentation).toHaveBeenCalledWith("micro", 72),
    );

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("does not show the folded island size setting outside Windows", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });

    const { container } = render(<App />);
    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/i }));

    await screen.findByRole("switch", { name: /Launch at login/i });
    expect(
      screen.queryByRole("switch", { name: /Small folded island/i }),
    ).not.toBeInTheDocument();
  });

  it("does not render the Windows micro listener dot while sessions are active", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.usesMicroIsland.mockResolvedValue(true);
    const activeTokens = {
      inputTokens: 100,
      outputTokens: 20,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const session = {
      sessionId: "cursor-auto-archive",
      agent: "cursor" as const,
      cwd: "/tmp/cursor-project",
      pendingCount: 0,
      totalCount: 0,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
      pinned: false,
      sessionHost: "cursorIde" as const,
      activeSubagents: [],
    };
    const baseSnapshot = {
      ...emptySnapshot,
      online: true,
      sessions: [session],
      dailyTokens: activeTokens,
      activeSessionTokens: activeTokens,
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(baseSnapshot);

    const { container } = render(<App />);
    await waitFor(() =>
      expect(container.querySelector(".is-micro")).not.toBeNull(),
    );
    expect(container.querySelector(".listener-dot")).toBeNull();

    await act(async () => {
      emitSnapshot?.({
        ...baseSnapshot,
        sessions: [],
        activeSessionTokens: emptySnapshot.activeSessionTokens,
      });
    });

    await waitFor(() =>
      expect(container.querySelector(".listener-dot")).toBeNull(),
    );

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("uses a wider micro island when an active session is present", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.usesMicroIsland.mockResolvedValue(true);
    const activeTokens = {
      inputTokens: 12_345,
      outputTokens: 678,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    };
    const session = {
      sessionId: "session-micro-width",
      agent: "claude" as const,
      cwd: "/tmp/project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
      pinned: false,
      sessionHost: "claudeCode" as const,
      activeSubagents: [],
    };
    bridge.getSnapshot.mockResolvedValue({
      ...emptySnapshot,
      online: true,
      sessions: [session],
      activeSessionTokens: activeTokens,
      hookHealth: connectedHookHealth,
    });

    render(<App />);

    await waitFor(() => {
      const microCalls = bridge.setIslandPresentation.mock.calls.filter(
        (call) => call[0] === "micro",
      );
      expect(
        microCalls.some(
          (call) => typeof call[1] === "number" && call[1] > 72,
        ),
      ).toBe(true);
    });

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("cancels micro shrink when the cursor re-enters before hover dwell completes on Windows", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const originalUserAgent = navigator.userAgent;
    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
    });
    bridge.usesMicroIsland.mockResolvedValue(true);
    const session = {
      sessionId: "session-shrink-race",
      agent: "claude" as const,
      cwd: "/tmp/project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [session],
      hookHealth: connectedHookHealth,
    });
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);

    const user = userEvent.setup();
    const { container } = render(<App />);
    await waitFor(() => expect(emitIslandHover).not.toBeNull());

    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );

    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Open Claude" })).toBeInTheDocument(),
    );

    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      fireEvent.click(screen.getByRole("button", { name: "Open Claude" }));
      await flushCollapseAnimation();
      expect(container.querySelector(".is-compact")).not.toBeNull();

      bridge.setIslandPresentation.mockClear();
      emitIslandHover?.({ hovering: false, cursorOverWindow: false });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(250);
      });
      emitIslandHover?.({ hovering: false, cursorOverWindow: true });
      await act(async () => {
        await vi.advanceTimersByTimeAsync(300);
      });

      expect(
        bridge.setIslandPresentation.mock.calls.some((call) => call[0] === "micro"),
      ).toBe(false);
    } finally {
      vi.useRealTimers();
    }

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("shows agent tab labels on non-notched expanded header", async () => {
    const multiAgentSnapshot = {
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [
        {
          sessionId: "session-claude",
          agent: "claude" as const,
          cwd: "/tmp/claude-project",
          pendingCount: 0,
          totalCount: 1,
          lastActivity: "2026-06-10T08:00:00Z",
          transcriptPath: null,
        },
        {
          sessionId: "session-codex",
          agent: "codex" as const,
          cwd: "/tmp/codex-project",
          pendingCount: 0,
          totalCount: 1,
          lastActivity: "2026-06-10T08:00:00Z",
          transcriptPath: null,
        },
      ],
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(multiAgentSnapshot);
    bridge.getNotchMetrics.mockResolvedValue({
      hasNotch: false,
      width: 0,
      height: 0,
    });
    const { container } = render(<App />);

    await waitForExpandedPanel(container);

    const tabbar = container.querySelector(".agent-tabbar");
    expect(tabbar).not.toBeNull();
    expect(tabbar?.classList.contains("is-compact")).toBe(false);
    expect(container.querySelector(".header-main.has-agent-tabs")).not.toBeNull();
    expect(container.querySelector(".atoll-indicator-wrap")).not.toBeNull();
    expect(tabbar?.textContent).toContain("Claude");
    expect(tabbar?.textContent).toContain("Codex");
  });

  it("hides agent tab labels on notched expanded header", async () => {
    const multiAgentSnapshot = {
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [
        {
          sessionId: "session-claude",
          agent: "claude" as const,
          cwd: "/tmp/claude-project",
          pendingCount: 0,
          totalCount: 1,
          lastActivity: "2026-06-10T08:00:00Z",
          transcriptPath: null,
        },
        {
          sessionId: "session-codex",
          agent: "codex" as const,
          cwd: "/tmp/codex-project",
          pendingCount: 0,
          totalCount: 1,
          lastActivity: "2026-06-10T08:00:00Z",
          transcriptPath: null,
        },
      ],
      hookHealth: connectedHookHealth,
    };
    bridge.getSnapshot.mockResolvedValue(multiAgentSnapshot);
    bridge.getNotchMetrics.mockResolvedValue({
      hasNotch: true,
      width: 200,
      height: 38,
      leftAreaWidth: 656,
      rightAreaWidth: 656,
    });
    const { container } = render(<App />);

    await waitForExpandedPanel(container);

    const tabbar = container.querySelector(".agent-tabbar");
    expect(tabbar).not.toBeNull();
    expect(tabbar?.classList.contains("is-compact")).toBe(true);
    expect(container.querySelector(".header-agent-tabs--compact")).not.toBeNull();
    expect(tabbar?.textContent).not.toContain("Claude");
    expect(tabbar?.textContent).not.toContain("Codex");
    expect(
      container.querySelectorAll(".agent-tab.is-compact[aria-label='Claude']"),
    ).toHaveLength(1);
    expect(
      container.querySelectorAll(".agent-tab.is-compact[aria-label='Codex']"),
    ).toHaveLength(1);
  });

  it("switches to the pending agent tab when a new approval arrives", async () => {
    const cursorSession = {
      sessionId: "session-cursor",
      agent: "cursor" as const,
      cwd: "/tmp/cursor-project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    const claudeSession = {
      sessionId: "session-claude",
      agent: "claude" as const,
      cwd: "/tmp/claude-project",
      pendingCount: 0,
      totalCount: 1,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
    };
    const idleSnapshot = {
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [cursorSession, claudeSession],
      dailyTokens: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 },
      activeSessionTokens: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 },
      hookHealth: connectedHookHealth,
    };
    const claudePending = {
      ...request,
      id: "claude-pending-1",
      agent: "claude" as const,
      session: "session-claude",
      command: "Bash: rm -rf /tmp/claude-scratch",
      cwd: "/tmp/claude-project",
    };
    bridge.getSnapshot.mockResolvedValue(idleSnapshot);
    bridge.getNotchMetrics.mockResolvedValue({
      hasNotch: false,
      width: 0,
      height: 0,
    });
    const user = userEvent.setup();
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    await user.click(screen.getByRole("button", { name: "Cursor" }));
    await waitFor(() => {
      expect(container.querySelector(".agent-tab.is-active[aria-label='Cursor']")).not.toBeNull();
    });

    await act(async () => {
      emitSnapshot?.({
        ...idleSnapshot,
        pendingCount: 1,
        activeRequest: claudePending,
        recent: [claudePending],
        sessions: [
          cursorSession,
          { ...claudeSession, pendingCount: 1 },
        ],
      });
    });

    await waitFor(() => {
      expect(container.querySelector(".agent-tab.is-active[aria-label='Claude']")).not.toBeNull();
    });
    expect(screen.getByText("Bash: rm -rf /tmp/claude-scratch")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
  });

  it("shows live logo after installing all hooks on first setup", async () => {
    bridge.getSnapshot.mockResolvedValue(emptySnapshot);
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /Open agent hooks/i }));
    await act(async () => {
      fireEvent.click(await screen.findByRole("button", { name: /Install all/i }));
    });

    await waitFor(() => expect(bridge.installClaudeHooks).toHaveBeenCalledOnce());
    await waitFor(() => {
      expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
    });
    expect(bridge.installCodexHooks).toHaveBeenCalledOnce();
  });

  it("releases focus after dragging so leaving can collapse the island", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    render(<App />);
    const island = screen.getByLabelText("Atoll");
    const header = screen.getByTitle("Hover to open");

    fireEvent.focus(island);
    await waitFor(() => expect(screen.getByTitle("Drag window")).toBeInTheDocument());
    fireEvent.mouseDown(header, { button: 0 });
    await waitFor(() => expect(windowBridge.startDragging).toHaveBeenCalledOnce());

    vi.useFakeTimers();
    fireEvent.pointerLeave(island);
    await vi.advanceTimersByTimeAsync(500);
    await vi.advanceTimersByTimeAsync(420);
    // No active sessions → super-collapses into the dormant drawer.
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "dormant",
      undefined,
      undefined,
      undefined,
      false,
      true,
    );
    vi.useRealTimers();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("shows menu action when an update is available", async () => {
    appUpdateBridge.checkAppUpdate.mockResolvedValue({
      status: "available",
      version: "0.2.0",
    });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    vi.useFakeTimers();
    const { container } = render(<App />);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3_000);
    });
    vi.useRealTimers();

    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    expect(
      screen.getByRole("menuitem", { name: /Update to v0.2.0/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /More options/i }).classList.contains("has-update"),
    ).toBe(true);
  });

  it("shows an up-to-date notice after manual update check", async () => {
    appUpdateBridge.checkAppUpdate.mockResolvedValue({ status: "idle" });
    appUpdateBridge.getAppVersion.mockResolvedValue("0.1.21");
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Check for updates/i }));

    await waitFor(() =>
      expect(container.querySelector(".update-notice-card")).not.toBeNull(),
    );
    expect(screen.getByRole("alertdialog")).toHaveTextContent("You're up to date");
    expect(screen.getByRole("alertdialog")).toHaveTextContent("v0.1.21");
  });

  it("shows launch at login in settings", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);

    await waitForExpandedPanel(container);
    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/i }));

    await waitFor(() =>
      expect(screen.getByRole("switch", { name: /Launch at login/i })).toBeInTheDocument(),
    );
    expect(bridge.isAutostartEnabled).toHaveBeenCalled();
  });

  it("merges session updates during closing transition", async () => {
    const cursorSession = {
      sessionId: "session-cursor-ask",
      agent: "cursor" as const,
      cwd: "/tmp/ask-project",
      pendingCount: 0,
      totalCount: 0,
      lastActivity: "2026-06-10T08:00:00Z",
      transcriptPath: null,
      pinned: false,
      sessionHost: "unknown" as const,
      activeSubagents: [],
    };
    const baseSnapshot = {
      online: true,
      pendingCount: 0,
      archivedCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      dailyTokens: emptySnapshot.dailyTokens,
      activeSessionTokens: emptySnapshot.activeSessionTokens,
      hookHealth: connectedHookHealth,
    };

    bridge.getSnapshot.mockResolvedValue(baseSnapshot);
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Collapse Atoll" })).toBeInTheDocument(),
    );

    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await act(async () => {
      emitSnapshot?.({
        ...baseSnapshot,
        sessions: [cursorSession],
      });
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(COLLAPSE_ANIMATION_MS);
    });
    vi.useRealTimers();

    await waitFor(() =>
      expect(container.querySelector(".compact-session-dot")).not.toBeNull(),
    );
  });
});
