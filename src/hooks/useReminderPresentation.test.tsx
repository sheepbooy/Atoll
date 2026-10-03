import { act, renderHook } from "@testing-library/react";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { useReminderPresentation } from "./useReminderPresentation";
import { EMPTY_REMINDERS, type ReminderDue } from "../tauri/reminders";
vi.mock("../tauri/reminders", async (original) => ({
  ...(await original<typeof import("../tauri/reminders")>()),
  onReminderCreateRequested: vi.fn(async () => () => {}),
}));
vi.mock("../tauri", () => ({
  onIslandHoverChanged: vi.fn(async () => () => {}),
  deactivateAtoll: vi.fn(async () => {}),
}));
const occurrence = {
  id: "1:1",
  reminderId: "1",
  title: "tea",
  dueAt: 1,
  completedAt: null,
  snoozedUntil: null,
  alerted: true,
};
function options() {
  return {
    due: null as ReminderDue | null,
    clearDue: vi.fn(),
    snapshot: { ...EMPTY_REMINDERS, occurrences: [occurrence] },
    pendingCount: 0,
    dragging: false,
    phase: "compact",
    panelViewRef: {
      current: { kind: "home" } as import("../appTypes").PanelView,
    },
    setPanelView: vi.fn(),
    expand: vi.fn(),
    resize: vi.fn(),
    collapse: vi.fn(),
    clearIdle: vi.fn(),
    holdRef: { current: false },
  };
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());
it("surfaces a due batch without navigating away and restores collapsed state after eight seconds", async () => {
  const o = options();
  const { result, rerender } = renderHook((p) => useReminderPresentation(p), {
    initialProps: o,
  });
  rerender({
    ...o,
    due: {
      occurrences: [occurrence],
      noticeMode: "interrupt",
      fallbackSound: false,
    },
  });
  expect(result.current.reminderAlertOpen).toBe(true);
  expect(o.expand).toHaveBeenCalledOnce();
  expect(o.setPanelView).not.toHaveBeenCalled();
  await act(async () => vi.advanceTimersByTimeAsync(8000));
  expect(result.current.reminderAlertOpen).toBe(false);
  expect(o.collapse).toHaveBeenCalledOnce();
});
it("defers behind approvals and drag, then displays once safe", () => {
  const o = options();
  const { result, rerender } = renderHook((p) => useReminderPresentation(p), {
    initialProps: { ...o, pendingCount: 1 },
  });
  rerender({
    ...o,
    pendingCount: 1,
    due: {
      occurrences: [occurrence],
      noticeMode: "interrupt",
      fallbackSound: false,
    },
  });
  expect(result.current.reminderAlertOpen).toBe(false);
  rerender({ ...o, dragging: true });
  expect(result.current.reminderAlertOpen).toBe(false);
  rerender(o);
  expect(result.current.reminderAlertOpen).toBe(true);
});
it("does not cover an active input; displays after focus leaves", async () => {
  const input = document.createElement("input");
  document.body.append(input);
  input.focus();
  const o = options();
  const { result, rerender } = renderHook((p) => useReminderPresentation(p), {
    initialProps: o,
  });
  rerender({
    ...o,
    due: {
      occurrences: [occurrence],
      noticeMode: "interrupt",
      fallbackSound: false,
    },
  });
  expect(result.current.reminderAlertOpen).toBe(false);
  await act(async () => input.blur());
  expect(result.current.reminderAlertOpen).toBe(true);
  input.remove();
});
it("notify-only mode retains badges without opening a card", () => {
  const o = options();
  const { result } = renderHook(() =>
    useReminderPresentation({
      ...o,
      due: {
        occurrences: [occurrence],
        noticeMode: "notify",
        fallbackSound: false,
      },
    }),
  );
  expect(result.current.reminderAlertOpen).toBe(false);
  expect(o.expand).not.toHaveBeenCalled();
});
it("holding a card cancels its automatic timeout", async () => {
  const o = options();
  const { result } = renderHook(() =>
    useReminderPresentation({
      ...o,
      due: {
        occurrences: [occurrence],
        noticeMode: "interrupt",
        fallbackSound: false,
      },
    }),
  );
  act(() => result.current.holdReminderAlert());
  await act(async () => vi.advanceTimersByTimeAsync(9000));
  expect(result.current.reminderAlertOpen).toBe(true);
});
