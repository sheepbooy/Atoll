import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { computeCollapsedWindowWidth } from "./compactLayout";

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
  setSessionAutoApprove: vi.fn(),
  archiveAllResolved: vi.fn(),
  archiveRequest: vi.fn(),
  getSessionRequests: vi.fn(),
  getSessionTranscript: vi.fn(),
  getNotchMetrics: vi.fn(),
  getSessionRetention: vi.fn(),
  setSessionRetention: vi.fn(),
}));

const windowBridge = vi.hoisted(() => ({
  startDragging: vi.fn(),
}));

const appUpdateBridge = vi.hoisted(() => ({
  checkAppUpdate: vi.fn(),
  installAppUpdate: vi.fn(),
}));

vi.mock("./appUpdate", () => ({
  checkAppUpdate: (...args: unknown[]) => appUpdateBridge.checkAppUpdate(...args),
  installAppUpdate: (...args: unknown[]) => appUpdateBridge.installAppUpdate(...args),
  UPDATE_INITIAL_DELAY_MS: 3_000,
  UPDATE_RECHECK_MS: 6 * 60 * 60 * 1000,
  isTauriUpdateRuntime: () => true,
}));

let emitIslandHover: ((state: { hovering: boolean }) => void) | null = null;
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
    bridge.getSessionRetention.mockResolvedValue(300);
    bridge.getNotchMetrics.mockResolvedValue({
      hasNotch: false,
      width: 0,
      height: 0,
    });
    bridge.setSessionRetention.mockResolvedValue(300);
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
    windowBridge.startDragging.mockResolvedValue(undefined);
    appUpdateBridge.checkAppUpdate.mockResolvedValue({ status: "idle" });
    appUpdateBridge.installAppUpdate.mockResolvedValue(undefined);
  });

  it("renders the command as compact code and contains no demo control", async () => {
    render(<App />);

    expect(await screen.findByText(request.command)).toHaveProperty("tagName", "CODE");
    expect(screen.queryByText("Demo")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/demo/i)).not.toBeInTheDocument();
  });

  it("collapses to a persistent capsule that can be reopened", async () => {
    const { container } = render(<App />);
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });

    vi.useFakeTimers();
    expect(fireEvent.mouseDown(collapseButton)).toBe(false);
    fireEvent.click(collapseButton);
    expect(collapseButton).not.toHaveFocus();
    expect(container.querySelector(".is-closing")).not.toBeNull();

    await vi.advanceTimersByTimeAsync(420);
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
    await vi.advanceTimersByTimeAsync(420);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      expect.any(Number),
      false,
      expect.any(Number),
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

    // Opening a session focuses the tapped button — this must NOT pin the
    // island open once the pointer leaves.
    await user.click(await screen.findByRole("button", { name: /project/i }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Back" })).toBeInTheDocument(),
    );

    fireEvent.pointerLeave(island);
    await waitFor(
      () => expect(container.querySelector(".is-compact")).not.toBeNull(),
      { timeout: 2000 },
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

    await vi.advanceTimersByTimeAsync(420);

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
    await vi.advanceTimersByTimeAsync(420);

    const collapseButton = screen.getByRole("button", { name: "Collapse Atoll" });
    fireEvent.click(collapseButton);
    emitIslandHover?.({ hovering: true });
    await vi.advanceTimersByTimeAsync(420);

    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "compact",
      expect.any(Number),
      undefined,
      expect.any(Number),
      false,
      true,
    );
    expect(container.querySelector(".is-compact")).not.toBeNull();

    emitIslandHover?.({ hovering: false });
    fireEvent.pointerEnter(screen.getByLabelText("Atoll"));
    await vi.advanceTimersByTimeAsync(420);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      expect.any(Number),
      false,
      expect.any(Number),
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
    fireEvent.click(approveButton);

    await waitFor(() => {
      expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
        "dormant",
        undefined,
        undefined,
        undefined,
        false,
        true,
      );
    });
    await waitFor(
      () => expect(container.querySelector(".is-dormant")).not.toBeNull(),
      { timeout: 1000 },
    );
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
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.clawd.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
  });

  it("shows dead agent mascot in header when one hook is uninstalled", async () => {
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
      },
    });
    const { container } = render(<App />);

    await waitFor(() => {
      expect(container.querySelector(".header-agent-logo.clawd.is-dead")).not.toBeNull();
    });
    expect(container.querySelector(".atoll-logo.is-dead")).toBeNull();
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

  it("shows update badge and menu action when an update is available", async () => {
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
    await waitFor(() =>
      expect(container.querySelector(".atoll-update-badge")).not.toBeNull(),
    );

    fireEvent.click(screen.getByRole("button", { name: /More options/i }));
    expect(
      screen.getByRole("menuitem", { name: /Update to v0.2.0/i }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /More options/i }).classList.contains("has-update"),
    ).toBe(true);
  });
});
