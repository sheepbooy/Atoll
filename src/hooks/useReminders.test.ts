import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { useReminders } from "./useReminders";
import {
  EMPTY_REMINDERS,
  getReminders,
  onRemindersChanged,
  onRemindersDue,
  type ReminderSnapshot,
} from "../tauri/reminders";
vi.mock("../tauri/reminders", async (original) => ({
  ...(await original<typeof import("../tauri/reminders")>()),
  getReminders: vi.fn(),
  onRemindersChanged: vi.fn(),
  onRemindersDue: vi.fn(),
}));
beforeEach(() => vi.resetAllMocks());
it("ignores an older IPC response arriving after a committed scheduler event", async () => {
  let changed!: (snapshot: ReminderSnapshot) => void;
  let resolve!: (snapshot: ReminderSnapshot) => void;
  vi.mocked(onRemindersChanged).mockImplementation(async (callback) => {
    changed = callback;
    return vi.fn();
  });
  vi.mocked(onRemindersDue).mockResolvedValue(vi.fn());
  vi.mocked(getReminders).mockReturnValue(
    new Promise((done) => {
      resolve = done;
    }),
  );
  const { result } = renderHook(useReminders);
  await waitFor(() => expect(getReminders).toHaveBeenCalledOnce());
  act(() =>
    changed({
      ...EMPTY_REMINDERS,
      revision: 2,
      settings: { sound: false, noticeMode: "notify" },
    }),
  );
  await act(async () => resolve({ ...EMPTY_REMINDERS, revision: 1 }));
  act(() => result.current.setReminders({ ...EMPTY_REMINDERS, revision: 1 }));
  expect(result.current.reminders.revision).toBe(2);
  expect(result.current.reminders.settings.sound).toBe(false);
});
it("unsubscribes a partial registration when the second listener fails", async () => {
  const stop = vi.fn();
  vi.mocked(onRemindersChanged).mockResolvedValue(stop);
  vi.mocked(onRemindersDue).mockRejectedValue(
    new Error("listener unavailable"),
  );
  const { result } = renderHook(useReminders);
  await waitFor(() =>
    expect(result.current.reminderError).toContain("listener unavailable"),
  );
  expect(stop).toHaveBeenCalledOnce();
  expect(getReminders).not.toHaveBeenCalled();
});
