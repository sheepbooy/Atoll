import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { computeCompactPresentation, estimateCompactCounterWidths } from "./compactLayout";
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

  it.each([1, 2])("keeps the expanded width through session navigation with %i agents", async (agentCount) => {
    let actionsWidth = 180;
    vi.spyOn(HTMLElement.prototype, "scrollWidth", "get").mockImplementation(function (this: HTMLElement) {
      return this.classList.contains("agent-tabbar") ? 360 : 0;
    });
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("header-actions")) return actionsWidth;
      if (this.classList.contains("token-counter-wrap--expanded")) return 120;
      return 0;
    });
    const sessions = [
      { ...makeSession([makeSubagent(1, { agentType: "worker-alpha" }), makeSubagent(2)]), cwd: "/tmp/claude-project" },
      { ...makeSession([]), sessionId: "session-codex", agent: "codex" as const, cwd: "/tmp/codex-project" },
    ].slice(0, agentCount);
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, sessions, hookHealth: connectedHookHealth });
    bridge.getNotchMetrics.mockResolvedValue({ hasNotch: true, width: 200, height: 38 });
    bridge.getSessionRequests.mockResolvedValue([]);
    bridge.getSessionTranscript.mockResolvedValue([]);
    const { container } = render(<App />);
    await waitForExpandedPanel(container);
    const homeWings = { expandedWingLeft: agentCount > 1 ? 422 : 64, expandedWingRight: 325 };
    await waitFor(() => expect(bridge.setIslandPresentation).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "expanded", ...homeWings }),
    ));

    bridge.setIslandPresentation.mockClear();

    // Detail headers omit the home tabs/counter/actions. Even resize/settled
    // events must retain the home wings instead of measuring absent elements.
    for (let visit = 0; visit < 2; visit += 1) {
      fireEvent.click(screen.getByRole("button", { name: /claude-project/i }));
      await screen.findByRole("button", { name: "Back" });
      await act(async () => {
        fireEvent(window, new Event("resize"));
        emitPresentationSettled?.("expanded");
        await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      });
      expect(bridge.setIslandPresentation).not.toHaveBeenCalled();
      expect(JSON.parse(window.localStorage.getItem("atoll:expanded-wings")!)).toEqual({
        left: homeWings.expandedWingLeft, right: homeWings.expandedWingRight,
      });

      fireEvent.click(screen.getByRole("button", { name: "Back" }));
      expect(container.querySelector(".is-session-subview")).toBeNull();
      expect(container.querySelector(".header-actions")).not.toBeNull();
      expect(bridge.setIslandPresentation).not.toHaveBeenCalled();
    }
    expect(JSON.parse(window.localStorage.getItem("atoll:expanded-wings")!)).toEqual({
      left: homeWings.expandedWingLeft, right: homeWings.expandedWingRight,
    });

    fireEvent.click(screen.getByTitle("View all subagents"));
    expect(screen.getByText("Subagents (2)")).toBeInTheDocument();
    await act(async () => {
      fireEvent(window, new Event("resize"));
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    });
    expect(bridge.setIslandPresentation).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /worker-alpha/i }));
    expect(screen.getByRole("heading", { name: "worker-alpha" })).toBeInTheDocument();
    await act(async () => {
      fireEvent(window, new Event("resize"));
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    });
    expect(bridge.setIslandPresentation).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(bridge.setIslandPresentation).not.toHaveBeenCalled();

    // Keeping detail widths must not disable live sizing after returning home.
    actionsWidth = 300;
    fireEvent(window, new Event("resize"));
    await waitFor(() => expect(bridge.setIslandPresentation).toHaveBeenCalledWith(
      expect.objectContaining({ mode: "expanded", expandedWingRight: 445 }),
    ));
  });

  it("collapses directly to the latest compact width when opening Claude", async () => {
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
    const expectedCompactWidth = computeCompactPresentation(
      noNotch,
      1,
      3,
      wideTokens.inputTokens + wideTokens.outputTokens,
      0,
      { counterWidths: estimateCompactCounterWidths("tokens", wideTokens.inputTokens + wideTokens.outputTokens) },
    ).windowWidth;

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
      (call) => call[0].mode === "compact" && call[0].animate !== false,
    );
    expect(compactAnimatedCalls).toHaveLength(1);
    const latestWidth = computeCompactPresentation(noNotch, 1, 3, 0, 0, {
      counterWidths: estimateCompactCounterWidths("tokens", 0),
    }).windowWidth;
    expect(compactAnimatedCalls[0]?.[0].compactWidth).toBe(latestWidth);
    expect(latestWidth).toBeLessThan(expectedCompactWidth);
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
