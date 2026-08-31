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
});
