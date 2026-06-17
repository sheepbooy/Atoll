import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

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
  quitAtoll: vi.fn(),
  deactivateAtoll: vi.fn(),
  resolvePermissionRequest: vi.fn(),
  setIslandPresentation: vi.fn(),
  getClaudeHookStatus: vi.fn(),
  installClaudeHooks: vi.fn(),
  uninstallClaudeHooks: vi.fn(),
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

let emitIslandHover: ((state: { hovering: boolean }) => void) | null = null;

vi.mock("./tauri", () => bridge);
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => windowBridge,
}));

describe("App", () => {
  beforeEach(() => {
    vi.useRealTimers();
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: request,
      recent: [request],
      sessions: [],
    });
    bridge.onSnapshotChanged.mockResolvedValue(() => undefined);
    emitIslandHover = null;
    bridge.onIslandHoverChanged.mockImplementation(async (callback) => {
      emitIslandHover = callback;
      return () => undefined;
    });
    bridge.onIslandOpenRequested.mockResolvedValue(() => undefined);
    bridge.setIslandPresentation.mockResolvedValue(undefined);
    bridge.quitAtoll.mockResolvedValue(undefined);
    bridge.deactivateAtoll.mockResolvedValue(undefined);
    bridge.resolvePermissionRequest.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [{ ...request, status: "approved" }],
      sessions: [],
    });
    bridge.getClaudeHookStatus.mockResolvedValue({
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
    bridge.setSessionAutoApprove.mockResolvedValue(undefined);
    bridge.archiveAllResolved.mockResolvedValue({
      online: true,
      pendingCount: 0,
      activeRequest: null,
      recent: [],
      sessions: [],
    });
    windowBridge.startDragging.mockResolvedValue(undefined);
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
    );
    expect(container.querySelector(".is-compact")).not.toBeNull();

    fireEvent.click(screen.getByLabelText("Atoll"));
    await vi.advanceTimersByTimeAsync(420);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      undefined,
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
    });
    const user = userEvent.setup();
    const { container } = render(<App />);
    const island = screen.getByLabelText("Atoll");

    fireEvent.pointerEnter(island);
    await waitFor(() => expect(container.querySelector(".is-expanded")).not.toBeNull());

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

  it("does not reopen from a stale hover event after manual collapse", async () => {
    const { container } = render(<App />);
    const collapseButton = await screen.findByRole("button", { name: "Collapse Atoll" });

    vi.useFakeTimers();
    fireEvent.click(collapseButton);
    emitIslandHover?.({ hovering: true });
    await vi.advanceTimersByTimeAsync(420);

    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "compact",
      expect.any(Number),
    );
    expect(container.querySelector(".is-compact")).not.toBeNull();

    emitIslandHover?.({ hovering: false });
    emitIslandHover?.({ hovering: true });
    await vi.advanceTimersByTimeAsync(420);
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
      "expanded",
      undefined,
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
    fireEvent.click(approveButton);

    await waitFor(() => {
      expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith(
        "compact",
        expect.any(Number),
      );
    });
    await waitFor(
      () => expect(container.querySelector(".is-compact")).not.toBeNull(),
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
    // No active sessions → super-collapses into the dormant drawer.
    expect(bridge.setIslandPresentation).toHaveBeenLastCalledWith("dormant");
    vi.useRealTimers();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  });
});
