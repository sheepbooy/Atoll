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

  it("renders the command as compact code and contains no demo control", async () => {
    const { container } = render(<App />);
    await waitFor(() => expect(container.querySelector(".is-opening, .is-expanded")).not.toBeNull());
    await emitSettledPhase("expanded");

    expect(await screen.findByText(request.command)).toHaveProperty("tagName", "CODE");
    expect(screen.queryByText("Demo")).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/demo/i)).not.toBeInTheDocument();
  });

  it("expands taller for plan mode permission requests", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planQuestionRequest,
      recent: [planQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    expect(container.querySelector(".is-plan")).not.toBeNull();
    expect(screen.getByText("Plan questions")).toBeInTheDocument();
    expect(
      bridge.setIslandPresentation.mock.calls.some(
        (call) => call[0] === "expanded" && call[6] === true,
      ),
    ).toBe(true);
  });

  it("notifies native code when a plan answer field is focused", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planQuestionRequest,
      recent: [planQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    fireEvent.click(screen.getByText("Other..."));
    const input = container.querySelector(".plan-other-input");
    expect(input).not.toBeNull();
    fireEvent.focusIn(input!);
    expect(bridge.setImeActive).toHaveBeenCalledWith(true);

    fireEvent.focusOut(input!);
    expect(bridge.setImeActive).toHaveBeenCalledWith(false);
  });

  it("submits multi-select answers joined into a single string", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planQuestionRequest,
      recent: [planQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    fireEvent.click(screen.getByText("Hook bridge"));
    fireEvent.click(screen.getByText("Plan mode UI"));
    fireEvent.click(screen.getByText("Other..."));
    const otherInput = container.querySelector(".plan-other-input");
    expect(otherInput).not.toBeNull();
    fireEvent.change(otherInput!, { target: { value: "Something else entirely" } });

    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => expect(bridge.resolvePermissionWithInput).toHaveBeenCalled());
    const updatedInput = bridge.resolvePermissionWithInput.mock.calls[0][3];
    expect(updatedInput).toEqual({
      questions: planQuestionRequest.toolInput.questions,
      answers: {
        "Which areas should we focus on first?":
          "Hook bridge, Plan mode UI, Something else entirely",
      },
    });
  });

  it("maps a free-form reply onto each question's answer", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planQuestionRequest,
      recent: [planQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    fireEvent.click(screen.getByText("Reply freely instead"));
    const textarea = container.querySelector(".plan-free-response");
    expect(textarea).not.toBeNull();
    fireEvent.change(textarea!, {
      target: { value: "Do the hook bridge first, skip the rest" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => expect(bridge.resolvePermissionWithInput).toHaveBeenCalled());
    const updatedInput = bridge.resolvePermissionWithInput.mock.calls[0][3];
    expect(updatedInput).toEqual({
      questions: planQuestionRequest.toolInput.questions,
      answers: {
        "Which areas should we focus on first?": "Do the hook bridge first, skip the rest",
      },
    });
  });

  it("keeps single-select Other answers as plain strings", async () => {
    bridge.getSnapshot.mockResolvedValue({
      online: true,
      pendingCount: 1,
      activeRequest: planSingleQuestionRequest,
      recent: [planSingleQuestionRequest],
      sessions: [],
      hookHealth: connectedHookHealth,
    });
    const { container } = render(<App />);
    await waitForExpandedPanel(container);

    fireEvent.click(screen.getByText("Other..."));
    const otherInput = container.querySelector(".plan-other-input");
    expect(otherInput).not.toBeNull();
    fireEvent.change(otherInput!, { target: { value: "Neither, roll our own" } });

    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => expect(bridge.resolvePermissionWithInput).toHaveBeenCalled());
    const updatedInput = bridge.resolvePermissionWithInput.mock.calls[0][3];
    expect(updatedInput).toEqual({
      questions: planSingleQuestionRequest.toolInput.questions,
      answers: { "Which library should we use?": "Neither, roll our own" },
    });
  });
});
