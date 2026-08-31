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
