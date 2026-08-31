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
});
