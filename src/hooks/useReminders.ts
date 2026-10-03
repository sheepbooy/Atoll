import { useCallback, useEffect, useRef, useState } from "react";
import {
  EMPTY_REMINDERS,
  getReminders,
  onRemindersChanged,
  onRemindersDue,
  type ReminderDue,
  type ReminderSnapshot,
} from "../tauri/reminders";
export function useReminders() {
  const [snapshot, setSnapshot] = useState<ReminderSnapshot>(EMPTY_REMINDERS);
  const [error, setError] = useState("");
  const [due, setDue] = useState<ReminderDue | null>(null);
  const setReminders = useCallback((value: ReminderSnapshot) => {
    setSnapshot((previous) =>
      (value.revision ?? 0) >= (previous.revision ?? 0) ? value : previous,
    );
  }, []);
  useEffect(() => {
    let active = true;
    let stopChanged: (() => void) | undefined;
    let stopDue: (() => void) | undefined;
    async function subscribe() {
      stopChanged = await onRemindersChanged((value) => {
        if (active) {
          setReminders(value);
          setError("");
        }
      });
      if (!active) {
        stopChanged();
        return;
      }
      stopDue = await onRemindersDue((value) => {
        if (active)
          setDue((previous) => ({
            ...value,
            occurrences: [
              ...(previous?.occurrences ?? []),
              ...value.occurrences,
            ].filter((o, i, all) => all.findIndex((x) => x.id === o.id) === i),
          }));
      });
      if (!active) {
        stopChanged();
        stopDue();
        return;
      }
      const value = await getReminders();
      if (active) setReminders(value);
    }
    void subscribe().catch((e) => {
      stopChanged?.();
      stopDue?.();
      if (active) setError(String(e));
    });
    return () => {
      active = false;
      stopChanged?.();
      stopDue?.();
    };
  }, []);
  return {
    reminders: snapshot,
    setReminders,
    reminderError: error,
    dueReminders: due,
    clearDueReminders: () => setDue(null),
  };
}
