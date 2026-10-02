import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { THEATER_DURATION_MS, useAgentTheater } from "./useAgentTheater";
import type { LifecycleEvent } from "../tauri/agentEvents";
const mocks = vi.hoisted(() => ({ listener: null as null | ((event: LifecycleEvent) => void), unsubscribe: vi.fn() }));
vi.mock("../tauri/agentEvents", () => ({ onAgentLifecycleChanged: vi.fn(async cb => { mocks.listener = cb; return mocks.unsubscribe; }) }));
beforeEach(() => { vi.useFakeTimers(); mocks.unsubscribe.mockClear(); });
afterEach(() => vi.useRealTimers());
function emit(event: Partial<LifecycleEvent> = {}) {
  mocks.listener?.({ eventId: "one", agent: "codex", sessionId: "s", subagentId: null, kind: "started", occurredAt: Date.now(), ...event });
}
it("ignores history and duplicate event ids, and finishes within three seconds", async () => {
  const { result } = renderHook(() => useAgentTheater(false));
  await act(async () => {});
  act(() => emit({ eventId: "history", occurredAt: Date.now() - 1 }));
  expect(result.current).toBeNull();
  act(() => emit());
  expect(result.current?.event.kind).toBe("started");
  await act(async () => { await vi.advanceTimersByTimeAsync(THEATER_DURATION_MS); });
  expect(result.current).toBeNull();
  act(() => emit());
  expect(result.current).toBeNull();
});
it("replaces the current scene without accumulating an animation queue", async () => {
  const { result } = renderHook(() => useAgentTheater(false));
  await act(async () => {});
  act(() => emit());
  await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
  act(() => emit({ eventId: "two", kind: "turnEnded" }));
  expect(result.current?.event.kind).toBe("turnEnded");
  await act(async () => { await vi.advanceTimersByTimeAsync(THEATER_DURATION_MS); });
  expect(result.current).toBeNull();
});
it("approval and drag interrupt immediately and discarded events never replay", async () => {
  const { result, rerender } = renderHook(({ blocked }) => useAgentTheater(blocked), { initialProps: { blocked: false } });
  await act(async () => {});
  act(() => emit());
  rerender({ blocked: true });
  expect(result.current).toBeNull();
  act(() => emit({ eventId: "hidden", kind: "subagentStarted", subagentId: "child" }));
  rerender({ blocked: false });
  expect(result.current).toBeNull();
  act(() => emit({ eventId: "end", kind: "turnEnded" }));
  expect(result.current?.actors).toEqual([null, "child"]);
});
it("shows a departing teammate once and removes it from subsequent scenes", async () => {
  const { result, unmount } = renderHook(() => useAgentTheater(false));
  await act(async () => {});
  act(() => emit({ eventId: "join", kind: "subagentStarted", subagentId: "child" }));
  act(() => emit({ eventId: "leave", kind: "subagentEnded", subagentId: "child" }));
  expect(result.current?.actors).toEqual([null, "child"]);
  act(() => emit({ eventId: "end", kind: "turnEnded" }));
  expect(result.current?.actors).toEqual([null]);
  unmount();
  expect(mocks.unsubscribe).toHaveBeenCalledOnce();
});
