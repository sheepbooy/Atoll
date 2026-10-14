import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  IDLE_COLLAPSE_DELAY_MS,
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
  emitSnapshot,
  emitSettledPhase,
  flushCollapseAnimation,
  flushPanelExit,
  request,
  resetAppTestBridge,
  waitForExpandedPanel,
  windowBridge,
} from "./test-utils/appTestBridge";
import { App } from "./App";
import { EMPTY_REMINDERS } from "./tauri/reminders";

const reminderBridge = vi.hoisted(() => ({ getReminders: vi.fn() }));
vi.mock("./tauri/reminders", async importOriginal => ({
  ...await importOriginal<typeof import("./tauri/reminders")>(),
  getReminders: reminderBridge.getReminders,
  onRemindersChanged: async () => () => undefined,
  onRemindersDue: async () => () => undefined,
}));

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

const defaultUserAgent = navigator.userAgent;

describe("App", () => {
  afterEach(() => {
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: defaultUserAgent });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.useRealTimers();
  });
  beforeEach(() => {
    resetAppTestBridge();
    reminderBridge.getReminders.mockResolvedValue(EMPTY_REMINDERS);
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

    fireEvent.click(island);
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
        (call) => call[0].mode === "micro",
      ),
    ).toBe(true);
    expect(
      bridge.setIslandPresentation.mock.calls.some((call) => call[0].mode === "compact"),
    ).toBe(false);

    vi.useRealTimers();
    fireEvent.click(screen.getByLabelText("Atoll"));
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
    expect(bridge.setIslandPresentation.mock.calls[0]?.[0].mode).not.toBe("dormant");
    await waitFor(() =>
      expect(bridge.setIslandPresentation).toHaveBeenCalledWith(expect.objectContaining({ mode: "micro", compactWidth: 72, animate: expect.any(Boolean), snap: expect.any(Boolean) })
      ),
    );

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("ignores the legacy regular size on Windows without changing its stored value", async () => {
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

    expect(container.querySelector(".is-compact")).toBeNull();
    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(window.localStorage.getItem("atoll.foldedIslandSize")).toBe("regular");
    expect(bridge.setIslandPresentation.mock.calls[0]?.[0].mode).not.toBe("dormant");

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("hides the old folded size setting on Windows", async () => {
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

    expect(screen.queryByRole("switch", { name: /Small folded island/i })).not.toBeInTheDocument();

    bridge.setIslandPresentation.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Collapse Atoll" }));
    await waitFor(() =>
      expect(bridge.setIslandPresentation).toHaveBeenCalledWith(expect.objectContaining({ mode: "micro", compactWidth: 72 })),
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
      expect(container.querySelector(".listener-dot.online")).not.toBeNull(),
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

  it("keeps a small edge entry even when session token counts grow", async () => {
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
        (call) => call[0].mode === "micro",
      );
      expect(
        microCalls.some(
          (call) => typeof call[0].compactWidth === "number" && call[0].compactWidth === 72,
        ),
      ).toBe(true);
    });

    Object.defineProperty(navigator, "userAgent", {
      configurable: true,
      value: originalUserAgent,
    });
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });

  it("does not expand on DOM or native hover, but does expand on click", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: "Windows NT 10.0" });
    bridge.usesMicroIsland.mockResolvedValue(true);
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, hookHealth: connectedHookHealth });
    const { container } = render(<App />);
    await waitFor(() => expect(emitIslandHover).not.toBeNull());
    await waitFor(() => expect(container.querySelector(".is-windows-edge.is-micro")).not.toBeNull());
    bridge.setIslandPresentation.mockClear();
    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await act(async () => { emitIslandHover?.({ hovering: true, cursorOverWindow: true }); });
    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(bridge.setIslandPresentation.mock.calls.some(call => call[0].mode === "expanded")).toBe(false);
    fireEvent.click(screen.getByLabelText("Atoll"));
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    expect(bridge.setIslandPresentation).toHaveBeenCalledWith(expect.objectContaining({ mode: "expanded" }));
  });

  function windowsRuntime() {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    Object.defineProperty(navigator, "userAgent", { configurable: true, value: "Windows NT 10.0" });
    bridge.usesMicroIsland.mockResolvedValue(true);
  }

  it("keeps notify-mode approvals folded at startup and opens from the tray", async () => {
    windowsRuntime();
    bridge.getApprovalNoticeMode.mockResolvedValue("notify");
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, pendingCount: 1, activeRequest: request, recent: [request], hookHealth: connectedHookHealth });
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".pending-badge")).toHaveTextContent("1"));
    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(bridge.setIslandPresentation.mock.calls.some(call => call[0].mode === "expanded")).toBe(false);
    await act(async () => { emitIslandOpen?.("focus"); });
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
  });

  it.each(["notify", "interrupt"] as const)("honors %s mode for incoming approvals", async mode => {
    windowsRuntime();
    bridge.getApprovalNoticeMode.mockResolvedValue(mode);
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, hookHealth: connectedHookHealth });
    const { container } = render(<App />);
    await waitFor(() => expect(emitSnapshot).not.toBeNull());
    await act(async () => { emitSnapshot?.({ ...emptySnapshot, online: true, pendingCount: 1, activeRequest: request, recent: [request], hookHealth: connectedHookHealth }); });
    if (mode === "notify") {
      expect(container.querySelector(".pending-badge")).toHaveTextContent("1");
      expect(container.querySelector(".is-micro")).not.toBeNull();
    } else {
      await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    }
  });

  it("shows independent approval and reminder badges within the edge entry", async () => {
    windowsRuntime();
    bridge.getApprovalNoticeMode.mockResolvedValue("notify");
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, pendingCount: 1, activeRequest: request, recent: [request], hookHealth: connectedHookHealth });
    reminderBridge.getReminders.mockResolvedValue({ ...EMPTY_REMINDERS, occurrences: [{
      id: "occurrence-1", reminderId: "reminder-1", title: "Drink water", dueAt: Date.now() / 1000 - 60, completedAt: null, snoozedUntil: null, alerted: true,
    }] });
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".compact-reminder b")).toHaveTextContent("1"));
    expect(container.querySelector(".pending-badge")).toHaveTextContent("1");
    expect(container.querySelector(".is-micro")).not.toBeNull();
    expect(container.querySelector(".token-counter")).toBeNull();
    await waitFor(() => expect(bridge.setIslandPresentation).toHaveBeenCalledWith(expect.objectContaining({ mode: "micro", compactWidth: 104 })));
  });

  it("expands quickly for a native file drag despite disabling hover opening", async () => {
    windowsRuntime();
    let drag: ((kind: string, paths: string[], position: { x: number; y: number }) => void) | undefined;
    bridge.onIslandDragDropEvent.mockImplementation(async callback => { drag = callback; return () => undefined; });
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, hookHealth: connectedHookHealth });
    const { container } = render(<App />);
    await waitFor(() => expect(drag).toBeDefined());
    await act(async () => { drag?.("enter", ["C:\\tmp\\test.txt"], { x: 20, y: 10 }); });
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());
    expect(bridge.setIslandPresentation).toHaveBeenCalledWith(expect.objectContaining({ mode: "expanded", durationMs: 140 }));
  });

  it("holds a shortcut summon open and returns to the edge on a second summon", async () => {
    windowsRuntime();
    bridge.getSnapshot.mockResolvedValue({ ...emptySnapshot, online: true, hookHealth: connectedHookHealth });
    const { container } = render(<App />);
    await waitFor(() => expect(emitIslandOpen).not.toBeNull());
    await act(async () => { emitIslandOpen?.("summon"); });
    await emitSettledPhase("expanded");
    vi.useFakeTimers();
    fireEvent.pointerLeave(screen.getByLabelText("Atoll"));
    await act(async () => {
      emitIslandHover?.({ hovering: false, cursorOverWindow: false });
      await vi.advanceTimersByTimeAsync(IDLE_COLLAPSE_DELAY_MS + 50);
    });
    expect(container.querySelector(".is-expanded")).not.toBeNull();
    await act(async () => { emitIslandOpen?.("summon"); });
    await flushCollapseAnimation("micro");
    expect(container.querySelector(".is-windows-edge.is-micro")).not.toBeNull();
  });
});
