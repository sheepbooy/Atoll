import {
  Bell,
  Check,
  Pause,
  Play,
  RotateCcw,
  Trash2,
  Pencil,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import type {
  Reminder,
  ReminderOccurrence,
  ReminderSnapshot,
} from "../tauri/reminders";
import { calendarLabel, reminderTime } from "../reminderFormat";
export function OccurrenceItems({
  occurrences,
  snapshot,
  busy,
  onAction,
}: {
  occurrences: ReminderOccurrence[];
  snapshot: ReminderSnapshot;
  busy: boolean;
  onAction: (
    id: string,
    action: "complete" | "snooze" | "restart",
    minutes?: number,
  ) => void;
}) {
  const { t, i18n } = useTranslation();
  return (
    <>
      {occurrences.map((o) => (
        <article className="reminder-item is-due" key={o.id}>
          <div className="reminder-item-copy">
            <strong>{o.title || t("reminders.timer")}</strong>
            <span>{calendarLabel(o.dueAt, i18n.language)}</span>
          </div>
          <div className="reminder-item-actions">
            <button
              type="button"
              disabled={busy}
              onClick={() => onAction(o.id, "complete")}
            >
              <Check size={13} />
              {t("reminders.done")}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onAction(o.id, "snooze", 5)}
            >
              {t("reminders.later", { minutes: 5 })}
            </button>
            <button
              type="button"
              disabled={busy}
              onClick={() => onAction(o.id, "snooze", 15)}
            >
              {t("reminders.later", { minutes: 15 })}
            </button>
            {snapshot.reminders.find((r) => r.id === o.reminderId)?.schedule
              .kind === "timer" && (
              <button
                type="button"
                disabled={busy}
                onClick={() => onAction(o.reminderId, "restart")}
              >
                <RotateCcw size={13} />
                {t("reminders.again")}
              </button>
            )}
          </div>
        </article>
      ))}
    </>
  );
}
export function ActiveReminderItem({
  reminder: r,
  now,
  busy,
  onEdit,
  onAction,
}: {
  reminder: Reminder;
  now: number;
  busy: boolean;
  onEdit: (r: Reminder) => void;
  onAction: (id: string, action: "pause" | "resume" | "delete") => void;
}) {
  const { t, i18n } = useTranslation();
  return (
    <article className={`reminder-item${r.enabled ? "" : " is-paused"}`}>
      <div className="reminder-item-copy">
        <strong>
          <Bell size={12} />
          {r.title || t("reminders.timer")}
        </strong>
        <span>
          {r.schedule.kind === "timer"
            ? reminderTime(
                r.enabled ? (r.dueAt ?? now) - now : (r.remainingSeconds ?? 0),
              )
            : r.dueAt != null
              ? calendarLabel(r.dueAt, i18n.language)
              : t("reminders.paused")}
          {(r.schedule.kind === "daily" || r.schedule.kind === "interval") &&
            ` · ${t("reminders.repeating")}`}
        </span>
      </div>
      <div className="reminder-item-actions">
        {r.schedule.kind !== "once" && (
          <button
            type="button"
            disabled={busy || (r.enabled && r.dueAt == null)}
            onClick={() => onAction(r.id, r.enabled ? "pause" : "resume")}
            aria-label={t(r.enabled ? "reminders.pause" : "reminders.resume")}
          >
            {r.enabled ? <Pause size={14} /> : <Play size={14} />}
          </button>
        )}
        <button
          type="button"
          disabled={busy}
          onClick={() => onEdit(r)}
          aria-label={t("reminders.edit")}
        >
          <Pencil size={14} />
        </button>
        <button
          type="button"
          disabled={busy}
          onClick={() => onAction(r.id, "delete")}
          aria-label={t("reminders.delete")}
        >
          <Trash2 size={14} />
        </button>
      </div>
    </article>
  );
}
