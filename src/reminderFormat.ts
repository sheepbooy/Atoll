import type { Reminder, ReminderSnapshot } from "./tauri/reminders";
export function reminderTime(seconds: number): string {
  const s = Math.max(0, Math.ceil(seconds));
  if (s >= 3600)
    return `${Math.floor(s / 3600)}:${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}
export function nearestReminder(
  snapshot: ReminderSnapshot,
): Reminder | undefined {
  return snapshot.reminders
    .filter((r) => r.enabled && r.dueAt != null)
    .sort((a, b) => a.dueAt! - b.dueAt!)[0];
}
export function pendingReminderOccurrences(
  snapshot: ReminderSnapshot,
  now: number,
) {
  return snapshot.occurrences
    .filter(
      (o) =>
        o.completedAt == null &&
        (o.snoozedUntil == null || o.snoozedUntil <= now),
    )
    .sort((a, b) => a.dueAt - b.dueAt);
}
export function calendarLabel(
  at: number,
  language: string,
  now = Date.now(),
  withSeconds = false,
): string {
  const date = new Date(at * 1000);
  const today = new Date(now);
  const tomorrow = new Date(now);
  tomorrow.setDate(tomorrow.getDate() + 1);
  const zh = language.startsWith("zh");
  const time = date.toLocaleTimeString(zh ? "zh-CN" : "en-US", {
    hour: "2-digit",
    minute: "2-digit",
    ...(withSeconds ? { second: "2-digit" as const } : {}),
    hour12: false,
  });
  if (date.toDateString() === today.toDateString())
    return `${zh ? "今天" : "Today"} ${time}`;
  if (date.toDateString() === tomorrow.toDateString())
    return `${zh ? "明天" : "Tomorrow"} ${time}`;
  return `${date.toLocaleDateString(zh ? "zh-CN" : "en-US", { month: "short", day: "numeric", ...(date.getFullYear() !== today.getFullYear() ? { year: "numeric" as const } : {}) })} ${time}`;
}
