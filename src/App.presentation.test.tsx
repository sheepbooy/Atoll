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

  it("collapses to a persistent capsule that can be reopened", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });

    vi.useFakeTimers();
    expect(fireEvent.mouseDown(collapseButton)).toBe(false);
    fireEvent.click(collapseButton);
    expect(collapseButton).not.toHaveFocus();
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await emitSettledPhase("compact");
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
    await emitSettledPhase("expanded");
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
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");
    await waitFor(() =>
      expect(container.querySelector(".is-expanded:not(.is-opening)")).not.toBeNull(),
    );

    const moreButton = screen.getByRole("button", { name: "More options" });
    await user.click(moreButton);
    expect(moreButton).not.toHaveFocus();

    fireEvent.pointerLeave(island);
    await waitFor(
      () => expect(container.querySelector(".is-closing")).not.toBeNull(),
      { timeout: 1500 },
    );
    await emitSettledPhase("compact");
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
    await emitSettledPhase("expanded");

    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument(),
    );

    fireEvent.pointerLeave(island);
    await waitFor(
      () => expect(container.querySelector(".is-closing")).not.toBeNull(),
      { timeout: 1500 },
    );
    await emitSettledPhase("compact");
    await waitFor(
      () => expect(container.querySelector(".is-compact")).not.toBeNull(),
      { timeout: 1500 },
    );
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
    await emitSettledPhase("expanded");

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

    await emitSettledPhase("compact");

    const compactAnimatedCalls = bridge.setIslandPresentation.mock.calls.filter(
      (call) => call[0] === "compact" && call[4] !== false,
    );
    expect(compactAnimatedCalls).toHaveLength(1);
    expect(compactAnimatedCalls[0]?.[1]).toBe(expectedCompactWidth);
    expect(container.querySelector(".is-compact")).not.toBeNull();
    vi.useRealTimers();
  });

  it("does not reopen from a stale hover event after manual collapse", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const { container } = render(<App />);

    await waitFor(() => expect(emitIslandHover).not.toBeNull());
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );
    await emitSettledPhase("expanded");
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });

    fireEvent.click(collapseButton);
    emitIslandHover?.({ hovering: true, cursorOverWindow: true });
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();
    await emitSettledPhase("compact");

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
    await emitSettledPhase("expanded");
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

  it("summon hotkey toggles: holds the island open, then collapses on the second press", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);

    await waitFor(() => expect(emitIslandOpen).not.toBeNull());
    await waitFor(() =>
      expect(container.querySelector(".is-compact")).not.toBeNull(),
    );

    // First press → expand and HOLD: past the idle delay it must stay open.
    act(() => {
      emitIslandOpen?.("summon");
    });
    await emitSettledPhase("expanded");
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );
    await act(async () => {
      await vi.advanceTimersByTimeAsync(IDLE_COLLAPSE_DELAY_MS * 4);
    });
    expect(container.querySelector(".is-expanded")).not.toBeNull();

    // Second press while expanded → collapse.
    act(() => {
      emitIslandOpen?.("summon");
    });
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();
    await emitSettledPhase("compact");
    expect(container.querySelector(".is-compact")).not.toBeNull();

    vi.useRealTimers();
  });

  it("non-summon open requests still expand then idle-collapse", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);

    await waitFor(() => expect(emitIslandOpen).not.toBeNull());
    await waitFor(() =>
      expect(container.querySelector(".is-compact")).not.toBeNull(),
    );

    act(() => {
      emitIslandOpen?.("focus");
    });
    await emitSettledPhase("expanded");
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );

    // The idle collapse scheduled by a focus open pulls the island back in.
    await act(async () => {
      await vi.advanceTimersByTimeAsync(IDLE_COLLAPSE_DELAY_MS * 4);
    });
    await flushPanelExit();
    expect(container.querySelector(".is-closing")).not.toBeNull();
    await emitSettledPhase("compact");
    expect(container.querySelector(".is-compact")).not.toBeNull();

    vi.useRealTimers();
  });

  it("automatically collapses after the final approval while still focused and hovered", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");
    const island = screen.getByLabelText("Atoll");
    const approveButton = await screen.findByRole("button", { name: "Approve" });

    fireEvent.pointerEnter(island);
    fireEvent.focus(approveButton);
    bridge.setIslandPresentation.mockClear();

    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      fireEvent.click(approveButton);
      await act(async () => {
        await vi.advanceTimersByTimeAsync(RESOLVE_FEEDBACK_MS + PANEL_EXIT_MS);
      });
      await emitSettledPhase("dormant");
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
    await emitSettledPhase("expanded");
    fireEvent.mouseDown(header, { button: 0 });
    await waitFor(() => expect(windowBridge.startDragging).toHaveBeenCalledOnce());

    vi.useFakeTimers();
    fireEvent.pointerLeave(island);
    await vi.advanceTimersByTimeAsync(500);
    await flushPanelExit();
    await emitSettledPhase("dormant");
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
    await emitSettledPhase("expanded");
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
      await emitPresentationSettled?.("compact");
    });

    vi.useRealTimers();

    await waitFor(() =>
      expect(container.querySelector(".compact-session-dot")).not.toBeNull(),
    );
  });
});
