use super::*;
use serde_json::json;

fn test_app_state() -> AppState {
    AppState {
        requests: Mutex::new(Vec::new()),
        session_request_totals: Mutex::new(HashMap::new()),
        hook_waiters: Mutex::new(HashMap::new()),
        auto_approve_sessions: Mutex::new(HashSet::new()),
        compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
        compact_left_width: Mutex::new(0.0),
        presentation_generation: Arc::new(AtomicU64::new(0)),
        home_bounds: Mutex::new(None),
        notch_metrics: Mutex::new(NotchMetrics::default()),
        session_last_seen: Mutex::new(HashMap::new()),
        session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
        subagent_retention_secs: Mutex::new(DEFAULT_SUBAGENT_RETENTION_SECS),
        session_token_usage: Mutex::new(HashMap::new()),
        session_token_usage_by_model: Mutex::new(HashMap::new()),
        session_agent_map: Mutex::new(HashMap::new()),
        token_usage_file_offsets: Mutex::new(HashMap::new()),
        token_usage_day: Mutex::new(current_local_day_key()),
        startup_daily_floor: Mutex::new(TokenUsage::default()),
        startup_daily_floor_by_model: Mutex::new(HashMap::new()),
        absolute_token_sessions: Mutex::new(HashSet::new()),
        daily_tokens_baseline: Mutex::new(TokenUsage::default()),
        known_sessions: Mutex::new(HashMap::new()),
        pinned_sessions: Mutex::new(HashSet::new()),
        previous_app_pid: Mutex::new(None),
        last_listening_online: Mutex::new(None),
        last_hook_health: Mutex::new(None),
        bridge_port: AtomicU16::new(0),
        bridge_auth_token: Mutex::new(uuid::Uuid::new_v4().to_string()),
        last_bridge_reachable: Mutex::new(None),
        active_subagents: Mutex::new(Vec::new()),
        cursor_subagent_conversations: Mutex::new(HashMap::new()),
        cursor_lifecycle_token_sessions: Mutex::new(HashSet::new()),
        last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
        snapshot_debounce_generation: AtomicU64::new(0),
        snapshot_debounce_worker_running: AtomicBool::new(false),
        last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
        last_hook_activity: Mutex::new(Instant::now()),
        token_history_dirty: AtomicBool::new(false),
        transcript_cache: Mutex::new(TranscriptCache::default()),
        media_card_enabled: Mutex::new(true),
        artwork_backdrop_enabled: Mutex::new(false),
        bluetooth_battery_card_enabled: Mutex::new(false),
        bluetooth_battery_alert_enabled: Mutex::new(false),
        bluetooth_battery_alert_threshold: Mutex::new(20),
        clipboard_history: Mutex::new(Vec::new()),
        clipboard_history_limit: Mutex::new(clipboard_history::DEFAULT_MAX_ENTRIES),
        clipboard_history_enabled: Mutex::new(false),
        file_station: Mutex::new(Vec::new()),
        lyrics_enabled: Mutex::new(false),
        lyrics: Mutex::new(None),
        lyrics_track_key: Mutex::new(String::new()),
        approval_notice_mode: Mutex::new(APPROVAL_NOTICE_INTERRUPT.to_string()),
        notification_language: Mutex::new("en".to_string()),
        preferred_monitor: Mutex::new(None),
        global_shortcuts: Mutex::new(shortcuts::GlobalShortcutsState::default()),
    }
}

#[test]
fn payload_helpers_support_claude_and_cursor_fields() {
    let claude = json!({
        "agent_id": "agent-claude",
        "session_id": "sess-claude",
        "agent_type": "explore"
    });
    assert_eq!(payload_subagent_id(&claude), Some("agent-claude"));
    assert_eq!(
        payload_subagent_parent_session_id(&claude),
        Some("sess-claude")
    );
    assert_eq!(payload_subagent_type(&claude), "explore");

    let cursor = json!({
        "subagent_id": "sub-123",
        "conversation_id": "conv-parent",
        "subagent_type": "generalPurpose"
    });
    assert_eq!(payload_subagent_id(&cursor), Some("sub-123"));
    assert_eq!(
        payload_subagent_parent_session_id(&cursor),
        Some("conv-parent")
    );
    assert_eq!(payload_subagent_type(&cursor), "generalPurpose");
}

