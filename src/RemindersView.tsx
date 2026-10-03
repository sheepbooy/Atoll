import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Bell, CalendarDays, Check, X } from "lucide-react";
import {
  createReminder,
  previewReminder,
  setReminderSettings,
  updateReminder,
  type Reminder,
  type ReminderDraft,
  type ReminderSchedule,
  type ReminderSnapshot,
} from "./tauri/reminders";
import { calendarLabel, pendingReminderOccurrences } from "./reminderFormat";
import {
  ActiveReminderItem,
  OccurrenceItems,
} from "./components/ReminderItems";
import { SettingsToggle } from "./SettingsControls";

export interface RemindersViewProps {
  snapshot: ReminderSnapshot;
  onChange: (snapshot: ReminderSnapshot) => void;
  focusToken: number;
  visible: boolean;
  onCreated: () => void;
  onAutostart: () => void;
  loadError?: string;
}
export function RemindersView({
  snapshot,
  onChange,
  focusToken,
  visible,
  onCreated,
  onAutostart,
  loadError,
}: RemindersViewProps) {
  const { t, i18n } = useTranslation();
  const [input, setInput] = useState("");
  const [preview, setPreview] = useState<ReminderDraft | null>(null);
  const [parseError, setParseError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [manual, setManual] = useState(false);
  const [title, setTitle] = useState("");
  const [date, setDate] = useState("");
  const [time, setTime] = useState("");
  const [repeat, setRepeat] = useState("once");
  const [weekday, setWeekday] = useState(0);
  const [interval, setIntervalMinutes] = useState(45);
  const [duration, setDuration] = useState(25);
  const [editId, setEditId] = useState<string>();
  const [tab, setTab] = useState<"pending" | "active" | "completed">("active");
  const [now, setNow] = useState(() => Date.now() / 1000);
  const inputRef = useRef<HTMLInputElement>(null);
  const previewSeq = useRef(0);
  const lastFocus = useRef(-1);
  const ticking =
    visible &&
    tab === "active" &&
    snapshot.reminders.some(
      (r) => r.enabled && r.dueAt != null && r.schedule.kind === "timer",
    );
  useEffect(() => {
    if (!visible) return;
    setNow(Date.now() / 1000);
    if (!ticking) return;
    const timer = window.setInterval(() => setNow(Date.now() / 1000), 1000);
    return () => window.clearInterval(timer);
  }, [visible, ticking]);
  useEffect(() => {
    if (visible && lastFocus.current !== focusToken) {
      lastFocus.current = focusToken;
      inputRef.current?.focus();
    }
  }, [focusToken, visible]);
  useEffect(() => {
    const seq = ++previewSeq.current;
    setPreview(null);
    setParseError("");
    if (!input.trim() || manual) return;
    const timer = window.setTimeout(() => {
      previewReminder(input)
        .then((value) => {
          if (seq === previewSeq.current) setPreview(value);
        })
        .catch((e) => {
          if (seq === previewSeq.current)
            setParseError(String(e).replace(/^Error: /, ""));
        });
    }, 150);
    return () => {
      window.clearTimeout(timer);
      previewSeq.current++;
    };
  }, [input, manual]);
  const message = (e: unknown) => {
    const code = String(e).replace(/^Error: /, "");
    return t(`reminders.errors.${code}`, { defaultValue: code });
  };
  async function run(work: () => Promise<ReminderSnapshot>, created = false) {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError("");
    try {
      onChange(await work());
      if (created) {
        setInput("");
        setPreview(null);
        setManual(false);
        setEditId(undefined);
        setTitle("");
        setTab("active");
        onCreated();
      }
    } catch (e) {
      setError(message(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  async function manualSchedule(): Promise<ReminderSchedule> {
    if (repeat === "timer") return { kind: "timer", seconds: duration * 60 };
    if (repeat === "interval")
      return { kind: "interval", seconds: interval * 60 };
    const [hour, minute] = time.split(":").map(Number);
    if (!time || !Number.isInteger(hour) || !Number.isInteger(minute))
      throw new Error("invalidTime");
    if (repeat !== "once")
      return {
        kind: "daily",
        hour,
        minute,
        weekdays:
          repeat === "weekdays"
            ? [0, 1, 2, 3, 4]
            : repeat === "weekly"
              ? [weekday]
              : [],
      };
    if (!date) throw new Error("invalidTime");
    // Resolve local dates in Rust too, so DST rules match natural-language input.
    return (await previewReminder(`${date} ${time}`)).schedule;
  }
  async function submit() {
    if (manual) {
      await run(
        async () => createReminder(title, await manualSchedule(), editId),
        true,
      );
    } else {
      // Reparse at submission so a relative deadline is anchored to this Enter.
      await run(async () => {
        const value = await previewReminder(input);
        return createReminder(value.title, value.schedule);
      }, true);
    }
  }
  function beginEdit(r: Reminder) {
    setEditId(r.id);
    setManual(true);
    setTitle(r.title);
    setInput("");
    setError("");
    const schedule = r.schedule;
    if (schedule.kind === "timer") {
      setRepeat("timer");
      setDuration(schedule.seconds / 60);
    } else if (schedule.kind === "interval") {
      setRepeat("interval");
      setIntervalMinutes(schedule.seconds / 60);
    } else if (schedule.kind === "daily") {
      setRepeat(
        schedule.weekdays.length === 0
          ? "daily"
          : schedule.weekdays.length > 1
            ? "weekdays"
            : "weekly",
      );
      setWeekday(schedule.weekdays[0] ?? 0);
      setTime(
        `${String(schedule.hour).padStart(2, "0")}:${String(schedule.minute).padStart(2, "0")}`,
      );
    } else if (schedule.kind === "once") {
      const d = new Date(schedule.at * 1000);
      setRepeat("once");
      setDate(
        `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`,
      );
      setTime(
        `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`,
      );
    }
    inputRef.current?.focus();
  }
  const pending = pendingReminderOccurrences(snapshot, now);
  const active = snapshot.reminders
    .filter((r) => r.dueAt != null || !r.enabled)
    .sort((a, b) => (a.dueAt ?? Infinity) - (b.dueAt ?? Infinity));
  const snoozed = snapshot.occurrences.filter(
    (o) =>
      o.completedAt == null && o.snoozedUntil != null && o.snoozedUntil > now,
  );
  const completed = snapshot.occurrences
    .filter((o) => o.completedAt != null)
    .sort((a, b) => b.completedAt! - a.completedAt!);
  return (
    <div
      className="reminders-view"
      data-no-drag
      style={visible ? undefined : { display: "none" }}
    >
      <form
        className="reminder-composer"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <div className="reminder-input-row">
          <Bell size={18} />
          <input
            ref={inputRef}
            aria-label={t(
              manual ? "reminders.titleLabel" : "reminders.inputLabel",
            )}
            placeholder={t(
              manual ? "reminders.titlePlaceholder" : "reminders.placeholder",
            )}
            value={manual ? title : input}
            maxLength={500}
            onChange={(e) =>
              manual ? setTitle(e.target.value) : setInput(e.target.value)
            }
            onKeyDown={(e) => {
              if (
                e.key === "Enter" &&
                (e.nativeEvent.isComposing || e.keyCode === 229)
              )
                e.preventDefault();
            }}
          />
          <button
            type="submit"
            disabled={busy || (!manual && !preview)}
            aria-label={t("reminders.create")}
          >
            <Check size={18} />
          </button>
        </div>
        {!manual && (
          <>
            <div className="reminder-preview" aria-live="polite">
              {preview
                ? `${preview.title || t("reminders.timer")} · ${calendarLabel(preview.dueAt, i18n.language, Date.now(), true)}${preview.schedule.kind === "daily" || preview.schedule.kind === "interval" ? ` · ${t("reminders.repeating")}` : ""}`
                : input.trim() && parseError
                  ? message(parseError)
                  : t("reminders.inputHint")}
            </div>
            <div className="reminder-presets">
              {[5, 15, 25, 60].map((minutes) => (
                <button
                  key={minutes}
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    void run(
                      () =>
                        createReminder("", {
                          kind: "timer",
                          seconds: minutes * 60,
                        }),
                      true,
                    )
                  }
                >
                  {t("reminders.minutes", { minutes })}
                </button>
              ))}
              <button
                type="button"
                disabled={busy}
                onClick={() => {
                  setManual(true);
                  setRepeat("once");
                  const d = new Date();
                  setDate(
                    `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`,
                  );
                }}
              >
                <CalendarDays size={13} />
                {t("reminders.pickDate")}
              </button>
            </div>
          </>
        )}
        {manual && (
          <div className="reminder-manual">
            <select
              aria-label={t("reminders.repeatLabel")}
              value={repeat}
              onChange={(e) => setRepeat(e.target.value)}
            >
              {[
                "once",
                "daily",
                "weekdays",
                "weekly",
                "interval",
                ...(editId ? ["timer"] : []),
              ].map((value) => (
                <option key={value} value={value}>
                  {t(`reminders.repeat.${value}`)}
                </option>
              ))}
            </select>
            {repeat === "once" && (
              <input
                type="date"
                aria-label={t("reminders.date")}
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            )}
            {repeat === "weekly" && (
              <select
                aria-label={t("reminders.weekday")}
                value={weekday}
                onChange={(e) => setWeekday(Number(e.target.value))}
              >
                {[0, 1, 2, 3, 4, 5, 6].map((day) => (
                  <option key={day} value={day}>
                    {t(`reminders.days.${day}`)}
                  </option>
                ))}
              </select>
            )}
            {repeat === "timer" || repeat === "interval" ? (
              <label>
                {t("reminders.duration")}
                <input
                  type="number"
                  min={1}
                  max={527040}
                  value={repeat === "timer" ? duration : interval}
                  onChange={(e) =>
                    repeat === "timer"
                      ? setDuration(Number(e.target.value))
                      : setIntervalMinutes(Number(e.target.value))
                  }
                />
              </label>
            ) : (
              <input
                type="time"
                aria-label={t("reminders.time")}
                value={time}
                onChange={(e) => setTime(e.target.value)}
              />
            )}
            <button
              type="button"
              aria-label={t("reminders.cancelEdit")}
              onClick={() => {
                setManual(false);
                setEditId(undefined);
              }}
            >
              <X size={14} />
            </button>
          </div>
        )}
        {(error || loadError) && (
          <p className="reminder-error" role="alert">
            {error || `${t("reminders.loadError")}: ${loadError}`}
          </p>
        )}
      </form>
      <p className="reminder-running-hint">
        {t("reminders.runningHint")}{" "}
        <button type="button" onClick={onAutostart}>
          {t("reminders.autostart")}
        </button>
      </p>
      <div className="reminder-tabs" role="tablist">
        {(["pending", "active", "completed"] as const).map((value) => (
          <button
            key={value}
            role="tab"
            type="button"
            aria-selected={tab === value}
            onClick={() => setTab(value)}
          >
            {t(`reminders.tabs.${value}`)}{" "}
            <span>
              {value === "pending"
                ? pending.length
                : value === "active"
                  ? active.length + snoozed.length
                  : completed.length}
            </span>
          </button>
        ))}
      </div>
      <div className="reminder-list" role="tabpanel">
        {tab === "pending" && (
          <OccurrenceItems
            occurrences={pending}
            snapshot={snapshot}
            busy={busy}
            onAction={(id, action, minutes) =>
              void run(() => updateReminder(id, action, minutes))
            }
          />
        )}
        {tab === "active" && (
          <>
            {active.map((r) => (
              <ActiveReminderItem
                key={r.id}
                reminder={r}
                now={now}
                busy={busy}
                onEdit={beginEdit}
                onAction={(id, action) =>
                  void run(() => updateReminder(id, action))
                }
              />
            ))}
            {snoozed.map((o) => (
              <article className="reminder-item" key={o.id}>
                <div className="reminder-item-copy">
                  <strong>{o.title || t("reminders.timer")}</strong>
                  <span>
                    {t("reminders.snoozed")} ·{" "}
                    {calendarLabel(o.snoozedUntil!, i18n.language)}
                  </span>
                </div>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    void run(() => updateReminder(o.id, "complete"))
                  }
                >
                  {t("reminders.done")}
                </button>
              </article>
            ))}
          </>
        )}
        {tab === "completed" &&
          completed.map((o) => (
            <article className="reminder-item" key={o.id}>
              <div className="reminder-item-copy">
                <strong>{o.title || t("reminders.timer")}</strong>
                <span>{calendarLabel(o.completedAt!, i18n.language)}</span>
              </div>
              {snapshot.reminders.find((r) => r.id === o.reminderId)?.schedule
                .kind === "timer" && (
                <button
                  type="button"
                  disabled={busy}
                  onClick={() =>
                    void run(() => updateReminder(o.reminderId, "restart"))
                  }
                >
                  {t("reminders.again")}
                </button>
              )}
            </article>
          ))}
        {(tab === "pending"
          ? pending.length === 0
          : tab === "active"
            ? active.length + snoozed.length === 0
            : completed.length === 0) && (
          <div className="reminder-empty">{t(`reminders.empty.${tab}`)}</div>
        )}
      </div>
      <details className="reminder-settings">
        <summary>{t("reminders.settings")}</summary>
        <SettingsToggle
          label={t("reminders.sound")}
          desc={t("reminders.soundDesc")}
          checked={snapshot.settings.sound}
          disabled={busy}
          onChange={(sound) =>
            void run(() => setReminderSettings({ ...snapshot.settings, sound }))
          }
        />
        <SettingsToggle
          label={t("reminders.notifyOnly")}
          desc={t("reminders.notifyOnlyDesc")}
          checked={snapshot.settings.noticeMode === "notify"}
          disabled={busy}
          onChange={(enabled) =>
            void run(() =>
              setReminderSettings({
                ...snapshot.settings,
                noticeMode: enabled ? "notify" : "interrupt",
              }),
            )
          }
        />
      </details>
    </div>
  );
}
