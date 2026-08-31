import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { computeCollapsedWindowWidth } from "./compactLayout";
import { markHookAgentConfigured } from "./hookAgentsConfigured";
import {
  IDLE_COLLAPSE_DELAY_MS,
  PANEL_EXIT_MS,
  RESOLVE_FEEDBACK_MS,
} from "./islandPresentation";
// Shared bridge lives in its own module so every App test file references the
// same mock objects; only the vi.mock declarations themselves are per-file.
import {
  appUpdateBridge,
  bridge,
  connectedHookHealth,
  emptyHookHealth,
  emptySnapshot,
  emitIslandHover,
  emitIslandOpen,
  emitPresentationSettled,
  emitSnapshot,
  emitSettledPhase,
  flushCollapseAnimation,
  flushPanelExit,
  makeSession,
  makeSubagent,
  planQuestionRequest,
  planSingleQuestionRequest,
  request,
  resetAppTestBridge,
  waitForExpandedPanel,
  windowBridge,
} from "./test-utils/appTestBridge";
import { App } from "./App";

vi.mock("./appUpdate", () => ({
  checkAppUpdate: (...args: unknown[]) => appUpdateBridge.checkAppUpdate(...args),
  installAppUpdate: (...args: unknown[]) => appUpdateBridge.installAppUpdate(...args),
  getAppVersion: (...args: unknown[]) => appUpdateBridge.getAppVersion(...args),
  UPDATE_INITIAL_DELAY_MS: 3_000,
  UPDATE_RECHECK_MS: 6 * 60 * 60 * 1000,
  isTauriUpdateRuntime: () => true,
}));

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
    resetAppTestBridge();
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
    await emitSettledPhase("expanded");

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

    await emitSettledPhase("compact");

    const compactAnimatedCalls = bridge.setIslandPresentation.mock.calls.filter(
      (call) => call[0] === "compact" && call[4] !== false,
    );
    expect(compactAnimatedCalls).toHaveLength(1);
    expect(compactAnimatedCalls[0]?.[1]).toBe(expectedCompactWidth);
    expect(container.querySelector(".is-compact")).not.toBeNull();
    vi.useRealTimers();
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
});
