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
});
