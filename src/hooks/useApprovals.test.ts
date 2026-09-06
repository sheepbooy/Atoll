import { act, fireEvent, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useApprovals } from "./useApprovals";
import { RESOLVE_FEEDBACK_MS } from "../islandPresentation";
import type { PanelView } from "../appTypes";
import type {
  IslandSnapshot,
  PermissionRequest,
} from "../tauri";

vi.mock("../tauri", () => ({
  deactivateAtoll: vi.fn(),
  getSessionRequests: vi.fn(),
  resolvePermissionRequest: vi.fn(),
  setSessionAutoApprove: vi.fn(),
}));

import {
  deactivateAtoll,
  getSessionRequests,
  resolvePermissionRequest,
  setSessionAutoApprove,
} from "../tauri";

const hookStatus = {
  installed: true,
  scriptFound: true,
  settingsPath: "/tmp/settings.json",
  scriptPath: "/tmp/atoll-claude-hook.mjs",
};

function makeRequest(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    id: "req-1",
    agent: "claude",
    session: "session-1",
    command: "npm install",
    detail: "",
    cwd: "/repo",
    requestedAt: new Date().toISOString(),
    status: "pending",
    ...overrides,
  };
}

function makeSnapshot(overrides: Partial<IslandSnapshot> = {}): IslandSnapshot {
  return {
    online: true,
    pendingCount: 0,
    archivedCount: 0,
    activeRequest: null,
    recent: [],
    sessions: [],
    dailyTokens: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 },
    activeSessionTokens: { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 },
    hookHealth: {
      claude: hookStatus,
      codex: hookStatus,
      cursor: hookStatus,
      zcode: hookStatus,
      gemini: hookStatus,
      opencode: hookStatus,
    },
    ...overrides,
  };
}

function buildOptions(overrides: Partial<Parameters<typeof useApprovals>[0]> = {}) {
  const snapshot = overrides.snapshot ?? makeSnapshot();
  return {
    snapshot,
    snapshotRef: { current: snapshot },
    panelView: { kind: "home" } as PanelView,
    selectedAgentRef: { current: null },
    menuOpenRef: { current: false },
    navigationSeqRef: { current: 0 },
    applySnapshot: vi.fn(),
    collapseIsland: vi.fn(),
    scheduleIdleCollapse: vi.fn(),
    setSessionRequests: vi.fn(),
    ...overrides,
  } as Parameters<typeof useApprovals>[0];
}

beforeEach(() => {
  vi.useFakeTimers();
  // restoreMocks resets implementations between tests, so (re)wire them here.
  vi.mocked(deactivateAtoll).mockResolvedValue(undefined);
  vi.mocked(getSessionRequests).mockResolvedValue([]);
  vi.mocked(resolvePermissionRequest).mockResolvedValue(makeSnapshot());
  vi.mocked(setSessionAutoApprove).mockResolvedValue(undefined);
});

afterEach(() => {
  vi.useRealTimers();
});

async function flushResolveFeedback() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(RESOLVE_FEEDBACK_MS + 50);
  });
}

