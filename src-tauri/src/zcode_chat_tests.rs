use super::{
    parse_zcode_db_session_path, read_zcode_chat_messages, truncate_transcript_content,
    zcode_db_session_path, TOKEN_HISTORY_ENV_LOCK, TRANSCRIPT_MAX_MESSAGES,
    TRANSCRIPT_MESSAGE_MAX_CHARS,
};
use crate::zcode_token_tests::HomeGuard;
use serde_json::{json, Value};
use std::path::Path;

const SESSION: &str = "sess_11111111-2222-4333-8444-555555555555";

fn write_zcode_db(home: &Path, session_id: &str, messages: &[Value]) {
    let db_path = home.join(".zcode").join("cli").join("db").join("db.sqlite");
    std::fs::create_dir_all(db_path.parent().unwrap()).expect("create db dir");
    let connection = rusqlite::Connection::open(&db_path).expect("open fixture db");
    connection
            .execute_batch(
                "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, sequence INTEGER, data TEXT);
                 CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, sequence INTEGER, data TEXT);",
            )
            .expect("create fixture tables");
    for (index, message) in messages.iter().enumerate() {
        let message_id = message["id"].as_str().expect("message id");
        let parts = message["parts"].as_array().cloned().unwrap_or_default();
        connection
            .execute(
                "INSERT INTO message (id, session_id, sequence, data) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    message_id,
                    session_id,
                    index as i64,
                    json!({ "role": message["role"], "id": message_id }).to_string()
                ],
            )
            .expect("insert message");
        for (part_index, part) in parts.iter().enumerate() {
            connection
                    .execute(
                        "INSERT INTO part (id, message_id, session_id, sequence, data) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            format!("{message_id}-part-{part_index}"),
                            message_id,
                            session_id,
                            part_index as i64,
                            part.to_string()
                        ],
                    )
                    .expect("insert part");
        }
    }
}

#[test]
fn zcode_db_session_path_round_trips_and_rejects_unsafe_ids() {
    let path = zcode_db_session_path("sess_abc-123").expect("valid");
    assert_eq!(path, "zcode-db://sess_abc-123");
    assert_eq!(parse_zcode_db_session_path(&path), Some("sess_abc-123"));
    assert_eq!(parse_zcode_db_session_path("file:///tmp/x.jsonl"), None);
    assert_eq!(parse_zcode_db_session_path("zcode-db://../escape"), None);
    assert_eq!(parse_zcode_db_session_path("zcode-db://"), None);
    assert!(zcode_db_session_path("/abs/path").is_none());
}

#[test]
fn reads_zcode_chat_from_sqlite() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let home = HomeGuard::new("chat");

    write_zcode_db(
        &home.home,
        SESSION,
        &[
            json!({
                "id": "msg_u1", "role": "user",
                "parts": [
                    { "type": "text", "text": "TodoWrite reminder noise", "synthetic": true },
                    { "type": "text", "text": "帮我看看这个文件" }
                ]
            }),
            json!({
                "id": "msg_a1", "role": "assistant",
                "parts": [
                    { "type": "step-start" },
                    { "type": "reasoning", "text": "internal reasoning trace" },
                    { "type": "text", "text": "我先查一下" },
                    { "type": "tool", "tool": "Bash", "state": { "status": "completed", "input": { "command": "ls -la" }, "output": "total 0" } },
                    { "type": "tool", "tool": "AskUserQuestion", "state": { "status": "completed", "input": { "questions": [{ "question": "先做哪个?", "options": [] }] }, "output": "User has answered your questions: \"先做哪个?\"=\"Hook bridge, Plan mode UI\"" } },
                    { "type": "step-finish" }
                ]
            }),
            json!({ "id": "msg_u2", "role": "user", "parts": [{ "type": "text", "text": "   " }] }),
        ],
    );

    let messages = read_zcode_chat_messages(SESSION).expect("read chat");
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "帮我看看这个文件");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "我先查一下");
    assert_eq!(messages[2].role, "assistant");
    assert_eq!(messages[2].content, "");
    assert_eq!(messages[2].tool_name.as_deref(), Some("Bash"));
    assert_eq!(messages[2].tool_input, Some(json!({ "command": "ls -la" })));
    // Non-question tool outputs stay out of the transcript.
    assert_eq!(messages[2].tool_output, None);
    assert_eq!(messages[3].tool_name.as_deref(), Some("AskUserQuestion"));
    assert_eq!(
        messages[3].tool_output.as_deref(),
        Some("User has answered your questions: \"先做哪个?\"=\"Hook bridge, Plan mode UI\"")
    );

    // Other sessions stay isolated; invalid ids are rejected before I/O.
    assert!(
        read_zcode_chat_messages("sess_ffffffff-ffff-4fff-8fff-ffffffffffff")
            .expect("unknown session reads empty")
            .is_empty()
    );
    assert!(read_zcode_chat_messages("../escape").is_err());
}

