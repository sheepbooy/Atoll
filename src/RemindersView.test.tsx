import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import { RemindersView } from "./RemindersView";
import { EMPTY_REMINDERS, type ReminderSnapshot } from "./tauri/reminders";
const mocks = vi.hoisted(() => ({
  create: vi.fn(),
  preview: vi.fn(),
  update: vi.fn(),
  settings: vi.fn(),
}));
vi.mock("./tauri/reminders", async (original) => ({
  ...(await original<typeof import("./tauri/reminders")>()),
  createReminder: mocks.create,
  previewReminder: mocks.preview,
  updateReminder: mocks.update,
  setReminderSettings: mocks.settings,
}));
const props = () => ({
  snapshot: EMPTY_REMINDERS,
  onChange: vi.fn(),
  focusToken: 0,
  visible: true,
  onCreated: vi.fn(),
  onAutostart: vi.fn(),
});
beforeEach(() => {
  vi.useFakeTimers();
  vi.clearAllMocks();
  mocks.create.mockResolvedValue(EMPTY_REMINDERS);
  mocks.update.mockResolvedValue(EMPTY_REMINDERS);
  mocks.preview.mockResolvedValue({
    title: "tea",
    schedule: { kind: "timer", seconds: 1200 },
    dueAt: Date.now() / 1000 + 1200,
  });
});
afterEach(() => vi.useRealTimers());
describe("quick reminders", () => {
  it("starts each preset in one click without a form", async () => {
    const p = props();
    render(<RemindersView {...p} />);
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "25 min" })),
    );
    expect(mocks.create).toHaveBeenCalledWith("", {
      kind: "timer",
      seconds: 1500,
    });
    expect(p.onCreated).toHaveBeenCalledOnce();
  });
  it("previews input and reparses on Enter without a second confirmation", async () => {
    render(<RemindersView {...props()} />);
    const input = screen.getByRole("textbox", { name: "Create a reminder" });
    fireEvent.change(input, { target: { value: "in 20 minutes tea" } });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    expect(screen.getByText(/tea ·/)).toBeInTheDocument();
    await act(async () => fireEvent.submit(input.closest("form")!));
    expect(mocks.preview).toHaveBeenCalledTimes(2);
    expect(mocks.create).toHaveBeenCalledWith("tea", {
      kind: "timer",
      seconds: 1200,
    });
    expect(input).toHaveValue("");
  });
  it("does not submit an IME confirmation key", async () => {
    render(<RemindersView {...props()} />);
    const input = screen.getByRole("textbox", { name: "Create a reminder" });
    fireEvent.change(input, { target: { value: "20分钟后喝水" } });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    expect(
      fireEvent.keyDown(input, {
        key: "Enter",
        isComposing: true,
        keyCode: 229,
      }),
    ).toBe(false);
    expect(mocks.create).not.toHaveBeenCalled();
  });
  it("retains input and shows persistence errors", async () => {
    const p = props();
    mocks.create.mockRejectedValue(new Error("disk full"));
    render(<RemindersView {...p} />);
    const input = screen.getByRole("textbox", { name: "Create a reminder" });
    fireEvent.change(input, { target: { value: "in 20 minutes tea" } });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    await act(async () => fireEvent.submit(input.closest("form")!));
    expect(input).toHaveValue("in 20 minutes tea");
    expect(screen.getByRole("alert")).toHaveTextContent("disk full");
    expect(p.onCreated).not.toHaveBeenCalled();
  });
  it("discards stale preview responses", async () => {
    let resolve: (v: unknown) => void = () => {};
    mocks.preview.mockImplementationOnce(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    render(<RemindersView {...props()} />);
    const input = screen.getByRole("textbox", { name: "Create a reminder" });
    fireEvent.change(input, { target: { value: "5m" } });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    fireEvent.change(input, { target: { value: "20m" } });
    await act(async () => vi.advanceTimersByTimeAsync(150));
    await act(async () =>
      resolve({
        title: "old",
        schedule: { kind: "timer", seconds: 300 },
        dueAt: 100,
      }),
    );
    expect(screen.queryByText(/old ·/)).not.toBeInTheDocument();
    expect(screen.getByText(/tea ·/)).toBeInTheDocument();
  });
  it("preserves a draft while hidden and restores focus after explicit reopening", () => {
    const p = props();
    const { rerender } = render(<RemindersView {...p} />);
    const input = screen.getByRole("textbox", { name: "Create a reminder" });
    fireEvent.change(input, { target: { value: "tomorrow meeting" } });
    rerender(<RemindersView {...p} visible={false} />);
    rerender(<RemindersView {...p} focusToken={1} />);
    expect(input).toHaveValue("tomorrow meeting");
    expect(input).toHaveFocus();
  });
  it("only refreshes a visible panel", () => {
    const p = {
      ...props(),
      snapshot: {
        ...EMPTY_REMINDERS,
        reminders: [
          {
            id: "timer",
            title: "",
            schedule: { kind: "timer" as const, seconds: 60 },
            enabled: true,
            dueAt: Date.now() / 1000 + 60,
            remainingSeconds: null,
          },
        ],
      },
    };
    const intervalSpy = vi.spyOn(window, "setInterval");
    const clearSpy = vi.spyOn(window, "clearInterval");
    const { rerender, unmount } = render(<RemindersView {...p} />);
    expect(intervalSpy).toHaveBeenCalledOnce();
    const count = clearSpy.mock.calls.length;
    rerender(<RemindersView {...p} visible={false} />);
    expect(clearSpy.mock.calls.length).toBe(count + 1);
    unmount();
  });
  it("completes or snoozes an occurrence and leaves its repeating definition untouched", async () => {
    const snapshot: ReminderSnapshot = {
      ...EMPTY_REMINDERS,
      reminders: [
        {
          id: "daily",
          title: "tea",
          schedule: { kind: "daily", hour: 15, minute: 0, weekdays: [] },
          dueAt: Date.now() / 1000 + 86400,
          enabled: true,
          remainingSeconds: null,
        },
      ],
      occurrences: [
        {
          id: "daily:1",
          reminderId: "daily",
          title: "tea",
          dueAt: Date.now() / 1000 - 1,
          completedAt: null,
          snoozedUntil: null,
          alerted: true,
        },
      ],
    };
    render(<RemindersView {...props()} snapshot={snapshot} />);
    fireEvent.click(screen.getByRole("tab", { name: /Due/ }));
    await act(async () =>
      fireEvent.click(screen.getByRole("button", { name: "In 5 min" })),
    );
    expect(mocks.update).toHaveBeenCalledWith("daily:1", "snooze", 5);
    expect(snapshot.reminders[0].enabled).toBe(true);
  });
});

it("resolves manually selected dates in the backend so DST behavior matches text input", async () => {
  const schedule = { kind: "once", at: 1_800_000_000 };
  mocks.preview.mockResolvedValue({ title: "", schedule, dueAt: schedule.at });
  render(<RemindersView {...props()} />);
  fireEvent.click(screen.getByRole("button", { name: "Pick date & time" }));
  fireEvent.change(screen.getByLabelText("Date"), {
    target: { value: "2026-03-08" },
  });
  fireEvent.change(screen.getByLabelText("Time"), {
    target: { value: "02:30" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Reminder title" }), {
    target: { value: "wake" },
  });
  await act(async () =>
    fireEvent.click(screen.getByRole("button", { name: "Create reminder" })),
  );
  expect(mocks.preview).toHaveBeenCalledWith("2026-03-08 02:30");
  expect(mocks.create).toHaveBeenCalledWith("wake", schedule, undefined);
});
