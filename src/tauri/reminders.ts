import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";
export type ReminderSchedule =
  | { kind: "timer" | "interval"; seconds: number }
  | { kind: "once"; at: number }
  | { kind: "daily"; hour: number; minute: number; weekdays: number[] };
export interface ReminderDraft {
  title: string;
  schedule: ReminderSchedule;
  dueAt: number;
}
export interface Reminder {
  id: string;
  title: string;
  schedule: ReminderSchedule;
  enabled: boolean;
  dueAt: number | null;
  remainingSeconds: number | null;
}
export interface ReminderOccurrence {
  id: string;
  reminderId: string;
  title: string;
  dueAt: number;
  completedAt: number | null;
  snoozedUntil: number | null;
  alerted: boolean;
}
export interface ReminderSettings {
  sound: boolean;
  noticeMode: "interrupt" | "notify";
}
export interface ReminderSnapshot {
  revision?: number;
  reminders: Reminder[];
  occurrences: ReminderOccurrence[];
  settings: ReminderSettings;
}
export interface ReminderDue {
  occurrences: ReminderOccurrence[];
  noticeMode: "interrupt" | "notify";
  fallbackSound: boolean;
}
export const EMPTY_REMINDERS: ReminderSnapshot = {
  revision: 0,
  reminders: [],
  occurrences: [],
  settings: { sound: true, noticeMode: "interrupt" },
};
export async function getReminders(): Promise<ReminderSnapshot> {
  return isTauriRuntime() ? invoke("get_reminders") : EMPTY_REMINDERS;
}
export async function previewReminder(input: string): Promise<ReminderDraft> {
  if (isTauriRuntime()) return invoke("preview_reminder", { input });
  throw new Error("missingTime");
}
export async function createReminder(
  title: string,
  schedule: ReminderSchedule,
  id?: string,
): Promise<ReminderSnapshot> {
  if (!isTauriRuntime()) throw new Error("Desktop app required");
  return invoke("create_reminder", { title, schedule, id: id ?? null });
}
export async function updateReminder(
  id: string,
  action: "pause" | "resume" | "restart" | "delete" | "complete" | "snooze",
  minutes?: number,
): Promise<ReminderSnapshot> {
  return invoke("update_reminder", { id, action, minutes: minutes ?? null });
}
export async function setReminderSettings(
  settings: ReminderSettings,
): Promise<ReminderSnapshot> {
  return invoke("set_reminder_settings", { settings });
}
export async function onRemindersChanged(
  callback: (snapshot: ReminderSnapshot) => void,
) {
  return isTauriRuntime()
    ? listen<ReminderSnapshot>("reminders-changed", (e) => callback(e.payload))
    : () => {};
}
export async function onRemindersDue(callback: (due: ReminderDue) => void) {
  return isTauriRuntime()
    ? listen<ReminderDue>("reminders-due", (e) => callback(e.payload))
    : () => {};
}
export async function onReminderCreateRequested(callback: () => void) {
  return isTauriRuntime()
    ? listen("reminder-create-requested", callback)
    : () => {};
}
