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

  it("puts Quit Atoll in the more menu", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");

    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Quit Atoll" }));

    expect(bridge.quitAtoll).toHaveBeenCalledOnce();
  });

  it("keeps the full window-control surfaces out of the drag handler", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");

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
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");

    fireEvent.click(await screen.findByRole("button", { name: "More options" }));
    fireEvent.keyDown(document, { key: "Escape" });

    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the more menu on an outside pointer press", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");

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

  it("switches the settings language to Chinese", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");
    fireEvent.click(await screen.findByRole("button", { name: /More options/i }));
    fireEvent.click(screen.getByRole("menuitem", { name: /Settings/i }));

    const chineseButton = await screen.findByRole("button", { name: "中文" });
    fireEvent.click(chineseButton);

    await waitFor(() => {
      expect(screen.getByText("语言")).toBeInTheDocument();
      expect(screen.getByText("通用")).toBeInTheDocument();
    });
  });

  it("keeps island and clipboard controls behind settings subpages", async () => {
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
      screen.queryByRole("switch", { name: /Clipboard history/i }),
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Island appearance/i }));
    expect(
      await screen.findByText(/Folded icon limit/i),
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Settings$/i }));

    fireEvent.click(
      await screen.findByRole("button", { name: /History recording/i }),
    );
    expect(
      await screen.findByRole("switch", { name: /Clipboard history/i }),
    ).toBeInTheDocument();
  });
});