#[test]
fn cursor_subagent_start_registers_subagent() {
    let state = test_app_state();
    let payload = json!({
        "hook_event_name": "subagentStart",
        "subagent_id": "sub-abc",
        "conversation_id": "conv-parent",
        "subagent_type": "explore",
        "transcript_path": "/tmp/main.jsonl"
    });
    register_subagent_start(&state, &payload, AgentKind::Cursor);

    let subagents = state.active_subagents.lock().expect("lock");
    assert_eq!(subagents.len(), 1);
    assert_eq!(subagents[0].agent_id, "sub-abc");
    assert_eq!(subagents[0].session_id, "conv-parent");
    assert_eq!(subagents[0].agent_type, "explore");
    assert!(subagents[0].completed_at.is_none());
}

#[test]
fn cursor_subagent_stop_completes_without_agent_id() {
    let state = test_app_state();
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore"
        }),
        AgentKind::Cursor,
    );

    complete_subagent(
        &state,
        &json!({
            "hook_event_name": "subagentStop",
            "conversation_id": "conv-parent",
            "subagent_type": "explore",
            "summary": "Found auth module",
            "agent_transcript_path": "/tmp/subagents/agent-sub-abc.jsonl"
        }),
    );

    let subagents = state.active_subagents.lock().expect("lock");
    assert_eq!(subagents.len(), 1);
    assert!(subagents[0].completed_at.is_some());
    assert_eq!(
        subagents[0].agent_transcript_path.as_deref(),
        Some("/tmp/subagents/agent-sub-abc.jsonl")
    );
    assert_eq!(
        subagents[0].last_message.as_deref(),
        Some("Found auth module")
    );
}

#[test]
fn cursor_subagent_conversation_maps_to_parent_session() {
    let state = test_app_state();
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore"
        }),
        AgentKind::Cursor,
    );

    let parent = resolve_cursor_session_for_payload(
        &state,
        &json!({
            "conversation_id": "conv-subagent-new",
            "hook_event_name": "preToolUse"
        }),
    );
    assert_eq!(parent.as_deref(), Some("conv-parent"));

    let map = state.cursor_subagent_conversations.lock().expect("lock");
    assert_eq!(
        map.get("conv-subagent-new").map(String::as_str),
        Some("conv-parent")
    );

    let subagents = state.active_subagents.lock().expect("lock");
    assert_eq!(
        subagents[0].conversation_id.as_deref(),
        Some("conv-subagent-new")
    );
}

