//! Persistent approval history.
//!
//! Every permission request surfaced by the hook bridge is recorded to a local
//! SQLite database (`~/.atoll/approval_history.db`) so the history survives
//! restarts — the in-memory `AppState.requests` list is still pruned and lost
//! on exit. Rows are upserted by request id: the hook bridge inserts a
//! `pending` row when a request arrives and updates it once the request is
//! resolved (by the user, a timeout, or the agent itself).
//!
//! Outcome mapping (the in-memory `PermissionStatus` only knows
//! pending/approved/denied; the rest is encoded in detail suffixes):
//! - `approved` — decided in Atoll, or auto-approved for the session.
//! - `denied` — decided in Atoll.
//! - `expired` — 30-minute hook timeout ("Timed out waiting for Atoll
//!   approval.") or the auto-archive idle timeout ("Auto-archived after idle
//!   timeout.").
//! - `answered_elsewhere` — the agent side resolved/executed the request while
//!   Atoll was still waiting ("Resolved in {agent}." / "Completed in {agent}.")
//!
//! Retention is a fixed cap: 5000 rows and 90 days, pruned after each write.
//! Tests override the location with `ATOLL_APPROVAL_HISTORY_PATH` (mirrors
//! `ATOLL_TOKEN_HISTORY_PATH` in token_history.rs).

use std::path::PathBuf;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::{platform, AppState, PermissionRequest};

pub const DEFAULT_MAX_ROWS: i64 = 5000;
pub const RETENTION_DAYS: u64 = 90;
pub const DEFAULT_PAGE_LIMIT: u32 = 100;
pub const MAX_PAGE_LIMIT: u32 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryStatus {
    Pending,
    Approved,
    Denied,
    Expired,
    AnsweredElsewhere,
}

