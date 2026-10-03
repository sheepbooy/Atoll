//! Local reminder schedules and occurrence ledger. All mutations commit before events.
use chrono::{
    DateTime, Datelike, Duration as CDuration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ReminderSchedule {
    Timer {
        seconds: i64,
    },
    Once {
        at: i64,
    },
    Daily {
        hour: u32,
        minute: u32,
        weekdays: Vec<u32>,
    },
    Interval {
        seconds: i64,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDraft {
    pub title: String,
    pub schedule: ReminderSchedule,
    pub due_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    pub id: String,
    pub title: String,
    pub schedule: ReminderSchedule,
    pub enabled: bool,
    pub due_at: Option<i64>,
    pub remaining_seconds: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderOccurrence {
    pub id: String,
    pub reminder_id: String,
    pub title: String,
    pub due_at: i64,
    pub completed_at: Option<i64>,
    pub snoozed_until: Option<i64>,
    pub alerted: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReminderSettings {
    pub sound: bool,
    pub notice_mode: String,
}
impl Default for ReminderSettings {
    fn default() -> Self {
        Self {
            sound: true,
            notice_mode: "interrupt".into(),
        }
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderSnapshot {
    pub reminders: Vec<Reminder>,
    pub occurrences: Vec<ReminderOccurrence>,
    pub settings: ReminderSettings,
    pub revision: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderDue {
    pub occurrences: Vec<ReminderOccurrence>,
    pub notice_mode: String,
    pub fallback_sound: bool,
}

/// Resolve a missing DST time by advancing to the first available minute;
/// on a repeated hour always use its first occurrence.
fn resolve_local<T: TimeZone>(tz: &T, mut local: NaiveDateTime) -> Option<DateTime<T>> {
    for _ in 0..=180 {
        match tz.from_local_datetime(&local) {
            chrono::LocalResult::Single(at) => return Some(at),
            chrono::LocalResult::Ambiguous(a, b) => {
                return Some(if a.timestamp() <= b.timestamp() { a } else { b })
            }
            chrono::LocalResult::None => {}
        }
        local += CDuration::minutes(1);
    }
    None
}
fn next_calendar<T: TimeZone>(
    tz: &T,
    now: i64,
    hour: u32,
    minute: u32,
    weekdays: &[u32],
) -> Result<i64, String> {
    if hour > 23 || minute > 59 || weekdays.iter().any(|day| *day > 6) {
        return Err("invalidTime".into());
    }
    let today = tz
        .timestamp_opt(now, 0)
        .single()
        .ok_or("invalidTime")?
        .date_naive();
    for day in 0..=8 {
        let date = today + CDuration::days(day);
        if !weekdays.is_empty() && !weekdays.contains(&date.weekday().num_days_from_monday()) {
            continue;
        }
        let at = resolve_local(tz, date.and_hms_opt(hour, minute, 0).ok_or("invalidTime")?)
            .ok_or("invalidTime")?
            .timestamp();
        if at > now {
            return Ok(at);
        }
    }
    Err("invalidTime".into())
}
pub fn next_due<T: TimeZone>(schedule: &ReminderSchedule, now: i64, tz: &T) -> Result<i64, String> {
    match schedule {
        ReminderSchedule::Timer { seconds } | ReminderSchedule::Interval { seconds } => {
            if *seconds <= 0 || *seconds > 366 * 86400 {
                return Err("invalidDuration".into());
            }
            now.checked_add(*seconds)
                .ok_or_else(|| "invalidDuration".into())
        }
        ReminderSchedule::Once { at } => {
            if *at > now {
                Ok(*at)
            } else {
                Err("pastTime".into())
            }
        }
        ReminderSchedule::Daily {
            hour,
            minute,
            weekdays,
        } => next_calendar(tz, now, *hour, *minute, weekdays),
    }
}
fn number(raw: &str) -> Option<i64> {
    if let Ok(v) = raw.parse::<i64>() {
        return Some(v);
    }
    if raw == "半" {
        return None;
    }
    let digit = |c| match c {
        '零' | '〇' => Some(0),
        '一' => Some(1),
        '二' | '两' => Some(2),
        '三' => Some(3),
        '四' => Some(4),
        '五' => Some(5),
        '六' => Some(6),
        '七' => Some(7),
        '八' => Some(8),
        '九' => Some(9),
        _ => None,
    };
    if raw.contains('十') {
        let (a, b) = raw.split_once('十')?;
        if a.chars().count() > 1 || b.chars().count() > 1 {
            return None;
        }
        return Some(
            if a.is_empty() {
                1
            } else {
                digit(a.chars().next()?)?
            } * 10
                + if b.is_empty() {
                    0
                } else {
                    digit(b.chars().next()?)?
                },
        );
    }
    if raw.chars().count() == 1 {
        digit(raw.chars().next()?)
    } else {
        None
    }
}
fn re(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).expect("static reminder regex")
}
fn title(raw: &str) -> String {
    raw.trim_matches(|c: char| c.is_whitespace() || matches!(c, ',' | '，' | ':' | '：' | '-'))
        .to_string()
}

pub fn parse<T: TimeZone>(input: &str, now: i64, tz: &T) -> Result<ReminderDraft, String> {
    let input = input.trim();
    if input.is_empty() || input.chars().count() > 500 {
        return Err("missingTime".into());
    }
    let duration = re(
        r"(?i)^(?:(每隔|every)\s*|(?:in)\s+)?(半|\d+|[一二两三四五六七八九十]+)\s*(小时|分钟|秒钟|秒|hours?|hrs?|h|minutes?|mins?|m|seconds?|secs?|s)(?:以后|之后|后)?\s*(.*)$",
    );
    if let Some(c) = duration.captures(input) {
        if input[c.get(3).unwrap().end()..]
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic())
            || re(r"(?i)^(?:或|或者|or\b)").is_match(c[4].trim())
        {
            return Err("missingTime".into());
        }
        let unit = c[3].to_lowercase();
        let scale = if unit == "小时" || unit.starts_with('h') {
            3600
        } else if unit == "分钟" || unit.starts_with('m') {
            60
        } else {
            1
        };
        let seconds = if &c[2] == "半" {
            scale / 2
        } else {
            number(&c[2])
                .ok_or("invalidDuration")?
                .checked_mul(scale)
                .ok_or("invalidDuration")?
        };
        let schedule = if c.get(1).is_some() {
            ReminderSchedule::Interval { seconds }
        } else {
            ReminderSchedule::Timer { seconds }
        };
        return Ok(ReminderDraft {
            title: title(&c[4]),
            due_at: next_due(&schedule, now, tz)?,
            schedule,
        });
    }
    let local_now = tz.timestamp_opt(now, 0).single().ok_or("invalidTime")?;
    let mut rest = input.to_string();
    let mut weekdays: Option<Vec<u32>> = None;
    let mut date: Option<NaiveDate> = None;
    let prefix = re(
        r"(?i)^(每天|工作日|每周[一二三四五六日天]|daily|every day|weekdays|every (?:monday|tuesday|wednesday|thursday|friday|saturday|sunday))\s*(?:at\s+)?",
    );
    if let Some(m) = prefix.find(&rest) {
        let p = m
            .as_str()
            .trim()
            .trim_end_matches("at")
            .trim()
            .to_lowercase();
        weekdays = Some(if p == "工作日" || p == "weekdays" {
            vec![0, 1, 2, 3, 4]
        } else if p.starts_with("每周") {
            vec![match p.chars().last().unwrap() {
                '一' => 0,
                '二' => 1,
                '三' => 2,
                '四' => 3,
                '五' => 4,
                '六' => 5,
                _ => 6,
            }]
        } else if p.starts_with("every ") && p != "every day" {
            vec![match p.trim_start_matches("every ") {
                "monday" => 0,
                "tuesday" => 1,
                "wednesday" => 2,
                "thursday" => 3,
                "friday" => 4,
                "saturday" => 5,
                _ => 6,
            }]
        } else {
            vec![]
        });
        rest = rest[m.end()..].to_string();
    } else {
        let prefix = re(r"(?i)^(今天|明天|后天|today|tomorrow)\s*(?:at\s+)?");
        if let Some(c) = prefix.captures(&rest) {
            let offset = match c[1].to_lowercase().as_str() {
                "明天" | "tomorrow" => 1,
                "后天" => 2,
                _ => 0,
            };
            date = Some(local_now.date_naive() + CDuration::days(offset));
            rest = rest[c.get(0).unwrap().end()..].to_string();
        } else if let Some(c) = re(r"^(\d{4}-\d{2}-\d{2})[ T]+(?:at\s+)?").captures(&rest) {
            date = Some(NaiveDate::parse_from_str(&c[1], "%Y-%m-%d").map_err(|_| "invalidTime")?);
            rest = rest[c.get(0).unwrap().end()..].to_string();
        }
    }
    let clock = re(
        r"(?i)^(上午|早上|中午|下午|晚上)?\s*(\d{1,2}|[一二两三四五六七八九十]+)(?:(?:[:：](\d{2}))|(?:[点时](半|\d{1,2}|[一二三四五六七八九十]+)?(?:分)?))\s*(am|pm)?\s*(.*)$",
    );
    // English hour with am/pm, or a bare hour only when a recurrence/date was specified.
    let simple_clock = re(r"(?i)^(\d{1,2})\s*(am|pm)\b\s*(.*)$");
    let (mut hour, minute, period, tail) = if let Some(c) = clock.captures(&rest) {
        let minute = if let Some(m) = c.get(3) {
            number(m.as_str()).ok_or("invalidTime")?
        } else if let Some(m) = c.get(4) {
            if m.as_str() == "半" {
                30
            } else {
                number(m.as_str()).ok_or("invalidTime")?
            }
        } else {
            0
        };
        (
            number(&c[2]).ok_or("invalidTime")?,
            minute,
            c.get(1)
                .or_else(|| c.get(5))
                .map(|m| m.as_str().to_lowercase())
                .unwrap_or_default(),
            title(&c[6]),
        )
    } else if let Some(c) = simple_clock.captures(&rest) {
        (
            number(&c[1]).ok_or("invalidTime")?,
            0,
            c[2].to_lowercase(),
            title(&c[3]),
        )
    } else {
        return Err("missingTime".into());
    };
    if re(r"(?i)^(?:或|或者|or\b)").is_match(&tail) {
        return Err("missingTime".into());
    }
    if !period.is_empty() {
        if !(1..=12).contains(&hour) {
            return Err("invalidTime".into());
        }
        if matches!(period.as_str(), "下午" | "晚上" | "pm" | "中午") && hour < 12 {
            hour += 12;
        }
        if matches!(period.as_str(), "上午" | "早上" | "am") && hour == 12 {
            hour = 0;
        }
    }
    if !(0..24).contains(&hour) || !(0..60).contains(&minute) {
        return Err("invalidTime".into());
    }
    let schedule = if let Some(weekdays) = weekdays {
        ReminderSchedule::Daily {
            hour: hour as u32,
            minute: minute as u32,
            weekdays,
        }
    } else {
        let mut date = date.unwrap_or(local_now.date_naive());
        let mut at = resolve_local(
            tz,
            date.and_hms_opt(hour as u32, minute as u32, 0)
                .ok_or("invalidTime")?,
        )
        .ok_or("invalidTime")?
        .timestamp();
        let explicit_date = re(r"(?i)^(今天|明天|后天|today|tomorrow|\d{4}-)").is_match(input);
        if at <= now && !explicit_date {
            date += CDuration::days(1);
            at = resolve_local(tz, date.and_hms_opt(hour as u32, minute as u32, 0).unwrap())
                .ok_or("invalidTime")?
                .timestamp();
        }
        ReminderSchedule::Once { at }
    };
    Ok(ReminderDraft {
        title: tail,
        due_at: next_due(&schedule, now, tz)?,
        schedule,
    })
}

fn database_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("ATOLL_REMINDERS_PATH") {
        return Ok(path.into());
    }
    dirs::home_dir()
        .map(|p| p.join(".atoll/reminders.db"))
        .ok_or_else(|| "No home directory".into())
}
fn connect(path: &std::path::Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS reminders (id TEXT PRIMARY KEY, data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS occurrences (id TEXT PRIMARY KEY, data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), data TEXT NOT NULL); CREATE TABLE IF NOT EXISTS reminder_meta (id INTEGER PRIMARY KEY CHECK(id=1), revision INTEGER NOT NULL); PRAGMA user_version=1;").map_err(|e|e.to_string())?;
    Ok(conn)
}
fn load(conn: &Connection) -> Result<ReminderSnapshot, String> {
    fn rows<T: serde::de::DeserializeOwned>(
        conn: &Connection,
        table: &str,
    ) -> Result<Vec<T>, String> {
        let mut stmt = conn
            .prepare(&format!("SELECT data FROM {table}"))
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.map(|r| {
            serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        })
        .collect()
    }
    let settings: Vec<ReminderSettings> = rows(conn, "settings")?;
    Ok(ReminderSnapshot {
        reminders: rows(conn, "reminders")?,
        occurrences: rows(conn, "occurrences")?,
        settings: settings.into_iter().next().unwrap_or_default(),
        revision: conn
            .query_row("SELECT revision FROM reminder_meta WHERE id=1", [], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?
            .unwrap_or(0),
    })
}
fn save(conn: &mut Connection, snapshot: &ReminderSnapshot) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute_batch("DELETE FROM reminders; DELETE FROM occurrences;")
        .map_err(|e| e.to_string())?;
    for r in &snapshot.reminders {
        tx.execute(
            "INSERT INTO reminders VALUES (?1,?2)",
            params![r.id, serde_json::to_string(r).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    }
    for o in &snapshot.occurrences {
        tx.execute(
            "INSERT INTO occurrences VALUES (?1,?2)",
            params![o.id, serde_json::to_string(o).map_err(|e| e.to_string())?],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO settings VALUES (1,?1)",
        params![serde_json::to_string(&snapshot.settings).map_err(|e| e.to_string())?],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT OR REPLACE INTO reminder_meta VALUES (1,?1)",
        params![snapshot.revision],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
#[derive(Default)]
pub struct ReminderService {
    gate: Mutex<Option<Connection>>,
    epoch: Mutex<u64>,
    wake: Condvar,
    ready: AtomicBool,
}
impl ReminderService {
    fn access<R>(&self, f: impl FnOnce(&mut Connection) -> Result<R, String>) -> Result<R, String> {
        let mut connection = self.gate.lock().map_err(|e| e.to_string())?;
        if connection.is_none() {
            *connection = Some(connect(&database_path()?)?);
        }
        f(connection.as_mut().unwrap())
    }
    pub fn snapshot(&self) -> Result<ReminderSnapshot, String> {
        self.access(|conn| load(conn))
    }
    fn mutate(
        &self,
        f: impl FnOnce(&mut ReminderSnapshot) -> Result<(), String>,
    ) -> Result<ReminderSnapshot, String> {
        let snapshot = self.access(|conn| {
            let mut s = load(conn)?;
            f(&mut s)?;
            s.revision = s.revision.saturating_add(1);
            save(conn, &s)?;
            Ok(s)
        })?;
        let mut epoch = self.epoch.lock().unwrap_or_else(|e| e.into_inner());
        *epoch = epoch.wrapping_add(1);
        self.wake.notify_one();
        Ok(snapshot)
    }
}
fn tick<T: TimeZone>(
    s: &mut ReminderSnapshot,
    now: i64,
    tz: &T,
) -> Result<Vec<ReminderOccurrence>, String> {
    for r in &mut s.reminders {
        if !r.enabled {
            continue;
        }
        // Calendar schedules follow the current local zone, including a zone change.
        if let ReminderSchedule::Daily {
            hour,
            minute,
            weekdays,
        } = &r.schedule
        {
            if r.due_at.is_some_and(|d| d > now) {
                r.due_at = Some(next_calendar(tz, now, *hour, *minute, weekdays)?);
            }
        }
        let Some(at) = r.due_at.filter(|at| *at <= now) else {
            continue;
        };
        let id = format!("{}:{at}", r.id);
        if !s.occurrences.iter().any(|o| o.id == id) {
            s.occurrences.push(ReminderOccurrence {
                id,
                reminder_id: r.id.clone(),
                title: r.title.clone(),
                due_at: at,
                completed_at: None,
                snoozed_until: None,
                alerted: false,
            });
        }
        r.due_at = match &r.schedule {
            ReminderSchedule::Daily {
                hour,
                minute,
                weekdays,
            } => Some(next_calendar(tz, now, *hour, *minute, weekdays)?),
            ReminderSchedule::Interval { seconds } if *seconds > 0 => {
                Some(at + ((now - at) / seconds + 1) * seconds)
            }
            ReminderSchedule::Interval { .. } => return Err("invalidDuration".into()),
            _ => None,
        };
    }
    let mut due = Vec::new();
    for o in &mut s.occurrences {
        if o.completed_at.is_none() && !o.alerted && o.snoozed_until.unwrap_or(o.due_at) <= now {
            o.alerted = true;
            o.snoozed_until = None;
            due.push(o.clone());
        }
    }
    s.occurrences
        .retain(|o| o.completed_at.is_none_or(|at| at > now - 30 * 86400));
    s.reminders.retain(|r| {
        r.due_at.is_some() || !r.enabled || s.occurrences.iter().any(|o| o.reminder_id == r.id)
    });
    Ok(due)
}
/// None means wait indefinitely for a mutation: pending already-alerted items do not poll.
fn scheduler_wait(snapshot: &ReminderSnapshot, now: i64) -> Option<Duration> {
    snapshot
        .reminders
        .iter()
        .filter(|r| r.enabled)
        .filter_map(|r| r.due_at)
        .chain(
            snapshot
                .occurrences
                .iter()
                .filter(|o| o.completed_at.is_none() && !o.alerted)
                .map(|o| o.snoozed_until.unwrap_or(o.due_at)),
        )
        .min()
        .map(|at| Duration::from_secs((at - now).clamp(1, 30) as u64))
}
#[cfg(windows)]
const REMINDER_SOUND: &str = "Default";
#[cfg(not(windows))]
const REMINDER_SOUND: &str = "Glass";

pub fn start(app: AppHandle) {
    std::thread::spawn(move || loop {
        let service = app.state::<ReminderService>();
        let epoch = *service.epoch.lock().unwrap_or_else(|e| e.into_inner());
        if !service.ready.load(Ordering::Acquire) {
            let guard = service.epoch.lock().unwrap_or_else(|e| e.into_inner());
            if !service.ready.load(Ordering::Acquire) {
                drop(service.wake.wait(guard));
            }
            continue;
        }
        let now = Utc::now().timestamp();
        let result = service.access(|conn| {
            let mut snapshot = load(conn)?;
            let before = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
            let due = tick(&mut snapshot, now, &Local)?;
            if serde_json::to_string(&snapshot).map_err(|e| e.to_string())? != before {
                snapshot.revision = snapshot.revision.saturating_add(1);
                save(conn, &snapshot)?;
                let _ = app.emit("reminders-changed", &snapshot);
            }
            Ok((snapshot, due))
        });
        let delay = match result {
            Ok((snapshot, due)) => {
                if !due.is_empty() {
                    crate::show_island_quietly(&app);
                    use tauri_plugin_notification::NotificationExt;
                    let language =
                        crate::lock_state(&app.state::<crate::AppState>().notification_language)
                            .clone();
                    let zh = language.starts_with("zh");
                    let heading = if zh { "Atoll 提醒" } else { "Atoll Reminder" };
                    let body = due
                        .iter()
                        .map(|o| {
                            if o.title.is_empty() {
                                if zh {
                                    "倒计时结束"
                                } else {
                                    "Timer finished"
                                }
                            } else {
                                o.title.as_str()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let mut notification = app.notification().builder().title(heading).body(body);
                    if snapshot.settings.sound {
                        notification = notification.sound(REMINDER_SOUND);
                    }
                    let fallback_sound = notification.show().is_err() && snapshot.settings.sound;
                    let _ = app.emit(
                        "reminders-due",
                        ReminderDue {
                            occurrences: due,
                            notice_mode: snapshot.settings.notice_mode.clone(),
                            fallback_sound,
                        },
                    );
                }
                scheduler_wait(&snapshot, now)
            }
            Err(error) => {
                eprintln!("[Atoll] reminders: {error}");
                Some(Duration::from_secs(30))
            }
        };
        let guard = service.epoch.lock().unwrap_or_else(|e| e.into_inner());
        if *guard != epoch {
            continue;
        }
        match delay {
            Some(delay) => {
                drop(service.wake.wait_timeout(guard, delay));
            }
            None => {
                drop(service.wake.wait(guard));
            }
        }
    });
}
#[tauri::command]
pub fn preview_reminder(input: String) -> Result<ReminderDraft, String> {
    parse(&input, Utc::now().timestamp(), &Local)
}
#[tauri::command]
pub fn get_reminders(
    service: tauri::State<'_, ReminderService>,
) -> Result<ReminderSnapshot, String> {
    let snapshot = service.snapshot()?;
    let mut epoch = service.epoch.lock().unwrap_or_else(|e| e.into_inner());
    service.ready.store(true, Ordering::Release);
    *epoch = epoch.wrapping_add(1);
    service.wake.notify_one();
    Ok(snapshot)
}
#[tauri::command]
pub fn create_reminder(
    app: AppHandle,
    service: tauri::State<'_, ReminderService>,
    title: String,
    schedule: ReminderSchedule,
    id: Option<String>,
) -> Result<ReminderSnapshot, String> {
    let now = Utc::now().timestamp();
    let at = next_due(&schedule, now, &Local)?;
    if title.chars().count() > 500 {
        return Err("titleTooLong".into());
    }
    let s = service.mutate(|s| {
        let r = Reminder {
            id: id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            title: title.trim().into(),
            schedule,
            enabled: true,
            due_at: Some(at),
            remaining_seconds: None,
        };
        if let Some(id) = id {
            let existing = s
                .reminders
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or("notFound")?;
            *existing = r;
        } else {
            s.reminders.push(r);
        }
        Ok(())
    })?;
    let _ = app.emit("reminders-changed", &s);
    Ok(s)
}
#[tauri::command]
pub fn update_reminder(
    app: AppHandle,
    service: tauri::State<'_, ReminderService>,
    id: String,
    action: String,
    minutes: Option<i64>,
) -> Result<ReminderSnapshot, String> {
    let now = Utc::now().timestamp();
    let s = service.mutate(|s| apply_action(s, &id, &action, minutes, now))?;
    let _ = app.emit("reminders-changed", &s);
    Ok(s)
}
fn apply_action(
    s: &mut ReminderSnapshot,
    id: &str,
    action: &str,
    minutes: Option<i64>,
    now: i64,
) -> Result<(), String> {
    if action == "complete" || action == "snooze" {
        let o = s
            .occurrences
            .iter_mut()
            .find(|o| o.id == id && o.completed_at.is_none())
            .ok_or("notFound")?;
        if action == "complete" {
            o.completed_at = Some(now);
            o.snoozed_until = None;
        } else {
            let minutes = minutes
                .filter(|m| *m > 0 && *m <= 1440)
                .ok_or("invalidDuration")?;
            o.snoozed_until = Some(now + minutes * 60);
            o.alerted = false;
        }
        return Ok(());
    }
    if action == "delete" {
        if !s.reminders.iter().any(|r| r.id == id) {
            return Err("notFound".into());
        }
        s.reminders.retain(|r| r.id != id);
        s.occurrences.retain(|o| o.reminder_id != id);
        return Ok(());
    }
    let r = s
        .reminders
        .iter_mut()
        .find(|r| r.id == id)
        .ok_or("notFound")?;
    match action {
        "pause" => {
            if r.enabled {
                r.remaining_seconds = r.due_at.map(|at| (at - now).max(1));
                r.due_at = None;
                r.enabled = false;
            }
        }
        "resume" => {
            if !r.enabled {
                r.due_at = Some(if matches!(r.schedule, ReminderSchedule::Timer { .. }) {
                    now + r.remaining_seconds.unwrap_or(1)
                } else {
                    next_due(&r.schedule, now, &Local)?
                });
                r.enabled = true;
                r.remaining_seconds = None;
            }
        }
        "restart" => {
            if !matches!(r.schedule, ReminderSchedule::Timer { .. }) {
                return Err("invalidAction".into());
            }
            r.due_at = Some(next_due(&r.schedule, now, &Local)?);
            r.enabled = true;
            r.remaining_seconds = None;
            for o in &mut s.occurrences {
                if o.reminder_id == id && o.completed_at.is_none() {
                    o.completed_at = Some(now);
                    o.snoozed_until = None;
                }
            }
        }
        _ => return Err("invalidAction".into()),
    }
    Ok(())
}
#[tauri::command]
pub fn set_reminder_settings(
    app: AppHandle,
    service: tauri::State<'_, ReminderService>,
    settings: ReminderSettings,
) -> Result<ReminderSnapshot, String> {
    if settings.notice_mode != "interrupt" && settings.notice_mode != "notify" {
        return Err("invalidMode".into());
    }
    let s = service.mutate(|s| {
        s.settings = settings;
        Ok(())
    })?;
    let _ = app.emit("reminders-changed", &s);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;
    fn now() -> i64 {
        FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 2, 16, 0, 0)
            .unwrap()
            .timestamp()
    }
    fn tz() -> FixedOffset {
        FixedOffset::east_opt(8 * 3600).unwrap()
    }
    fn timer(at: i64) -> ReminderSnapshot {
        ReminderSnapshot {
            reminders: vec![Reminder {
                id: "timer".into(),
                title: "tea".into(),
                schedule: ReminderSchedule::Timer { seconds: 60 },
                enabled: true,
                due_at: Some(at),
                remaining_seconds: None,
            }],
            ..Default::default()
        }
    }
    #[test]
    fn parses_all_documented_inputs() {
        for (input, seconds) in [
            ("20分钟后喝水", 1200),
            ("半小时", 1800),
            ("25m", 1500),
            ("in 20 minutes drink water", 1200),
            ("二十五分钟后喝水", 1500),
        ] {
            let draft = parse(input, now(), &tz()).unwrap();
            assert_eq!(draft.due_at, now() + seconds);
            assert_eq!(draft.schedule, ReminderSchedule::Timer { seconds });
        }
        for (input, hour, day) in [
            ("明天下午3点开会", 15, 3),
            ("18:30取快递", 18, 2),
            ("tomorrow at 3pm meeting", 15, 3),
            ("2026-10-03 15:00 meeting", 15, 3),
            ("今天晚上10点睡觉", 22, 2),
        ] {
            let draft = parse(input, now(), &tz()).unwrap();
            let d = tz().timestamp_opt(draft.due_at, 0).unwrap();
            assert_eq!(chrono::Timelike::hour(&d), hour);
            assert_eq!(d.day(), day);
        }
        for input in [
            "每天晚上10点睡觉",
            "every day at 10pm sleep",
            "daily 22:00 sleep",
        ] {
            assert!(matches!(
                parse(input, now(), &tz()).unwrap().schedule,
                ReminderSchedule::Daily { hour: 22, .. }
            ));
        }
        for input in ["工作日15点休息", "weekdays at 15:00 break"] {
            assert!(
                matches!(parse(input,now(),&tz()).unwrap().schedule,ReminderSchedule::Daily{weekdays,..} if weekdays==vec![0,1,2,3,4])
            );
        }
        for input in ["每周五18点写周报", "every friday at 6pm report"] {
            assert!(
                matches!(parse(input,now(),&tz()).unwrap().schedule,ReminderSchedule::Daily{weekdays,..} if weekdays==vec![4])
            );
        }
        for input in ["每隔45分钟站起来", "every 45 minutes stretch"] {
            assert_eq!(
                parse(input, now(), &tz()).unwrap().schedule,
                ReminderSchedule::Interval { seconds: 2700 }
            );
        }
    }
    #[test]
    fn rejects_ambiguity_past_dates_and_invalid_values() {
        for input in [
            "喝水",
            "20",
            "今天15点开会",
            "25:30 event",
            "0m",
            "-5m",
            "99999999999999999999h",
            "明天3点或4点开会",
            "20分钟或30分钟喝水",
            "25meeting",
            "一二十分钟",
        ] {
            assert!(parse(input, now(), &tz()).is_err(), "{input}");
        }
        assert!(parse("10pm sleep", now(), &tz()).is_ok());
        assert_eq!(
            tz().timestamp_opt(parse("15:00 task", now(), &tz()).unwrap().due_at, 0)
                .unwrap()
                .day(),
            3
        );
        assert!(next_due(
            &ReminderSchedule::Daily {
                hour: 3,
                minute: 0,
                weekdays: vec![7]
            },
            now(),
            &tz()
        )
        .is_err());
    }
    #[test]
    fn catchup_is_once_and_advances_interval_without_replay() {
        let mut s = timer(100);
        s.reminders[0].schedule = ReminderSchedule::Interval { seconds: 60 };
        assert_eq!(tick(&mut s, 1000, &Utc).unwrap().len(), 1);
        assert_eq!(s.reminders[0].due_at, Some(1060));
        assert!(tick(&mut s, 1000, &Utc).unwrap().is_empty());
        assert_eq!(s.occurrences.len(), 1);
    }
    #[test]
    fn snooze_leaves_recurrence_and_completion_only_handles_one_occurrence() {
        let mut s = timer(100);
        s.reminders[0].schedule = ReminderSchedule::Interval { seconds: 60 };
        tick(&mut s, 100, &Utc).unwrap();
        let id = s.occurrences[0].id.clone();
        apply_action(&mut s, &id, "snooze", Some(5), 100).unwrap();
        assert_eq!(s.reminders[0].due_at, Some(160));
        assert!(tick(&mut s, 101, &Utc).unwrap().is_empty());
        apply_action(&mut s, &id, "complete", None, 102).unwrap();
        assert!(s.reminders[0].enabled);
        assert_eq!(s.reminders[0].due_at, Some(160));
    }
    #[test]
    fn paused_timer_restores_remaining_time_and_restart_clears_old_due() {
        let mut s = timer(160);
        apply_action(&mut s, "timer", "pause", None, 120).unwrap();
        assert_eq!(s.reminders[0].remaining_seconds, Some(40));
        assert!(tick(&mut s, 500, &Utc).unwrap().is_empty());
        apply_action(&mut s, "timer", "resume", None, 500).unwrap();
        assert_eq!(s.reminders[0].due_at, Some(540));
        tick(&mut s, 550, &Utc).unwrap();
        apply_action(&mut s, "timer", "restart", None, 560).unwrap();
        assert_eq!(s.reminders[0].due_at, Some(620));
        assert_eq!(s.occurrences[0].completed_at, Some(560));
    }
    #[test]
    fn snoozed_timer_fires_after_restart_without_realerting_old_due() {
        let mut s = timer(100);
        tick(&mut s, 100, &Utc).unwrap();
        let id = s.occurrences[0].id.clone();
        apply_action(&mut s, &id, "snooze", Some(5), 110).unwrap();
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE reminders(id TEXT PRIMARY KEY,data TEXT);CREATE TABLE occurrences(id TEXT PRIMARY KEY,data TEXT);CREATE TABLE settings(id INTEGER PRIMARY KEY,data TEXT);CREATE TABLE reminder_meta(id INTEGER PRIMARY KEY,revision INTEGER NOT NULL);").unwrap();
        save(&mut conn, &s).unwrap();
        let mut restored = load(&conn).unwrap();
        assert!(tick(&mut restored, 409, &Utc).unwrap().is_empty());
        assert_eq!(tick(&mut restored, 410, &Utc).unwrap().len(), 1);
        assert!(tick(&mut restored, 411, &Utc).unwrap().is_empty());
    }
    #[test]
    fn pending_never_expires_and_completed_prunes_after_thirty_days() {
        let mut s = timer(100);
        tick(&mut s, 100, &Utc).unwrap();
        let mut done = s.occurrences[0].clone();
        done.id = "done".into();
        done.completed_at = Some(100);
        s.occurrences.push(done);
        tick(&mut s, 100 + 31 * 86400, &Utc).unwrap();
        assert_eq!(s.occurrences.len(), 1);
        assert!(s.occurrences[0].completed_at.is_none());
    }
    #[test]
    fn batch_multiple_due_and_failed_transaction_keeps_database_unchanged() {
        let mut s = timer(100);
        let mut second = s.reminders[0].clone();
        second.id = "second".into();
        s.reminders.push(second);
        assert_eq!(tick(&mut s, 100, &Utc).unwrap().len(), 2);
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE reminders(id TEXT PRIMARY KEY,data TEXT);CREATE TABLE occurrences(id TEXT PRIMARY KEY,data TEXT);CREATE TABLE settings(id INTEGER PRIMARY KEY,data TEXT);CREATE TABLE reminder_meta(id INTEGER PRIMARY KEY,revision INTEGER NOT NULL);").unwrap();
        save(&mut conn, &s).unwrap();
        let before = load(&conn).unwrap();
        s.reminders.push(s.reminders[0].clone());
        assert!(save(&mut conn, &s).is_err());
        assert_eq!(load(&conn).unwrap().reminders.len(), before.reminders.len());
    }
    #[test]
    fn timezone_switch_preserves_once_but_retargets_calendar() {
        let mut s = timer(now() + 3600);
        s.reminders[0].schedule = ReminderSchedule::Once { at: now() + 3600 };
        s.reminders.push(Reminder {
            id: "daily".into(),
            title: "wake".into(),
            schedule: ReminderSchedule::Daily {
                hour: 18,
                minute: 0,
                weekdays: vec![],
            },
            enabled: true,
            due_at: Some(now() + 7200),
            remaining_seconds: None,
        });
        tick(&mut s, now(), &Utc).unwrap();
        assert_eq!(s.reminders[0].due_at, Some(now() + 3600));
        assert_eq!(
            s.reminders[1].due_at,
            Some(
                Utc.with_ymd_and_hms(2026, 10, 2, 18, 0, 0)
                    .unwrap()
                    .timestamp()
            )
        );
    }
    #[test]
    fn idle_scheduler_has_no_timer_and_active_waits_calibrate_within_thirty_seconds() {
        assert_eq!(scheduler_wait(&ReminderSnapshot::default(), 0), None);
        let mut s = timer(100);
        assert_eq!(scheduler_wait(&s, 0), Some(Duration::from_secs(30)));
        assert_eq!(scheduler_wait(&s, 99), Some(Duration::from_secs(1)));
        tick(&mut s, 100, &Utc).unwrap();
        assert_eq!(scheduler_wait(&s, 100), None);
        let id = s.occurrences[0].id.clone();
        apply_action(&mut s, &id, "snooze", Some(5), 100).unwrap();
        assert_eq!(scheduler_wait(&s, 100), Some(Duration::from_secs(30)));
    }

    #[test]
    fn dst_gap_and_repeated_hour() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "reminders::tests::dst_worker", "--nocapture"])
            .env("TZ", "America/New_York")
            .env("ATOLL_DST_TEST", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    #[test]
    fn dst_worker() {
        if std::env::var("ATOLL_DST_TEST").as_deref() != Ok("1") {
            return;
        }
        let before = Utc
            .with_ymd_and_hms(2026, 3, 8, 5, 0, 0)
            .unwrap()
            .timestamp();
        let gap = parse("2026-03-08 02:30 wake", before, &Local).unwrap();
        assert_eq!(
            gap.due_at,
            Utc.with_ymd_and_hms(2026, 3, 8, 7, 0, 0)
                .unwrap()
                .timestamp()
        );
        let before = Utc
            .with_ymd_and_hms(2026, 11, 1, 4, 0, 0)
            .unwrap()
            .timestamp();
        let first = next_calendar(&Local, before, 1, 30, &[]).unwrap();
        assert_eq!(
            first,
            Utc.with_ymd_and_hms(2026, 11, 1, 5, 30, 0)
                .unwrap()
                .timestamp()
        );
        let second = next_calendar(&Local, first + 1, 1, 30, &[]).unwrap();
        assert_eq!(
            second,
            Utc.with_ymd_and_hms(2026, 11, 2, 6, 30, 0)
                .unwrap()
                .timestamp()
        );
    }
}