#[test]
fn zcode_chat_keeps_only_the_newest_messages() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let home = HomeGuard::new("chat-limit");

    let total = TRANSCRIPT_MAX_MESSAGES + 7;
    let messages: Vec<Value> = (0..total)
        .map(|i| {
            json!({
                "id": format!("msg_{i:04}"), "role": "user",
                "parts": [{ "type": "text", "text": format!("message {i}") }]
            })
        })
        .collect();
    write_zcode_db(&home.home, SESSION, &messages);

    let read = read_zcode_chat_messages(SESSION).expect("read chat");
    assert_eq!(read.len(), TRANSCRIPT_MAX_MESSAGES);
    assert_eq!(
        read[0].content,
        format!("message {}", total - TRANSCRIPT_MAX_MESSAGES)
    );
    assert_eq!(
        read[read.len() - 1].content,
        format!("message {}", total - 1)
    );
}

#[test]
fn zcode_chat_errors_without_database() {
    let _env_lock = TOKEN_HISTORY_ENV_LOCK.lock().expect("env lock");
    let _home = HomeGuard::new("chat-missing-db");
    let error = read_zcode_chat_messages(SESSION).expect_err("no db fixture");
    assert!(error.contains("ZCode database"), "{error}");
}

#[test]
fn transcript_truncation_marker_is_applied() {
    let long = "字".repeat(TRANSCRIPT_MESSAGE_MAX_CHARS + 1);
    let truncated = truncate_transcript_content(long.clone());
    assert_eq!(
        truncated.chars().take(100).collect::<String>(),
        long.chars().take(100).collect::<String>()
    );
    assert!(truncated.contains("[message truncated by Atoll]"));
    let short = "short message";
    assert_eq!(truncate_transcript_content(short.to_string()), short);
}

#[test]
fn snapshot_exposes_zcode_db_path_for_chat() {
    use super::{
        iso_timestamp_now, snapshot_from, KnownSession, PermissionRequest, PermissionStatus,
    };
    use super::{platform, AgentKind};
    use std::collections::{HashMap, HashSet};

    let ephemeral = "/tmp/atoll-ephemeral-hook-transcript.jsonl";
    let virtual_path = "zcode-db://sess_11111111-2222-4333-8444-555555555555";
    let known_sessions = HashMap::from([(
        SESSION.to_string(),
        KnownSession {
            agent: AgentKind::Zcode,
            cwd: "/tmp/project".into(),
            transcript_path: Some(ephemeral.into()),
            last_activity: iso_timestamp_now(),
            host: platform::SessionHost::ZcodeCli,
            conversation_id: None,
        },
    )]);
    let make_request = |archived: bool| PermissionRequest {
        id: "req-zcode-1".into(),
        tool_use_id: None,
        agent: AgentKind::Zcode,
        tool_name: String::new(),
        session: SESSION.into(),
        command: "Bash: ls".into(),
        detail: "List files".into(),
        cwd: "/tmp/project".into(),
        requested_at: iso_timestamp_now(),
        status: PermissionStatus::Approved,
        archived,
        supports_always: false,
        transcript_path: Some(ephemeral.into()),
        tool_input: None,
    };

    // Live request source.
    let snapshot = snapshot_from(
        &[make_request(false)],
        &HashMap::new(),
        900,
        &HashMap::new(),
        &known_sessions,
        &HashSet::new(),
        true,
        &HashSet::new(),
    );
    let session = snapshot
        .sessions
        .iter()
        .find(|s| s.session_id == SESSION)
        .expect("zcode session in snapshot");
    assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));

    // Archived-retention source: every request resolved, session rebuilt
    // from history carrying the raw ephemeral hook path.
    let snapshot = snapshot_from(
        &[make_request(true)],
        &HashMap::new(),
        900,
        &HashMap::new(),
        &known_sessions,
        &HashSet::new(),
        true,
        &HashSet::new(),
    );
    let session = snapshot
        .sessions
        .iter()
        .find(|s| s.session_id == SESSION)
        .expect("retained zcode session");
    assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));

    // Known-session-only source (observer events, no requests at all).
    let snapshot = snapshot_from(
        &[],
        &HashMap::new(),
        900,
        &HashMap::new(),
        &known_sessions,
        &HashSet::new(),
        true,
        &HashSet::new(),
    );
    let session = snapshot
        .sessions
        .iter()
        .find(|s| s.session_id == SESSION)
        .expect("known zcode session");
    assert_eq!(session.transcript_path.as_deref(), Some(virtual_path));
}