impl HistoryStatus {
    fn as_key(self) -> &'static str {
        match self {
            HistoryStatus::Pending => "pending",
            HistoryStatus::Approved => "approved",
            HistoryStatus::Denied => "denied",
            HistoryStatus::Expired => "expired",
            HistoryStatus::AnsweredElsewhere => "answered_elsewhere",
        }
    }

    fn from_key(key: &str) -> Option<HistoryStatus> {
        match key {
            "pending" => Some(HistoryStatus::Pending),
            "approved" => Some(HistoryStatus::Approved),
            "denied" => Some(HistoryStatus::Denied),
            "expired" => Some(HistoryStatus::Expired),
            "answered_elsewhere" => Some(HistoryStatus::AnsweredElsewhere),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalHistoryEntry {
    pub id: String,
    pub agent: String,
    pub session_id: String,
    pub command: String,
    pub detail: String,
    pub cwd: String,
    pub tool_input: Option<serde_json::Value>,
    pub transcript_path: Option<String>,
    /// Unix seconds when the request arrived.
    pub requested_at: u64,
    /// Unix seconds when the request left `pending`; null while pending.
    pub decided_at: Option<u64>,
    pub status: HistoryStatus,
    /// SessionHost key ("" when unknown) — CLI vs Desktop etc.
    pub host: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApprovalHistoryQuery {
    pub search: Option<String>,
    pub agent: Option<String>,
    pub status: Option<String>,
    pub session_id: Option<String>,
    pub from_secs: Option<u64>,
    pub to_secs: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalHistoryPage {
    pub items: Vec<ApprovalHistoryEntry>,
    pub total: i64,
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Guards the process-wide `ATOLL_APPROVAL_*` env overrides in parallel tests.
#[cfg(test)]
pub(crate) fn approval_history_env_lock() -> std::sync::MutexGuard<'static, ()> {
    crate::APPROVAL_HISTORY_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn db_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("ATOLL_APPROVAL_HISTORY_PATH") {
        if !path.is_empty() {
            return Some(PathBuf::from(path));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("approval_history.db"))
}

fn export_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ATOLL_APPROVAL_EXPORT_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    dirs::download_dir().or_else(dirs::home_dir)
}

/// Open (creating and migrating as needed) the history database. A corrupt
/// database file is removed and recreated rather than wedging approvals.
fn open_db() -> Result<Connection, String> {
    let path = db_path().ok_or_else(|| "approval history path unavailable".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if let Some(conn) = open_with_schema(&path) {
        return Ok(conn);
    }
    eprintln!("Atoll approval history: database unreadable, recreating");
    let _ = std::fs::remove_file(&path);
    let conn = Connection::open(&path).map_err(|error| error.to_string())?;
    initialize_schema(&conn)?;
    Ok(conn)
}

fn open_with_schema(path: &std::path::Path) -> Option<Connection> {
    let conn = Connection::open(path).ok()?;
    initialize_schema(&conn).ok()?;
    Some(conn)
}

fn initialize_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS approval_history (
            id              TEXT PRIMARY KEY,
            agent           TEXT NOT NULL,
            session_id      TEXT NOT NULL,
            command         TEXT NOT NULL,
            detail          TEXT NOT NULL,
            cwd             TEXT NOT NULL,
            tool_input      TEXT,
            transcript_path TEXT,
            requested_at    INTEGER NOT NULL,
            decided_at      INTEGER,
            status          TEXT NOT NULL,
            host            TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_approval_history_requested_at
            ON approval_history(requested_at DESC);
        CREATE INDEX IF NOT EXISTS idx_approval_history_session
            ON approval_history(session_id);",
    )
    .map_err(|error| error.to_string())?;
    // Migration-if-exists: databases created by older builds may miss columns.
    migrate_column(
        conn,
        "tool_input",
        "ALTER TABLE approval_history ADD COLUMN tool_input TEXT",
    )?;
    migrate_column(
        conn,
        "transcript_path",
        "ALTER TABLE approval_history ADD COLUMN transcript_path TEXT",
    )?;
    migrate_column(
        conn,
        "host",
        "ALTER TABLE approval_history ADD COLUMN host TEXT NOT NULL DEFAULT ''",
    )?;
    Ok(())
}

fn migrate_column(conn: &Connection, column: &str, add: &str) -> Result<(), String> {
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('approval_history') WHERE name = ?1",
            [column],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if exists == 0 {
        conn.execute_batch(add).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn host_key(host: platform::SessionHost) -> String {
    serde_json::to_value(host)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn entry_from_request_with_host(
    request: &PermissionRequest,
    status: HistoryStatus,
    host: String,
) -> ApprovalHistoryEntry {
    ApprovalHistoryEntry {
        id: request.id.clone(),
        agent: crate::token_history::agent_kind_key(&request.agent),
        session_id: request.session.clone(),
        command: request.command.clone(),
        detail: request.detail.clone(),
        cwd: request.cwd.clone(),
        tool_input: request.tool_input.clone(),
        transcript_path: request.transcript_path.clone(),
        requested_at: crate::parse_iso_timestamp_secs(&request.requested_at),
        decided_at: (!matches!(status, HistoryStatus::Pending)).then(now_secs),
        status,
        host,
    }
}

fn entry_from_request(
    state: &AppState,
    request: &PermissionRequest,
    status: HistoryStatus,
) -> ApprovalHistoryEntry {
    let host = host_key(crate::get_stored_session_host(state, &request.session));
    entry_from_request_with_host(request, status, host)
}

/// Record a request's current state. Pending rows never overwrite an existing
/// decision, and decisions never overwrite each other (first outcome wins), so
/// concurrent timeout/resolve races cannot regress history. Failures are
/// logged and swallowed: history must never break the approval path.
fn upsert_entry(conn: &Connection, entry: &ApprovalHistoryEntry) -> Result<(), String> {
    let tool_input = entry
        .tool_input
        .as_ref()
        .map(|value| serde_json::to_string(value).map_err(|error| error.to_string()))
        .transpose()?;
    conn.execute(
        "INSERT INTO approval_history
            (id, agent, session_id, command, detail, cwd, tool_input,
             transcript_path, requested_at, decided_at, status, host)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(id) DO UPDATE SET
            detail = excluded.detail,
            status = excluded.status,
            decided_at = excluded.decided_at,
            host = CASE WHEN excluded.host != '' THEN excluded.host
                        ELSE approval_history.host END
         WHERE approval_history.status = 'pending'",
        rusqlite::params![
            entry.id,
            entry.agent,
            entry.session_id,
            entry.command,
            entry.detail,
            entry.cwd,
            tool_input,
            entry.transcript_path,
            entry.requested_at as i64,
            entry.decided_at.map(|secs| secs as i64),
            entry.status.as_key(),
            entry.host,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

/// Record the arrival of a permission request (status `pending`).
pub(crate) fn record_pending(state: &AppState, request: &PermissionRequest) {
    let entry = entry_from_request(state, request, HistoryStatus::Pending);
    if let Err(error) = record_entry(&entry) {
        eprintln!("Atoll approval history: failed to record arrival: {error}");
    }
}

/// Record the outcome of a permission request (approved/denied/expired/
/// answered_elsewhere). The detail already carries the human-readable marker.
pub(crate) fn record_outcome(state: &AppState, request: &PermissionRequest, status: HistoryStatus) {
    if matches!(status, HistoryStatus::Pending) {
        return;
    }
    let entry = entry_from_request(state, request, status);
    if let Err(error) = record_entry(&entry) {
        eprintln!("Atoll approval history: failed to record outcome: {error}");
    }
}

fn record_entry(entry: &ApprovalHistoryEntry) -> Result<(), String> {
    let conn = open_db()?;
    upsert_entry(&conn, entry)?;
    prune_db(&conn)?;
    Ok(())
}

/// Delete rows older than the retention window, then cap the table to
/// `DEFAULT_MAX_ROWS` keeping the newest rows.
fn prune_db(conn: &Connection) -> Result<(), String> {
    let cutoff = now_secs().saturating_sub(RETENTION_DAYS * 24 * 60 * 60);
    conn.execute(
        "DELETE FROM approval_history WHERE requested_at < ?1",
        [cutoff as i64],
    )
    .map_err(|error| error.to_string())?;
    conn.execute(
        "DELETE FROM approval_history WHERE rowid NOT IN (
            SELECT rowid FROM approval_history ORDER BY requested_at DESC LIMIT ?1
        )",
        [DEFAULT_MAX_ROWS],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn like_pattern(term: &str) -> String {
    let escaped = term
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Build the shared WHERE clause for queries and exports.
fn build_filters(query: &ApprovalHistoryQuery) -> (String, Vec<rusqlite::types::Value>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<rusqlite::types::Value> = Vec::new();

    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        clauses.push(
            "(command LIKE ? ESCAPE '\\' OR detail LIKE ? ESCAPE '\\' OR cwd LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        let pattern = like_pattern(search);
        params.push(pattern.clone().into());
        params.push(pattern.clone().into());
        params.push(pattern.into());
    }
    if let Some(agent) = query.agent.as_deref().filter(|s| !s.is_empty()) {
        clauses.push("agent = ?".to_string());
        params.push(agent.to_string().into());
    }
    if let Some(status) = query
        .status
        .as_deref()
        .filter(|s| !s.is_empty())
        .and_then(HistoryStatus::from_key)
    {
        clauses.push("status = ?".to_string());
        params.push(status.as_key().to_string().into());
    }
    if let Some(session) = query.session_id.as_deref().filter(|s| !s.is_empty()) {
        clauses.push("session_id = ?".to_string());
        params.push(session.to_string().into());
    }
    if let Some(from) = query.from_secs {
        clauses.push("requested_at >= ?".to_string());
        params.push((from as i64).into());
    }
    if let Some(to) = query.to_secs {
        clauses.push("requested_at <= ?".to_string());
        params.push((to as i64).into());
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    (where_clause, params)
}

fn entry_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApprovalHistoryEntry> {
    let tool_input: Option<String> = row.get("tool_input")?;
    let status_key: String = row.get("status")?;
    Ok(ApprovalHistoryEntry {
        id: row.get("id")?,
        agent: row.get("agent")?,
        session_id: row.get("session_id")?,
        command: row.get("command")?,
        detail: row.get("detail")?,
        cwd: row.get("cwd")?,
        tool_input: tool_input.and_then(|text| serde_json::from_str(&text).ok()),
        transcript_path: row.get("transcript_path")?,
        requested_at: row.get::<_, i64>("requested_at")?.max(0) as u64,
        decided_at: row
            .get::<_, Option<i64>>("decided_at")?
            .map(|secs| secs.max(0) as u64),
        status: HistoryStatus::from_key(&status_key).unwrap_or(HistoryStatus::Pending),
        host: row.get("host")?,
    })
}

/// Paginated, filtered query over the history. Exposed as `get_approval_history`;
/// a missing page size defaults to [`DEFAULT_PAGE_LIMIT`].
pub fn query_history(query: &ApprovalHistoryQuery) -> Result<ApprovalHistoryPage, String> {
    let mut query = query.clone();
    if query.limit.is_none() {
        query.limit = Some(DEFAULT_PAGE_LIMIT);
    }
    let conn = open_db()?;
    query_entries(&conn, &query)
}

/// Core query. `limit: None` means "everything" (export path).
fn query_entries(
    conn: &Connection,
    query: &ApprovalHistoryQuery,
) -> Result<ApprovalHistoryPage, String> {
    let (where_clause, params) = build_filters(query);
    let total: i64 = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM approval_history{where_clause}"),
            rusqlite::params_from_iter(params.iter()),
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;

    let page_clause = match query.limit {
        Some(limit) => {
            let clamped = limit.clamp(1, MAX_PAGE_LIMIT);
            let offset = query.offset.unwrap_or(0);
            format!(" LIMIT {clamped} OFFSET {offset}")
        }
        None => String::new(),
    };

    let mut statement = conn
        .prepare(&format!(
            "SELECT * FROM approval_history{where_clause}
             ORDER BY requested_at DESC, id DESC{page_clause}"
        ))
        .map_err(|error| error.to_string())?;
    let items = statement
        .query_map(rusqlite::params_from_iter(params.iter()), entry_from_row)
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    Ok(ApprovalHistoryPage { items, total })
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn csv_row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|field| csv_escape(field))
        .collect::<Vec<_>>()
        .join(",")
}

/// Export the filtered history (pagination ignored) to
/// `~/Downloads/atoll-history-<timestamp>.json|csv` and return the path.
/// CSV carries the readable columns; raw `tool_input` JSON stays JSON-only.
pub fn export_history(query: &ApprovalHistoryQuery, format: &str) -> Result<String, String> {
    let mut full_query = query.clone();
    full_query.limit = None;
    full_query.offset = None;
    let conn = open_db()?;
    let page = query_entries(&conn, &full_query)?;

    let extension = match format {
        "csv" => "csv",
        _ => "json",
    };
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let file_name = format!("atoll-history-{stamp}.{extension}");
    let dir = export_dir().ok_or_else(|| "export directory unavailable".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(file_name);

    let content = match extension {
        "csv" => {
            let mut rows = vec![csv_row(&[
                "id".into(),
                "status".into(),
                "agent".into(),
                "session".into(),
                "command".into(),
                "detail".into(),
                "cwd".into(),
                "host".into(),
                "requestedAt".into(),
                "decidedAt".into(),
                "transcriptPath".into(),
            ])];
            for entry in &page.items {
                rows.push(csv_row(&[
                    entry.id.clone(),
                    entry.status.as_key().to_string(),
                    entry.agent.clone(),
                    entry.session_id.clone(),
                    entry.command.clone(),
                    entry.detail.clone(),
                    entry.cwd.clone(),
                    entry.host.clone(),
                    crate::format_unix_timestamp(entry.requested_at),
                    entry
                        .decided_at
                        .map(crate::format_unix_timestamp)
                        .unwrap_or_default(),
                    entry.transcript_path.clone().unwrap_or_default(),
                ]));
            }
            // BOM so Excel opens UTF-8 commands/projects correctly.
            format!("\u{feff}{}\n", rows.join("\n"))
        }
        _ => serde_json::to_string_pretty(&page.items).map_err(|error| error.to_string())?,
    };
    std::fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Erase the whole history. Exposed as `clear_approval_history`.
pub fn clear_history() -> Result<(), String> {
    let conn = open_db()?;
    conn.execute("DELETE FROM approval_history", [])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AgentKind;

    fn temp_db_path(test_name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "atoll-approval-history-{}-{test_name}.db",
            std::process::id()
        ))
    }

    fn with_temp_db(test_name: &str) -> (std::sync::MutexGuard<'static, ()>, PathBuf) {
        let guard = approval_history_env_lock();
        let path = temp_db_path(test_name);
        let _ = std::fs::remove_file(&path);
        std::env::set_var(
            "ATOLL_APPROVAL_HISTORY_PATH",
            path.to_string_lossy().as_ref(),
        );
        (guard, path)
    }

    fn entry(id: &str, command: &str, cwd: &str, agent: &str, at: u64) -> ApprovalHistoryEntry {
        ApprovalHistoryEntry {
            id: id.into(),
            agent: agent.into(),
            session_id: "sess-1".into(),
            command: command.into(),
            detail: format!("{command} detail"),
            cwd: cwd.into(),
            tool_input: None,
            transcript_path: None,
            requested_at: at,
            decided_at: None,
            status: HistoryStatus::Pending,
            host: String::new(),
        }
    }

    fn insert(conn: &Connection, entry: &ApprovalHistoryEntry) {
        upsert_entry(conn, entry).expect("upsert entry");
    }

    #[test]
    fn upsert_arrival_then_outcome_transitions_status() {
        let (_guard, path) = with_temp_db("transition");
        let conn = open_db().expect("open db");

        let arrival = entry("r1", "Bash: ls", "/tmp/proj", "claude", 1000);
        insert(&conn, &arrival);

        let mut outcome = arrival.clone();
        outcome.status = HistoryStatus::Approved;
        outcome.detail = "Bash: ls Approved from Atoll".into();
        outcome.decided_at = Some(1100);
        insert(&conn, &outcome);

        let page = query_entries(&conn, &ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].status, HistoryStatus::Approved);
        assert_eq!(page.items[0].decided_at, Some(1100));
        assert_eq!(page.items[0].detail, "Bash: ls Approved from Atoll");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn upsert_never_regresses_a_decided_row() {
        let (_guard, path) = with_temp_db("no-regress");
        let conn = open_db().expect("open db");

        let mut decided = entry("r1", "Bash: rm", "/tmp/proj", "claude", 1000);
        decided.status = HistoryStatus::Denied;
        decided.decided_at = Some(1050);
        insert(&conn, &decided);

        // A racing timeout (expired) must not overwrite the user's denial.
        let mut stale = decided.clone();
        stale.status = HistoryStatus::Expired;
        insert(&conn, &stale);
        // A late arrival replay must not resurrect the row as pending either.
        let mut arrival = decided.clone();
        arrival.status = HistoryStatus::Pending;
        arrival.decided_at = None;
        insert(&conn, &arrival);

        let page = query_entries(&conn, &ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].status, HistoryStatus::Denied);
        assert_eq!(page.items[0].decided_at, Some(1050));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn search_matches_command_detail_and_cwd_case_insensitively() {
        let (_guard, path) = with_temp_db("search");
        let conn = open_db().expect("open db");
        insert(
            &conn,
            &entry("a", "Bash: cargo test", "/work/Atoll", "claude", 100),
        );
        insert(
            &conn,
            &entry("b", "Edit: main.rs", "/work/other", "codex", 200),
        );
        insert(
            &conn,
            &entry("c", "Bash: npm run build", "/work/atoll-site", "zcode", 300),
        );

        let query = |search: &str| ApprovalHistoryQuery {
            search: Some(search.into()),
            ..Default::default()
        };
        let page = query_entries(&conn, &query("cargo")).expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "a");

        // cwd hit, case-insensitive
        let page = query_entries(&conn, &query("ATOLL")).expect("query");
        assert_eq!(page.total, 2);
        assert_eq!(page.items[0].id, "c");

        // LIKE metacharacters are escaped, not interpreted
        let page = query_entries(&conn, &query("1%0")).expect("query");
        assert_eq!(page.total, 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn filters_combine_agent_status_session_and_range() {
        let (_guard, path) = with_temp_db("filters");
        let conn = open_db().expect("open db");
        let mut e1 = entry("a", "Bash: one", "/p1", "claude", 100);
        e1.session_id = "s1".into();
        e1.status = HistoryStatus::Approved;
        let mut e2 = entry("b", "Bash: two", "/p2", "codex", 200);
        e2.session_id = "s2".into();
        e2.status = HistoryStatus::Expired;
        let mut e3 = entry("c", "Bash: three", "/p3", "claude", 300);
        e3.session_id = "s1".into();
        e3.status = HistoryStatus::AnsweredElsewhere;
        insert(&conn, &e1);
        insert(&conn, &e2);
        insert(&conn, &e3);

        let page = query_entries(
            &conn,
            &ApprovalHistoryQuery {
                agent: Some("claude".into()),
                status: Some("approved".into()),
                ..Default::default()
            },
        )
        .expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "a");

        let page = query_entries(
            &conn,
            &ApprovalHistoryQuery {
                session_id: Some("s1".into()),
                ..Default::default()
            },
        )
        .expect("query");
        assert_eq!(page.total, 2);

        let page = query_entries(
            &conn,
            &ApprovalHistoryQuery {
                from_secs: Some(150),
                to_secs: Some(250),
                ..Default::default()
            },
        )
        .expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "b");

        // Unknown status keys are ignored rather than erroring.
        let page = query_entries(
            &conn,
            &ApprovalHistoryQuery {
                status: Some("bogus".into()),
                ..Default::default()
            },
        )
        .expect("query");
        assert_eq!(page.total, 3);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn pagination_reports_total_and_slices() {
        let (_guard, path) = with_temp_db("paging");
        let conn = open_db().expect("open db");
        for i in 0..7 {
            insert(
                &conn,
                &entry(&format!("r{i}"), &format!("cmd {i}"), "/p", "claude", i),
            );
        }
        let page = query_entries(
            &conn,
            &ApprovalHistoryQuery {
                limit: Some(3),
                offset: Some(3),
                ..Default::default()
            },
        )
        .expect("query");
        assert_eq!(page.total, 7);
        assert_eq!(page.items.len(), 3);
        // newest first: offset 3 skips r6..r4
        assert_eq!(page.items[0].id, "r3");
        assert_eq!(page.items[2].id, "r1");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn query_history_defaults_the_page_size() {
        let (_guard, path) = with_temp_db("default-page");
        let conn = open_db().expect("open db");
        for i in 0..(DEFAULT_PAGE_LIMIT + 5) {
            insert(
                &conn,
                &entry(&format!("r{i}"), "cmd", "/p", "claude", i as u64),
            );
        }
        let page = query_history(&ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, (DEFAULT_PAGE_LIMIT + 5) as i64);
        assert_eq!(page.items.len(), DEFAULT_PAGE_LIMIT as usize);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn prune_drops_old_rows_and_caps_row_count() {
        let (_guard, path) = with_temp_db("prune");
        let conn = open_db().expect("open db");
        let now = now_secs();
        let old = now - RETENTION_DAYS * 24 * 60 * 60 - 10;
        insert(&conn, &entry("old", "cmd", "/p", "claude", old));
        for i in 0..(DEFAULT_MAX_ROWS + 10) {
            insert(
                &conn,
                &entry(&format!("r{i}"), "cmd", "/p", "claude", now + i as u64),
            );
        }
        prune_db(&conn).expect("prune");

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM approval_history", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(total, DEFAULT_MAX_ROWS);
        let oldest_gone: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM approval_history WHERE id = 'old'",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(oldest_gone, 0);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_writes_json_and_csv_with_escaping() {
        let (_guard, db_path) = with_temp_db("export");
        let export_dir =
            std::env::temp_dir().join(format!("atoll-approval-export-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&export_dir);
        std::env::set_var(
            "ATOLL_APPROVAL_EXPORT_DIR",
            export_dir.to_string_lossy().as_ref(),
        );

        let conn = open_db().expect("open db");
        let mut tricky = entry("e1", "Bash: echo \"a,b\"\nline2", "/my proj", "claude", 500);
        tricky.status = HistoryStatus::Approved;
        tricky.decided_at = Some(600);
        insert(&conn, &tricky);

        let json_path =
            export_history(&ApprovalHistoryQuery::default(), "json").expect("json export");
        assert!(json_path.ends_with(".json"));
        let json = std::fs::read_to_string(&json_path).expect("read json");
        assert!(json.contains("\"sessionId\": \"sess-1\""));
        assert!(json.contains("echo \\\"a,b\\\""));

        let csv_path = export_history(&ApprovalHistoryQuery::default(), "csv").expect("csv export");
        assert!(csv_path.ends_with(".csv"));
        let csv = std::fs::read_to_string(&csv_path).expect("read csv");
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains(
            "id,status,agent,session,command,detail,cwd,host,requestedAt,decidedAt,transcriptPath"
        ));
        // Embedded quote/comma/newline are CSV-escaped.
        assert!(csv.contains("\"Bash: echo \"\"a,b\"\""));
        assert!(csv.contains("line2\""));

        let _ = std::fs::remove_file(db_path);
        let _ = std::fs::remove_dir_all(&export_dir);
        std::env::remove_var("ATOLL_APPROVAL_EXPORT_DIR");
    }

    #[test]
    fn open_db_recreates_a_corrupt_database() {
        let (_guard, path) = with_temp_db("corrupt");
        std::fs::write(&path, "this is not sqlite").expect("write junk");
        let conn = open_db().expect("open should recreate a usable db");
        insert(&conn, &entry("r1", "Bash: ok", "/p", "claude", 1));
        let page = query_entries(&conn, &ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrate_adds_missing_columns_to_legacy_database() {
        let (_guard, path) = with_temp_db("legacy");
        {
            let legacy = Connection::open(&path).expect("open legacy");
            legacy
                .execute_batch(
                    "CREATE TABLE approval_history (
                        id TEXT PRIMARY KEY,
                        agent TEXT NOT NULL,
                        session_id TEXT NOT NULL,
                        command TEXT NOT NULL,
                        detail TEXT NOT NULL,
                        cwd TEXT NOT NULL,
                        requested_at INTEGER NOT NULL,
                        decided_at INTEGER,
                        status TEXT NOT NULL
                    );",
                )
                .expect("create legacy table");
        }
        let conn = open_db().expect("migration should succeed");
        insert(&conn, &entry("r1", "Bash: legacy", "/p", "claude", 42));
        let page = query_entries(&conn, &ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].host, "");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn entry_from_request_maps_agent_status_and_host() {
        let requested_at = "2026-08-30T10:00:00Z";
        let request = PermissionRequest {
            id: "req-1".into(),
            tool_use_id: Some("tu-1".into()),
            agent: AgentKind::Claude,
            session: "sess-9".into(),
            command: "Bash: git status".into(),
            detail: "Bash: git status".into(),
            cwd: "/work/repo".into(),
            requested_at: requested_at.into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: Some(serde_json::json!({ "command": "git status" })),
        };

        let pending = entry_from_request_with_host(&request, HistoryStatus::Pending, String::new());
        assert_eq!(pending.status, HistoryStatus::Pending);
        assert_eq!(pending.agent, "claude");
        assert_eq!(pending.session_id, "sess-9");
        assert!(pending.decided_at.is_none());
        assert_eq!(
            pending.requested_at,
            crate::parse_iso_timestamp_secs(requested_at)
        );
        assert_eq!(
            pending.tool_input,
            Some(serde_json::json!({ "command": "git status" }))
        );

        let mut denied_request = request.clone();
        denied_request.status = crate::PermissionStatus::Denied;
        let denied = entry_from_request_with_host(
            &denied_request,
            HistoryStatus::Denied,
            "claudeCli".into(),
        );
        assert_eq!(denied.status, HistoryStatus::Denied);
        assert!(denied.decided_at.is_some());
        assert_eq!(denied.host, "claudeCli");
    }
}
