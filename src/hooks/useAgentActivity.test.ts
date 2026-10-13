import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useAgentActivity } from "./useAgentActivity";
import type { LifecycleEvent } from "../tauri/agentEvents";
const mocks = vi.hoisted(() => ({ listener: null as null | ((event: LifecycleEvent) => void), unsubscribe: vi.fn() }));
vi.mock("../tauri/agentEvents", () => ({ onAgentLifecycleChanged: vi.fn(async cb => { mocks.listener = cb; return mocks.unsubscribe; }) }));
beforeEach(() => { vi.useFakeTimers(); mocks.unsubscribe.mockClear(); });
afterEach(() => vi.useRealTimers());
const emit = (event: Partial<LifecycleEvent>) => mocks.listener?.({ eventId: "one", agent: "codex", sessionId: "s", subagentId: null, kind: "started", occurredAt: Date.now(), ...event });

it("keeps working after finite theater feedback and stops on a real turn ending", async () => {
  const { result, unmount } = renderHook(useAgentActivity);
  await act(async () => {});
  act(() => emit({}));
  expect(result.current.has("codex:s")).toBe(true);
  act(() => vi.advanceTimersByTime(4000));
  expect(result.current.has("codex:s")).toBe(true);
  act(() => emit({ eventId: "end", kind: "turnEnded" }));
  expect(result.current.has("codex:s")).toBe(false);
  unmount();
  expect(mocks.unsubscribe).toHaveBeenCalledOnce();
});

it("ignores historical, subagent and out-of-order events", async () => {
  const { result } = renderHook(useAgentActivity);
  await act(async () => {});
  const now = Date.now();
  act(() => emit({ occurredAt: now - 1 }));
  act(() => emit({ subagentId: "child", kind: "subagentStarted" }));
  expect(result.current.size).toBe(0);
  act(() => vi.advanceTimersByTime(100));
  act(() => emit({ kind: "turnEnded" }));
  act(() => emit({ occurredAt: now }));
  expect(result.current.size).toBe(0);
});