#[test]
fn derive_subagent_transcript_path_uses_parent_directory() {
    let parent_uuid = "819943d1-a823-47ce-bef3-97ca63fa0f34";
    let sub_uuid = "60bcad01-8db6-4e9f-91b3-d3e55f2b504c";
    let dir = std::env::temp_dir().join(format!("atoll-subagent-derive-{}", std::process::id()));
    let parent_dir = dir.join(parent_uuid);
    let subagents_dir = parent_dir.join("subagents");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
    std::fs::write(&main, "{}").expect("write parent transcript");
    let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
    std::fs::write(&sub_path, "{}").expect("write subagent transcript");
    let main_str = main.to_string_lossy().into_owned();

    let resolved =
        derive_subagent_transcript_path(Some(&main_str), "call_tool_id", Some(sub_uuid), None)
            .expect("resolved path");

    assert_eq!(resolved, sub_path.to_string_lossy().into_owned());
    assert!(
        !resolved.contains(&format!("{parent_uuid}/{parent_uuid}/subagents")),
        "should not nest an extra parent-uuid directory"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn cursor_subagent_conversation_binding_updates_transcript_path() {
    let state = test_app_state();
    let parent_uuid = "conv-parent";
    let sub_uuid = "conv-subagent-new";
    let dir = std::env::temp_dir().join(format!("atoll-subagent-bind-{}", std::process::id()));
    let parent_dir = dir.join(parent_uuid);
    let subagents_dir = parent_dir.join("subagents");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
    std::fs::write(&main, "{}").expect("write parent transcript");
    let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
    std::fs::write(&sub_path, "{}").expect("write subagent transcript");
    let main_str = main.to_string_lossy().into_owned();

    register_known_session(
        &state,
        parent_uuid,
        AgentKind::Cursor,
        "/tmp/project",
        Some(&main_str),
    );
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore",
            "transcript_path": main_str
        }),
        AgentKind::Cursor,
    );

    let parent = resolve_cursor_session_for_payload(
        &state,
        &json!({
            "conversation_id": sub_uuid,
            "hook_event_name": "preToolUse"
        }),
    );
    assert_eq!(parent.as_deref(), Some(parent_uuid));

    let subagents = state.active_subagents.lock().expect("lock");
    assert_eq!(
        subagents[0].agent_transcript_path.as_deref(),
        Some(sub_path.to_string_lossy().as_ref())
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn complete_subagent_falls_back_to_conversation_transcript_path() {
    let state = test_app_state();
    let parent_uuid = "conv-parent";
    let sub_uuid = "conv-subagent-new";
    let dir = std::env::temp_dir().join(format!("atoll-subagent-complete-{}", std::process::id()));
    let parent_dir = dir.join(parent_uuid);
    let subagents_dir = parent_dir.join("subagents");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
    std::fs::write(&main, "{}").expect("write parent transcript");
    let sub_path = subagents_dir.join(format!("{sub_uuid}.jsonl"));
    std::fs::write(&sub_path, "{}").expect("write subagent transcript");
    let main_str = main.to_string_lossy().into_owned();

    register_known_session(
        &state,
        parent_uuid,
        AgentKind::Cursor,
        "/tmp/project",
        Some(&main_str),
    );
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore",
            "transcript_path": main_str
        }),
        AgentKind::Cursor,
    );
    let _ = resolve_cursor_session_for_payload(
        &state,
        &json!({
            "conversation_id": sub_uuid,
            "hook_event_name": "preToolUse"
        }),
    );

    complete_subagent(
        &state,
        &json!({
            "hook_event_name": "subagentStop",
            "conversation_id": "conv-parent",
            "subagent_type": "explore",
            "summary": "Done"
        }),
    );

    let subagents = state.active_subagents.lock().expect("lock");
    assert!(subagents[0].completed_at.is_some());
    assert_eq!(
        subagents[0].agent_transcript_path.as_deref(),
        Some(sub_path.to_string_lossy().as_ref())
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn parent_conversation_id_is_not_treated_as_subagent_session() {
    let state = test_app_state();
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore"
        }),
        AgentKind::Cursor,
    );

    let parent = resolve_cursor_session_for_payload(
        &state,
        &json!({
            "conversation_id": "conv-parent",
            "hook_event_name": "preToolUse"
        }),
    );
    assert!(parent.is_none());
}

#[test]
fn cursor_subagent_pretooluse_request_attributes_to_parent() {
    let state = test_app_state();
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": "sub-abc",
            "conversation_id": "conv-parent",
            "subagent_type": "explore"
        }),
        AgentKind::Cursor,
    );

    let payload = json!({
        "hook_event_name": "preToolUse",
        "conversation_id": "conv-subagent",
        "cwd": "/tmp/project",
        "tool_name": "Shell",
        "tool_input": { "command": "echo hi" },
        "tool_use_id": "tool-1"
    });
    let mut request = hook_bridge::permission_request_from_cursor_payload(
        "req-1".into(),
        payload.clone(),
        "2026-01-01T00:00:00Z".into(),
    )
    .expect("cursor request");
    assert_eq!(request.session, "conv-subagent");

    hook_bridge::attribute_cursor_request_to_parent_session(&state, &payload, &mut request);
    assert_eq!(request.session, "conv-parent");
}

#[test]
fn bind_cursor_subagent_conversation_rewrites_ghost_requests() {
    let state = test_app_state();
    register_known_session(
        &state,
        "conv-subagent",
        AgentKind::Cursor,
        "/tmp/project",
        Some("/tmp/project/sub.jsonl"),
    );
    {
        let mut requests = state.requests.lock().expect("lock");
        requests.push(PermissionRequest {
            id: "req-ghost".into(),
            tool_use_id: None,
            agent: AgentKind::Cursor,
            session: "conv-subagent".into(),
            command: "Bash: echo".into(),
            detail: "echo".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-10T08:00:00Z".into(),
            status: PermissionStatus::Approved,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        });
    }

    bind_cursor_subagent_conversation(&state, "conv-subagent", "conv-parent");

    let requests = state.requests.lock().expect("lock");
    assert_eq!(requests[0].session, "conv-parent");
    assert!(!state
        .known_sessions
        .lock()
        .expect("lock")
        .contains_key("conv-subagent"));
    assert_eq!(
        state
            .cursor_subagent_conversations
            .lock()
            .expect("lock")
            .get("conv-subagent")
            .map(String::as_str),
        Some("conv-parent")
    );
}

