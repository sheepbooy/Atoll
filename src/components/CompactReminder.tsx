import { useEffect, useState } from "react";
import { Bell, Timer } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { ReminderSnapshot } from "../tauri/reminders";
import {
  calendarLabel,
  nearestReminder,
  pendingReminderOccurrences,
  reminderTime,
} from "../reminderFormat";
export function CompactReminder({
  snapshot,
  small,
  onOpen,
}: {
  snapshot: ReminderSnapshot;
  small: boolean;
  onOpen: () => void;
}) {
  const { t, i18n } = useTranslation();
  const [now, setNow] = useState(() => Date.now() / 1000);
  const nearest = nearestReminder(snapshot);
  const pending = pendingReminderOccurrences(snapshot, now).length;
  const snoozed = snapshot.occurrences
    .filter((o) => o.completedAt == null && o.snoozedUntil != null)
    .sort((a, b) => a.snoozedUntil! - b.snoozedUntil!)[0];
  const nextAt = Math.min(
    nearest?.dueAt ?? Infinity,
    snoozed?.snoozedUntil ?? Infinity,
  );
  const active =
    snapshot.reminders.filter((r) => r.enabled && r.dueAt != null).length +
    snapshot.occurrences.filter(
      (o) => o.completedAt == null && o.snoozedUntil != null,
    ).length;
  useEffect(() => {
    if (!Number.isFinite(nextAt)) return;
    const timer = window.setInterval(
      () => setNow(Date.now() / 1000),
      nearest?.schedule.kind === "timer" ? 1000 : 30000,
    );
    return () => window.clearInterval(timer);
  }, [nextAt, nearest?.schedule.kind]);
  if (!active && !pending) return null;
  const name =
    nextAt === nearest?.dueAt
      ? nearest.title || t("reminders.timer")
      : snoozed?.title || t("reminders.timer");
  const label = pending
    ? t("reminders.pendingCount", { count: pending })
    : nearest?.schedule.kind === "timer" && nextAt === nearest.dueAt
      ? reminderTime(nextAt - now)
      : calendarLabel(nextAt, i18n.language);
  return (
    <button
      type="button"
      data-no-drag
      className={`compact-reminder${pending ? " has-unread" : ""}${small ? " is-small" : ""}`}
      title={`${pending ? label : name} · ${Number.isFinite(nextAt) ? calendarLabel(nextAt, i18n.language, Date.now(), true) : ""}`}
      aria-label={`${t("reminders.title")}: ${label}`}
      onClick={(e) => {
        e.stopPropagation();
        onOpen();
      }}
    >
      {pending ? <Bell size={13} /> : <Timer size={13} />}
      <span className="compact-reminder-label">{label}</span>
      {(pending || active > 1 || small) && (
        <b>{pending || (small ? active : `+${active - 1}`)}</b>
      )}
    </button>
  );
}
