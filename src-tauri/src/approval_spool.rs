//! Offline permission-request spool importer.
//!
//! Hook scripts spool permission requests to `<data>/hook-spool/*.json`
//! when the bridge is unreachable (Atoll not running) — those approvals
//! happen in the agent's own UI and would otherwise vanish from the history
//! entirely. On startup Atoll imports every spooled event into the approval
//! history as `answered_elsewhere` and deletes the file.
//!
//! The imported row id is `spool-<file stem>`, so a crash between insert and
//! delete cannot duplicate rows on the next run: the retry upserts the same
//! id (and decided rows never regress). Spool files older than the history
//! retention window are dropped instead of imported.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::approval_history::{self, ApprovalHistoryEntry, HistoryStatus};
use crate::hook_bridge;
use crate::{AgentKind, PermissionRequest};

#[derive(Debug, Deserialize, Serialize)]
struct SpoolRecord {
    agent: String,
    payload: String,
    spooled_at_secs: u64,
}

fn spool_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("ATOLL_HOOK_SPOOL_DIR") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir));
        }
    }
    dirs::home_dir().map(|home| home.join(".atoll").join("hook-spool"))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Import every spooled permission request into the approval history.
/// Returns the number of imported rows; failures log to stderr and leave the
/// spool file in place for the next startup to retry.
pub(crate) fn import_spooled_requests() -> usize {
    let Some(dir) = spool_dir() else {
        return 0;
    };
    let Ok(read_dir) = std::fs::read_dir(&dir) else {
        return 0; // no spool dir — nothing happened offline
    };
    let cutoff = now_secs().saturating_sub(approval_history::RETENTION_DAYS * 24 * 60 * 60);
    let mut imported = 0;
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(file_stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            let _ = std::fs::remove_file(&path); // unreadable junk
            continue;
        };
        let Ok(record) = serde_json::from_str::<SpoolRecord>(&raw) else {
            let _ = std::fs::remove_file(&path); // malformed spool file
            continue;
        };
        if record.spooled_at_secs < cutoff {
            // Older than the retention window: importing would just be pruned.
            let _ = std::fs::remove_file(&path);
            continue;
        }
        match spool_record_to_entry(&record, file_stem) {
            Some(history_entry) => {
                if let Err(error) = approval_history::record_raw_entry(&history_entry) {
                    eprintln!("Atoll approval spool: import failed: {error}");
                    continue; // keep the file; the spool-<stem> id makes the retry idempotent
                }
                imported += 1;
                let _ = std::fs::remove_file(&path);
            }
            None => {
                // Unknown agent or a payload that is not a permission request.
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    imported
}

fn spool_record_to_entry(record: &SpoolRecord, file_stem: &str) -> Option<ApprovalHistoryEntry> {
    let payload: Value = serde_json::from_str(&record.payload).ok()?;
    let id = format!("spool-{file_stem}");
    // Reuse the bridge payload parsers so spooled rows match live ones; the
    // requested_at is formatted from the spool time (whole seconds, the same
    // shape iso_timestamp_now produces).
    let requested_at = crate::format_unix_timestamp(record.spooled_at_secs);
    let request: PermissionRequest = match record.agent.as_str() {
        "claude" => hook_bridge::permission_request_from_claude_payload(id, payload, requested_at)?,
        "codex" => hook_bridge::permission_request_from_codex_payload(id, payload, requested_at)?,
        "cursor" => hook_bridge::permission_request_from_cursor_payload(id, payload, requested_at)?,
        "zcode" => hook_bridge::permission_request_from_zcode_payload(id, payload, requested_at)?,
        "gemini" => hook_bridge::permission_request_from_gemini_payload(id, payload, requested_at)?,
        "opencode" => {
            hook_bridge::permission_request_from_opencode_payload(id, payload, requested_at)?
        }
        _ => return None,
    };
    Some(ApprovalHistoryEntry {
        id: request.id.clone(),
        agent: crate::token_history::agent_kind_key(&request.agent),
        session_id: request.session.clone(),
        command: request.command.clone(),
        detail: format!(
            "{} Resolved outside Atoll (Atoll was not running).",
            request.command
        ),
        cwd: request.cwd.clone(),
        tool_input: request.tool_input.clone(),
        transcript_path: request.transcript_path.clone(),
        requested_at: record.spooled_at_secs,
        decided_at: Some(now_secs()),
        status: HistoryStatus::AnsweredElsewhere,
        host: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval_history::{query_history, ApprovalHistoryQuery};

    fn with_temp_dirs(test_name: &str) -> (std::sync::MutexGuard<'static, ()>, PathBuf, PathBuf) {
        let guard = crate::approval_history::approval_history_env_lock();
        let tag = format!("{}-{}", std::process::id(), test_name);
        let spool = std::env::temp_dir().join(format!("atoll-spool-{tag}"));
        let db = std::env::temp_dir().join(format!("atoll-spool-db-{tag}.db"));
        let _ = std::fs::remove_dir_all(&spool);
        let _ = std::fs::remove_file(&db);
        std::fs::create_dir_all(&spool).expect("create spool dir");
        std::env::set_var("ATOLL_HOOK_SPOOL_DIR", spool.to_string_lossy().as_ref());
        std::env::set_var("ATOLL_APPROVAL_HISTORY_PATH", db.to_string_lossy().as_ref());
        (guard, spool, db)
    }

    fn write_spool(dir: &Path, name: &str, agent: &str, spooled_at_secs: u64, event: &str) {
        let payload = serde_json::json!({
            "hook_event_name": event,
            "session_id": "sess-spool-1",
            "cwd": "/tmp/proj",
            "tool_name": "Bash",
            "tool_input": { "command": "echo hi" },
        });
        let record = SpoolRecord {
            agent: agent.into(),
            payload: payload.to_string(),
            spooled_at_secs,
        };
        std::fs::write(
            dir.join(name),
            serde_json::to_string(&record).expect("serialize record"),
        )
        .expect("write spool file");
    }

    fn cleanup(spool: &Path, db: &Path) {
        let _ = std::fs::remove_dir_all(spool);
        let _ = std::fs::remove_file(db);
    }

    #[test]
    fn imports_spooled_request_and_clears_the_spool() {
        let (_guard, spool, db) = with_temp_dirs("import");
        let now = now_secs();
        write_spool(
            &spool,
            "zcode-1000-abc.json",
            "zcode",
            now,
            "PermissionRequest",
        );

        let imported = import_spooled_requests();
        assert_eq!(imported, 1);
        assert!(std::fs::read_dir(&spool).unwrap().next().is_none());

        let page = query_history(&ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        let item = &page.items[0];
        assert_eq!(item.agent, "zcode");
        assert_eq!(item.command, "Bash: echo hi");
        assert_eq!(item.requested_at, now);
        assert!(item.id.starts_with("spool-zcode-1000-abc"));
        assert!(matches!(item.status, HistoryStatus::AnsweredElsewhere));
        assert!(item.detail.contains("Resolved outside Atoll"));
        cleanup(&spool, &db);
    }

    #[test]
    fn reimport_of_the_same_event_does_not_duplicate() {
        let (_guard, spool, db) = with_temp_dirs("idempotent");
        let now = now_secs();
        write_spool(
            &spool,
            "claude-2000-xyz.json",
            "claude",
            now,
            "PermissionRequest",
        );
        import_spooled_requests();
        // Simulate a crash between insert and delete: rewrite the same spool
        // file and import again.
        write_spool(
            &spool,
            "claude-2000-xyz.json",
            "claude",
            now,
            "PermissionRequest",
        );
        import_spooled_requests();

        let page = query_history(&ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, "spool-claude-2000-xyz");
        cleanup(&spool, &db);
    }

    #[test]
    fn drops_spool_files_older_than_retention() {
        let (_guard, spool, db) = with_temp_dirs("retention");
        let ancient = now_secs() - approval_history::RETENTION_DAYS * 24 * 60 * 60 - 60;
        write_spool(
            &spool,
            "codex-3000-old.json",
            "codex",
            ancient,
            "PermissionRequest",
        );

        assert_eq!(import_spooled_requests(), 0);
        assert!(std::fs::read_dir(&spool).unwrap().next().is_none());
        let page = query_history(&ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 0);
        cleanup(&spool, &db);
    }

    #[test]
    fn drops_unusable_spool_files_without_importing() {
        let (_guard, spool, db) = with_temp_dirs("unusable");
        let now = now_secs();
        // Unknown agent, and an observer event that is not a permission request.
        write_spool(
            &spool,
            "other-4000-a.json",
            "unknown-agent",
            now,
            "PermissionRequest",
        );
        write_spool(&spool, "zcode-4001-b.json", "zcode", now, "SessionStart");
        std::fs::write(spool.join("junk-4002-c.json"), "not json").expect("write junk");

        assert_eq!(import_spooled_requests(), 0);
        assert!(std::fs::read_dir(&spool).unwrap().next().is_none());
        let page = query_history(&ApprovalHistoryQuery::default()).expect("query");
        assert_eq!(page.total, 0);
        cleanup(&spool, &db);
    }

    #[test]
    fn missing_spool_dir_is_not_an_error() {
        let (_guard, spool, db) = with_temp_dirs("missing");
        let _ = std::fs::remove_dir_all(&spool);
        assert_eq!(import_spooled_requests(), 0);
        cleanup(&spool, &db);
        std::env::remove_var("ATOLL_HOOK_SPOOL_DIR");
        std::env::remove_var("ATOLL_APPROVAL_HISTORY_PATH");
    }
}