describe("useApprovals", () => {
  describe("resolveActive", () => {
    it("resolves the request and applies the returned snapshot", async () => {
      const next = makeSnapshot({ pendingCount: 0 });
      vi.mocked(resolvePermissionRequest).mockResolvedValue(next);
      const options = buildOptions();
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });
      const request = makeRequest();

      let work = result.current.resolveActive(request, "approved");
      await flushResolveFeedback();
      await work;

      expect(resolvePermissionRequest).toHaveBeenCalledWith("req-1", "approved", "");
      expect(options.applySnapshot).toHaveBeenCalledWith(next);
      expect(result.current.busyDecision).toBeNull();
    });

    it("marks the busy decision while the resolve is in flight", async () => {
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions(),
      });

      let work;
      act(() => {
        work = result.current.resolveActive(makeRequest(), "denied");
      });
      expect(result.current.busyDecision).toBe("denied");
      await flushResolveFeedback();
      await work;

      expect(result.current.busyDecision).toBeNull();
    });

    it("turns on session auto-approve for always approvals", async () => {
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions(),
      });

      let work = result.current.resolveActive(makeRequest(), "approved", true);
      await flushResolveFeedback();
      await work;

      expect(setSessionAutoApprove).toHaveBeenCalledWith("session-1", true);
    });

    it("does not touch auto-approve for plain approvals", async () => {
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions(),
      });

      let work = result.current.resolveActive(makeRequest(), "approved");
      await flushResolveFeedback();
      await work;

      expect(setSessionAutoApprove).not.toHaveBeenCalled();
    });

    it("collapses the island and deactivates when the queue drains", async () => {
      const next = makeSnapshot({ pendingCount: 0 });
      vi.mocked(resolvePermissionRequest).mockResolvedValue(next);
      const options = buildOptions();
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });
      const request = makeRequest();

      let work = result.current.resolveActive(request, "approved");
      await flushResolveFeedback();
      await work;

      expect(options.collapseIsland).toHaveBeenCalledWith(true);
      expect(deactivateAtoll).toHaveBeenCalledWith("claude", "session-1", "/repo");
    });

    it("keeps the island open when requests remain pending", async () => {
      const next = makeSnapshot({ pendingCount: 2 });
      vi.mocked(resolvePermissionRequest).mockResolvedValue(next);
      const options = buildOptions();
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });

      let work = result.current.resolveActive(makeRequest(), "approved");
      await flushResolveFeedback();
      await work;

      expect(options.collapseIsland).not.toHaveBeenCalled();
      expect(deactivateAtoll).not.toHaveBeenCalled();
    });

    it("does nothing without a request", async () => {
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions(),
      });

      await result.current.resolveActive(null, "approved");

      expect(resolvePermissionRequest).not.toHaveBeenCalled();
    });
  });

  describe("resolveRequest", () => {
    it("resolves by id without refreshing when not in a session panel", async () => {
      const request = makeRequest();
      const options = buildOptions({
        snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
      });
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });

      let work = result.current.resolveRequest("req-1", "denied");
      await flushResolveFeedback();
      await work;

      expect(resolvePermissionRequest).toHaveBeenCalledWith("req-1", "denied", "");
      expect(getSessionRequests).not.toHaveBeenCalled();
      expect(options.setSessionRequests).not.toHaveBeenCalled();
    });

    it("refreshes the session request list while viewing a session", async () => {
      const request = makeRequest();
      const refreshed = [makeRequest({ id: "req-2", status: "pending" })];
      vi.mocked(getSessionRequests).mockResolvedValue(refreshed);
      const options = buildOptions({
        snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
        panelView: { kind: "session", sessionId: "session-1" },
      });
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });

      let work = result.current.resolveRequest("req-1", "approved");
      await flushResolveFeedback();
      await work;

      expect(getSessionRequests).toHaveBeenCalledWith("session-1");
      expect(options.setSessionRequests).toHaveBeenCalledWith(refreshed);
    });

    it("schedules an idle collapse instead of collapsing immediately", async () => {
      const request = makeRequest();
      vi.mocked(resolvePermissionRequest).mockResolvedValue(makeSnapshot({ pendingCount: 0 }));
      const options = buildOptions({
        snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
      });
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });

      let work = result.current.resolveRequest("req-1", "approved");
      await flushResolveFeedback();
      await work;

      expect(options.scheduleIdleCollapse).toHaveBeenCalled();
      expect(options.collapseIsland).not.toHaveBeenCalled();
      expect(deactivateAtoll).toHaveBeenCalledWith("claude", "session-1", "/repo");
    });
  });

  describe("keyboard shortcuts", () => {
    function keyOnDocument(key: string, shift = false) {
      fireEvent.keyDown(document, { key, shiftKey: shift });
    }

    it("approves the active request with Enter", async () => {
      const request = makeRequest();
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
        }),
      });

      keyOnDocument("Enter");
      await flushResolveFeedback();

      expect(resolvePermissionRequest).toHaveBeenCalledWith("req-1", "approved", "");
    });

    it("always-approves with Shift+Enter", async () => {
      const request = makeRequest();
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
        }),
      });

      keyOnDocument("Enter", true);
      await flushResolveFeedback();

      expect(setSessionAutoApprove).toHaveBeenCalledWith("session-1", true);
    });

    it("denies with Backspace", async () => {
      const request = makeRequest();
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
        }),
      });

      keyOnDocument("Backspace");
      await flushResolveFeedback();

      expect(resolvePermissionRequest).toHaveBeenCalledWith("req-1", "denied", "");
    });

    it("targets the selected agent's pending request over the active one", async () => {
      const claudeRequest = makeRequest({ id: "req-claude", agent: "claude" });
      const codexRequest = makeRequest({ id: "req-codex", agent: "codex", cwd: "/codex" });
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({
            activeRequest: claudeRequest,
            pendingCount: 2,
            recent: [claudeRequest, codexRequest],
          }),
          selectedAgentRef: { current: "codex" },
        }),
      });

      keyOnDocument("Enter");
      await flushResolveFeedback();

      expect(resolvePermissionRequest).toHaveBeenCalledWith("req-codex", "approved", "");
      expect(deactivateAtoll).toHaveBeenCalledWith("codex", "session-1", "/codex");
    });

    it("ignores keys while a resolve is busy", async () => {
      const request = makeRequest();
      const { result } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({ activeRequest: request, pendingCount: 1 }),
        }),
      });

      let work;
      act(() => {
        work = result.current.resolveActive(request, "approved");
      });
      keyOnDocument("Enter");
      await flushResolveFeedback();
      await work;

      expect(resolvePermissionRequest).toHaveBeenCalledTimes(1);
    });

    it("ignores keys while the menu is open", async () => {
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({
            activeRequest: makeRequest(),
            pendingCount: 1,
          }),
          menuOpenRef: { current: true },
        }),
      });

      keyOnDocument("Enter");
      await flushResolveFeedback();

      expect(resolvePermissionRequest).not.toHaveBeenCalled();
    });

    it("ignores keys typed into inputs", async () => {
      renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({
          snapshot: makeSnapshot({
            activeRequest: makeRequest(),
            pendingCount: 1,
          }),
        }),
      });
      const input = document.createElement("input");
      document.body.appendChild(input);

      fireEvent.keyDown(input, { key: "Enter" });
      await flushResolveFeedback();

      expect(resolvePermissionRequest).not.toHaveBeenCalled();
    });
  });

  describe("justResolved flash", () => {
    it("flashes when the pending queue drains, then clears", async () => {
      const options = buildOptions({ snapshot: makeSnapshot({ pendingCount: 1 }) });
      const { result, rerender } = renderHook((props) => useApprovals(props), {
        initialProps: options,
      });
      expect(result.current.justResolved).toBe(false);

      rerender(buildOptions({ snapshot: makeSnapshot({ pendingCount: 0 }) }));
      expect(result.current.justResolved).toBe(true);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1400);
      });
      expect(result.current.justResolved).toBe(false);
    });

    it("does not flash when the queue stays empty", async () => {
      const { result, rerender } = renderHook((props) => useApprovals(props), {
        initialProps: buildOptions({ snapshot: makeSnapshot({ pendingCount: 0 }) }),
      });

      rerender(buildOptions({ snapshot: makeSnapshot({ pendingCount: 0 }) }));
      await act(async () => {
        await vi.advanceTimersByTimeAsync(100);
      });

      expect(result.current.justResolved).toBe(false);
    });
  });
});
