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

  it("collapses to micro when opening Cursor from a session subview on Windows micro island", async () => {
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
    await emitSettledPhase("expanded");

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

    await emitSettledPhase("micro");

    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(container.querySelector(".is-compact")).toBeNull();
    expect(
      bridge.setIslandPresentation.mock.calls.some(
        (call) => call[0] === "micro" && call[5] === true,
      ),
    ).toBe(true);
    expect(
      bridge.setIslandPresentation.mock.calls.some((call) => call[0] === "compact"),
    ).toBe(false);

    vi.useRealTimers();
    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );
    await emitSettledPhase("expanded");

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
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
    fireEvent.click(
      await screen.findByRole("button", { name: /Island appearance/i }),
    );

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
    fireEvent.click(screen.getByRole("button", { name: /Island appearance/i }));
    await screen.findByText(/Folded icon limit/i);
    expect(
      screen.queryByRole("switch", { name: /Small folded island/i }),
    ).not.toBeInTheDocument();
  });

  it("shows listener dot without session logos in Windows micro island", async () => {
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
    await waitFor(() => {
      expect(container.querySelector(".is-micro")).not.toBeNull();
      expect(container.querySelector(".listener-dot")).not.toBeNull();
    });
    expect(container.querySelector(".compact-session-dot")).toBeNull();
    expect(container.querySelector(".header-agent-logo")).toBeNull();

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

  it("shows Atoll logo instead of dead agent mascot in Windows micro island", async () => {
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
      sessionId: "session-micro-dead-logo",
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
    await waitFor(() =>
      expect(container.querySelector(".is-micro")).not.toBeNull(),
    );
    expect(container.querySelector(".atoll-logo")).not.toBeNull();
    expect(container.querySelector(".header-agent-logo")).toBeNull();

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
    window.localStorage.setItem("atoll.foldedIslandSize", "regular");
    bridge.usesMicroIsland.mockResolvedValue(true);
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
      hookHealth: connectedHookHealth,
    });

    const { container } = render(<App />);
    await waitFor(() => expect(emitIslandHover).not.toBeNull());
    await waitFor(() =>
      expect(container.querySelector(".is-compact")).not.toBeNull(),
    );

    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await waitFor(() =>
      expect(container.querySelector(".is-expanded")).not.toBeNull(),
    );
    await emitSettledPhase("expanded");
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /More options/i })).toBeInTheDocument(),
    );

    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/i }));
    fireEvent.click(
      await screen.findByRole("button", { name: /Island appearance/i }),
    );
    const toggle = await screen.findByRole("switch", {
      name: /Small folded island/i,
    });
    fireEvent.click(toggle);
    await waitFor(() =>
      expect(toggle).toHaveAttribute("aria-checked", "true"),
    );

    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    await flushCollapseAnimation();
    expect(container.querySelector(".is-micro")).not.toBeNull();

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

    vi.useRealTimers();

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });
});