#[test]
fn snapshot_excludes_bound_subagent_conversation_ids() {
    let requests = vec![PermissionRequest {
        id: "req-parent".into(),
        tool_use_id: None,
        agent: AgentKind::Cursor,
        session: "conv-parent".into(),
        command: "Bash: ls".into(),
        detail: "ls".into(),
        cwd: "/tmp/project".into(),
        requested_at: "2026-06-10T08:00:00Z".into(),
        status: PermissionStatus::Approved,
        archived: false,
        supports_always: false,
        transcript_path: None,
        tool_input: None,
    }];
    let mut known_sessions = HashMap::new();
    known_sessions.insert(
        "conv-subagent".into(),
        KnownSession {
            agent: AgentKind::Cursor,
            cwd: "/tmp/project".into(),
            transcript_path: Some("/tmp/sub.jsonl".into()),
            last_activity: "2026-06-10T08:01:00Z".into(),
            host: platform::SessionHost::CursorIde,
            conversation_id: Some("conv-subagent".into()),
        },
    );
    let mut excluded = HashSet::new();
    excluded.insert("conv-subagent".into());

    let snapshot = snapshot_from(
        &requests,
        &HashMap::new(),
        900,
        &HashMap::new(),
        &known_sessions,
        &HashSet::new(),
        true,
        &excluded,
    );

    let session_ids: Vec<&str> = snapshot
        .sessions
        .iter()
        .map(|session| session.session_id.as_str())
        .collect();
    assert_eq!(session_ids, vec!["conv-parent"]);
    assert!(!session_ids.contains(&"conv-subagent"));
}

#[test]
fn sanitize_subagent_id_strips_newlines_for_transcript_path() {
    let agent_id = "call_abc\nfc_def";
    let sanitized = sanitize_subagent_id_for_filename(agent_id);
    assert!(!sanitized.contains('\n'));
    assert!(sanitized.contains("call_abc"));
}

fn test_session_summary(session_id: &str) -> SessionSummary {
    SessionSummary {
        session_id: session_id.to_string(),
        agent: AgentKind::Cursor,
        cwd: "/tmp/project".into(),
        pending_count: 0,
        total_count: 0,
        last_activity: "2026-06-10T08:10:00Z".into(),
        transcript_path: None,
        pinned: false,
        session_host: platform::SessionHost::Unknown,
        active_subagents: Vec::new(),
    }
}

fn test_active_subagent(
    agent_id: &str,
    session_id: &str,
    completed_at: Option<String>,
    archived: bool,
) -> ActiveSubagent {
    ActiveSubagent {
        agent_id: agent_id.to_string(),
        session_id: session_id.to_string(),
        agent_kind: AgentKind::Cursor,
        agent_type: agent_id.to_string(),
        started_at: "2026-06-10T08:00:00Z".into(),
        agent_transcript_path: None,
        completed_at,
        archived,
        last_message: None,
        conversation_id: None,
    }
}

#[test]
fn reconcile_incomplete_subagents_refreshes_path_and_terminal_message() {
    let state = test_app_state();
    let parent_uuid = "conv-parent-reconcile";
    let agent_id = "sub-reconcile";
    let dir = std::env::temp_dir().join(format!("atoll-subagent-reconcile-{}", std::process::id()));
    let parent_dir = dir.join(parent_uuid);
    let subagents_dir = parent_dir.join("subagents");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&subagents_dir).expect("create subagents dir");
    let main = parent_dir.join(format!("{parent_uuid}.jsonl"));
    std::fs::write(&main, "{}").expect("write parent transcript");
    let sub_path = subagents_dir.join(format!("agent-{agent_id}.jsonl"));
    let terminal_entry = json!({
        "type": "assistant",
        "message": {
            "content": [{
                "type": "text",
                "text": "Request interrupted by user for tool use"
            }]
        }
    });
    std::fs::write(&sub_path, format!("{terminal_entry}\n")).expect("write sub transcript");
    let main_str = main.to_string_lossy().into_owned();

    register_known_session(
        &state,
        parent_uuid,
        AgentKind::Cursor,
        "/tmp/project",
        Some(&main_str),
    );
    register_subagent_start(
        &state,
        &json!({
            "subagent_id": agent_id,
            "conversation_id": parent_uuid,
            "subagent_type": "explore"
        }),
        AgentKind::Cursor,
    );

    reconcile_incomplete_subagents(&state);

    let subagents = state.active_subagents.lock().expect("lock");
    assert_eq!(subagents.len(), 1);
    assert_eq!(
        subagents[0].agent_transcript_path.as_deref(),
        Some(sub_path.to_string_lossy().as_ref())
    );
    assert!(subagents[0].completed_at.is_some());
    assert_eq!(
        subagents[0].last_message.as_deref(),
        Some("Request interrupted by user for tool use")
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn archive_completed_subagents_keeps_running_sibling_visible() {
    let state = test_app_state();
    let now = parse_iso_timestamp_secs("2026-06-10T08:10:00Z");
    let completed_at = Some(format_unix_timestamp(now - 30));
    let mut completed = test_active_subagent("done", "session-a", completed_at, false);
    completed.conversation_id = Some("conv-done".into());
    let running = test_active_subagent("running", "session-a", None, false);
    {
        let mut subagents = state.active_subagents.lock().expect("lock");
        subagents.push(completed);
        subagents.push(running);
    }
    state
        .cursor_subagent_conversations
        .lock()
        .expect("lock")
        .insert("conv-done".into(), "session-a".into());

    let conv_ids = archive_completed_subagents_in_state(&state, "session-a");
    assert_eq!(conv_ids, vec!["conv-done".to_string()]);
    for conv_id in conv_ids {
        unbind_cursor_subagent_conversation(&state, Some(&conv_id));
    }

    assert!(!state
        .cursor_subagent_conversations
        .lock()
        .expect("lock")
        .contains_key("conv-done"));

    let active_subagents = state.active_subagents.lock().expect("lock").clone();
    assert!(active_subagents
        .iter()
        .any(|sub| sub.agent_id == "done" && sub.archived));
    assert!(active_subagents
        .iter()
        .any(|sub| sub.agent_id == "running" && !sub.archived));

    let mut sessions = vec![test_session_summary("session-a")];
    assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 60, now);
    let visible_ids: Vec<&str> = sessions[0]
        .active_subagents
        .iter()
        .map(|sub| sub.agent_id.as_str())
        .collect();
    assert_eq!(visible_ids, vec!["running"]);
}

#[test]
fn snapshot_subagent_assignment_groups_by_session_without_changing_filters_or_order() {
    let now = parse_iso_timestamp_secs("2026-06-10T08:10:00Z");
    let recent_completed = Some(format_unix_timestamp(now - 30));
    let old_completed = Some(format_unix_timestamp(now - 120));
    let active_subagents = vec![
        test_active_subagent("a-running", "session-a", None, false),
        test_active_subagent("b-running", "session-b", None, false),
        test_active_subagent("a-old", "session-a", old_completed.clone(), false),
        test_active_subagent("a-archived", "session-a", None, true),
        test_active_subagent("a-recent", "session-a", recent_completed, false),
        test_active_subagent("orphan", "missing-session", None, false),
    ];
    let mut sessions = vec![
        test_session_summary("session-a"),
        test_session_summary("session-b"),
    ];

    assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 60, now);

    let session_a_ids: Vec<&str> = sessions[0]
        .active_subagents
        .iter()
        .map(|sub| sub.agent_id.as_str())
        .collect();
    let session_b_ids: Vec<&str> = sessions[1]
        .active_subagents
        .iter()
        .map(|sub| sub.agent_id.as_str())
        .collect();
    assert_eq!(session_a_ids, vec!["a-running", "a-recent"]);
    assert_eq!(session_b_ids, vec!["b-running"]);

    assign_active_subagents_to_sessions(&mut sessions, &active_subagents, 0, now);
    let session_a_ids: Vec<&str> = sessions[0]
        .active_subagents
        .iter()
        .map(|sub| sub.agent_id.as_str())
        .collect();
    assert_eq!(session_a_ids, vec!["a-running", "a-old", "a-recent"]);
}
