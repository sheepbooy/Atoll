use super::*;
use crate::core_tests::test_app_state;

#[test]
fn rollover_completes_while_requests_mutex_is_held() {
    let _env = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let path =
        std::env::temp_dir().join(format!("atoll-rollover-lock-{}.json", uuid::Uuid::new_v4()));
    std::env::set_var("ATOLL_TOKEN_HISTORY_PATH", &path);

    let state = Arc::new(test_app_state());
    *state.token_usage_day.lock().expect("day") = "2000-01-01".into();
    state.session_token_usage.lock().expect("usage").insert(
        "session-a".into(),
        TokenUsage {
            input_tokens: 1,
            ..TokenUsage::default()
        },
    );
    state
        .session_agent_map
        .lock()
        .expect("agent map")
        .insert("session-a".into(), "codex".into());
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let worker_state = Arc::clone(&state);
    std::thread::spawn(move || {
        let _requests = worker_state.requests.lock().expect("requests");
        roll_over_token_usage_if_needed(&worker_state);
        let _ = tx.send(());
    });
    assert!(rx.recv_timeout(Duration::from_secs(2)).is_ok());

    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("json.bak"));
    let _ = std::fs::remove_file(path.with_extension("json.tmp"));
}

#[test]
fn active_session_tokens_only_sum_visible_sessions() {
    let requests = vec![PermissionRequest {
        id: "req-active".into(),
        tool_use_id: None,
        agent: AgentKind::Claude,
        tool_name: String::new(),
        session: "session-active".into(),
        command: "Bash: ls".into(),
        detail: String::new(),
        cwd: "/tmp/active".into(),
        requested_at: iso_timestamp_now(),
        status: PermissionStatus::Approved,
        archived: false,
        supports_always: false,
        transcript_path: None,
        tool_input: None,
    }];
    let token_usage = HashMap::from([
        (
            "session-active".into(),
            TokenUsage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        ),
        (
            "session-expired".into(),
            TokenUsage {
                input_tokens: 200,
                output_tokens: 80,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        ),
    ]);

    let snapshot = snapshot_from(
        &requests,
        &HashMap::new(),
        900,
        &token_usage,
        &HashMap::new(),
        &HashSet::new(),
        true,
        &HashSet::new(),
    );

    assert_eq!(snapshot.daily_tokens.input_tokens, 300);
    assert_eq!(snapshot.daily_tokens.output_tokens, 130);
    assert_eq!(snapshot.active_session_tokens.input_tokens, 100);
    assert_eq!(snapshot.active_session_tokens.output_tokens, 50);
}

#[test]
fn cursor_after_agent_response_accumulates_tokens() {
    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "cursor-after-agent-response"
    ));
    let _ = std::fs::remove_file(&history_path);
    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    let state = test_app_state();
    let session_id = "conv-ask-tokens";
    let payload = json!({
        "conversation_id": session_id,
        "input_tokens": 1200,
        "output_tokens": 300
    });

    ingest_cursor_token_usage_from_payload(&state, session_id, &payload, "afterAgentResponse")
        .expect("token ingest");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(session_id)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 1200);
    assert_eq!(usage.output_tokens, 300);

    let follow_up = json!({
        "conversation_id": session_id,
        "token_usage": {
            "input_tokens": 400,
            "output_tokens": 100
        }
    });
    ingest_cursor_token_usage_from_payload(&state, session_id, &follow_up, "afterAgentResponse")
        .expect("follow-up ingest");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(session_id)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 1600);
    assert_eq!(usage.output_tokens, 400);

    let _ = std::fs::remove_file(&history_path);
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn cursor_token_ingest_accepts_usage_aliases() {
    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "cursor-usage-aliases"
    ));
    let _ = std::fs::remove_file(&history_path);
    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    let state = test_app_state();
    let session_id = "conv-usage-aliases";
    let payload = json!({
        "conversation_id": session_id,
        "usage": {
            "prompt_tokens": "1200",
            "completion_tokens": 300.0,
            "cache_read_input_tokens": 40,
            "cache_creation_input_tokens": 12
        }
    });

    ingest_cursor_token_usage_from_payload(&state, session_id, &payload, "afterAgentResponse")
        .expect("token ingest");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(session_id)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 1200);
    assert_eq!(usage.output_tokens, 300);
    assert_eq!(usage.cache_read_tokens, 40);
    assert_eq!(usage.cache_creation_tokens, 12);

    let _ = std::fs::remove_file(&history_path);
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn cursor_stop_token_fallback_uses_runtime_lifecycle_signal() {
    let state = test_app_state();
    let session_id = "conv-runtime-token-signal";
    let token_payload = json!({
        "conversation_id": session_id,
        "usage": {
            "prompt_tokens": 120,
            "completion_tokens": 30
        }
    });
    let empty_payload = json!({
        "conversation_id": session_id
    });

    assert!(cursor_payload_has_token_usage(&token_payload));
    assert!(!cursor_payload_has_token_usage(&empty_payload));
    assert!(crate::hook_bridge::cursor_stop_should_ingest_tokens(
        &state,
        &token_payload
    ));

    remember_cursor_lifecycle_token_session(&state, session_id);

    assert!(cursor_lifecycle_token_seen(&state, session_id));
    assert!(!crate::hook_bridge::cursor_stop_should_ingest_tokens(
        &state,
        &token_payload
    ));
}

#[test]
fn cursor_token_ingest_skips_empty_payload() {
    let state = test_app_state();
    ingest_cursor_token_usage_from_payload(
        &state,
        "conv-empty",
        &json!({ "conversation_id": "conv-empty" }),
        "afterAgentResponse",
    )
    .expect("empty ingest");

    let usage = state.session_token_usage.lock().expect("lock");
    assert!(!usage.contains_key("conv-empty"));
}

#[test]
fn archived_session_tokens_still_count_toward_daily_total() {
    let token_usage = HashMap::from([(
        "session-archived".into(),
        TokenUsage {
            input_tokens: 400,
            output_tokens: 100,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        },
    )]);

    let snapshot = snapshot_from(
        &[],
        &HashMap::new(),
        900,
        &token_usage,
        &HashMap::new(),
        &HashSet::new(),
        true,
        &HashSet::new(),
    );

    assert!(snapshot.sessions.is_empty());
    assert_eq!(snapshot.daily_tokens.input_tokens, 400);
    assert_eq!(snapshot.daily_tokens.output_tokens, 100);
    assert_eq!(snapshot.active_session_tokens.input_tokens, 0);
    assert_eq!(snapshot.active_session_tokens.output_tokens, 0);
}

#[test]
fn effective_daily_tokens_avoids_restart_transcript_double_count() {
    let startup_floor = TokenUsage {
        input_tokens: 3_000_000,
        output_tokens: 1_200_000,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    };
    let session_usage = HashMap::from([(
        "session-rescan".into(),
        TokenUsage {
            input_tokens: 2_000_000,
            output_tokens: 800_000,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        },
    )]);
    let absolute_sessions = HashSet::from(["session-rescan".into()]);

    let daily = effective_daily_tokens(&session_usage, startup_floor, &absolute_sessions);
    assert_eq!(daily.input_tokens, 3_000_000);
    assert_eq!(daily.output_tokens, 1_200_000);

    let hook_only = HashMap::from([(
        "session-new".into(),
        TokenUsage {
            input_tokens: 500,
            output_tokens: 100,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        },
    )]);
    let daily = effective_daily_tokens(&hook_only, startup_floor, &HashSet::new());
    assert_eq!(daily.input_tokens, 3_000_500);
    assert_eq!(daily.output_tokens, 1_200_100);
}

#[test]
fn effective_daily_tokens_by_model_uses_startup_floor() {
    let usage = |input: u64| TokenUsage {
        input_tokens: input,
        output_tokens: 0,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    };
    let floor = HashMap::from([("gpt-4o".into(), usage(1_000_000))]);
    let live = HashMap::from([(
        "session-new".into(),
        HashMap::from([("gpt-4o".into(), usage(200_000))]),
    )]);
    let merged = effective_daily_tokens_by_model(&live, &floor, &HashSet::new());
    assert_eq!(merged.get("gpt-4o").unwrap().input_tokens, 1_200_000);

    let absolute = HashSet::from(["session-new".into()]);
    let merged_abs = effective_daily_tokens_by_model(&live, &floor, &absolute);
    assert_eq!(merged_abs.get("gpt-4o").unwrap().input_tokens, 1_000_000);
}

#[test]
fn cursor_session_end_uses_max_for_cumulative_totals() {
    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "cursor-session-end"
    ));
    let _ = std::fs::remove_file(&history_path);
    let _ = std::fs::remove_file(history_path.with_extension("json.bak"));
    let _ = std::fs::remove_file(history_path.with_extension("json.tmp"));
    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    let state = test_app_state();
    let session_id = "conv-session-end";
    ingest_cursor_token_usage_from_payload(
        &state,
        session_id,
        &json!({ "input_tokens": 1200, "output_tokens": 300 }),
        "afterAgentResponse",
    )
    .expect("turn ingest");
    ingest_cursor_token_usage_from_payload(
        &state,
        session_id,
        &json!({ "input_tokens": 1500, "output_tokens": 400 }),
        "sessionEnd",
    )
    .expect("session end ingest");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(session_id)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 1500);
    assert_eq!(usage.output_tokens, 400);

    let _ = std::fs::remove_file(&history_path);
    let _ = std::fs::remove_file(history_path.with_extension("json.bak"));
    let _ = std::fs::remove_file(history_path.with_extension("json.tmp"));
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn rollover_flushes_previous_local_day_before_clearing_usage() {
    use chrono::{Duration, Local};

    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "rollover-test"
    ));
    let _ = std::fs::remove_file(&history_path);
    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    let state = test_app_state();
    let flushed_day = (Local::now().date_naive() - Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();
    {
        let mut usage_day = state.token_usage_day.lock().expect("lock");
        *usage_day = flushed_day.clone();
    }

    {
        let mut usage = state.session_token_usage.lock().expect("lock");
        usage.insert(
            "session-rollover".into(),
            TokenUsage {
                input_tokens: 250,
                output_tokens: 75,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        );
    }

    roll_over_token_usage_if_needed(&state);

    let usage_after = state.session_token_usage.lock().expect("lock");
    assert!(usage_after.is_empty());

    let history = token_history::get_token_history(365).expect("history");
    let flushed = history
        .days
        .iter()
        .find(|day| day.date == flushed_day)
        .expect("previous day should be persisted");
    assert_eq!(flushed.usage.input_tokens, 250);
    assert_eq!(flushed.usage.output_tokens, 75);

    let _ = std::fs::remove_file(&history_path);
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn restart_preserves_historical_days_when_sessions_are_empty() {
    use chrono::{Duration, Local};

    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "restart-test"
    ));
    let _ = std::fs::remove_file(&history_path);

    let today = Local::now().date_naive();
    let yesterday = today - Duration::days(1);
    let two_days_ago = today - Duration::days(2);
    let today_key = today.format("%Y-%m-%d").to_string();
    let yesterday_key = yesterday.format("%Y-%m-%d").to_string();
    let two_days_ago_key = two_days_ago.format("%Y-%m-%d").to_string();

    let seed = serde_json::json!({
        "version": 1,
        "timezone": "Asia/Shanghai",
        "days": {
            &two_days_ago_key: {
                "inputTokens": 1000,
                "outputTokens": 500,
                "cacheReadTokens": 0,
                "cacheCreationTokens": 0,
                "byAgent": { "claude": { "inputTokens": 1000, "outputTokens": 500, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
            },
            &yesterday_key: {
                "inputTokens": 2000,
                "outputTokens": 800,
                "cacheReadTokens": 0,
                "cacheCreationTokens": 0,
                "byAgent": { "codex": { "inputTokens": 2000, "outputTokens": 800, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
            },
            &today_key: {
                "inputTokens": 3000,
                "outputTokens": 1200,
                "cacheReadTokens": 0,
                "cacheCreationTokens": 0,
                "byAgent": { "claude": { "inputTokens": 3000, "outputTokens": 1200, "cacheReadTokens": 0, "cacheCreationTokens": 0 } }
            }
        }
    });
    std::fs::write(
        &history_path,
        serde_json::to_string_pretty(&seed).expect("serialize"),
    )
    .expect("write seed history");

    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    // Simulate app restart: baseline loaded from persisted file, sessions empty.
    let baseline = token_history::load_today_baseline();
    assert_eq!(baseline.input_tokens, 3000);
    assert_eq!(baseline.output_tokens, 1200);

    let state = test_app_state();
    *state.daily_tokens_baseline.lock().expect("lock") = baseline;
    *state.startup_daily_floor.lock().expect("lock") = baseline;

    // First snapshot sync with no active sessions (upgrade/restart edge case).
    token_history::sync_today_to_history(&state).expect("sync");

    let history = token_history::get_token_history(365).expect("history");
    let past_two = history
        .days
        .iter()
        .find(|day| day.date == two_days_ago_key)
        .expect("two days ago");
    let past_one = history
        .days
        .iter()
        .find(|day| day.date == yesterday_key)
        .expect("yesterday");
    assert_eq!(past_two.usage.input_tokens, 1000);
    assert_eq!(past_two.usage.output_tokens, 500);
    assert_eq!(past_one.usage.input_tokens, 2000);
    assert_eq!(past_one.usage.output_tokens, 800);

    // Today's file value must also be preserved (not overwritten with zeros).
    let today_record = history
        .days
        .iter()
        .find(|day| day.date == today_key)
        .expect("today");
    assert_eq!(today_record.usage.input_tokens, 3000);
    assert_eq!(today_record.usage.output_tokens, 1200);

    // UI floor: daily total must not drop below persisted baseline.
    let live_daily = effective_daily_tokens(&HashMap::new(), baseline, &HashSet::new());
    assert_eq!(live_daily.input_tokens, 3000);
    assert_eq!(live_daily.output_tokens, 1200);

    // Post-restart hook increments must add on top of the startup floor.
    let post_restart = HashMap::from([(
        "session-new".into(),
        TokenUsage {
            input_tokens: 500,
            output_tokens: 100,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        },
    )]);
    let combined = effective_daily_tokens(&post_restart, baseline, &HashSet::new());
    assert_eq!(combined.input_tokens, 3500);
    assert_eq!(combined.output_tokens, 1300);

    let _ = std::fs::remove_file(&history_path);
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn full_scan_does_not_regress_session_token_usage() {
    let _env_guard = TOKEN_HISTORY_ENV_LOCK
        .lock()
        .expect("token history env lock");
    let history_path = std::env::temp_dir().join(format!(
        "atoll-token-history-{}-{}.json",
        std::process::id(),
        "full-scan-regression"
    ));
    let _ = std::fs::remove_file(&history_path);
    std::env::set_var(
        "ATOLL_TOKEN_HISTORY_PATH",
        history_path.to_string_lossy().as_ref(),
    );

    let state = test_app_state();
    let session_id = "session-rescan";
    let dir = std::env::temp_dir().join(format!("atoll-token-rescan-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let transcript_path = dir.join("transcript.jsonl");
    // Empty transcript simulates rotation/truncation after hooks already counted usage.
    std::fs::write(&transcript_path, "").expect("write transcript");
    let transcript = transcript_path.to_string_lossy().into_owned();

    {
        let mut usage = state.session_token_usage.lock().expect("lock");
        usage.insert(
            session_id.into(),
            TokenUsage {
                input_tokens: 8000,
                output_tokens: 2000,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
        );
    }

    refresh_session_token_usage(
        &state,
        session_id,
        Some(transcript.as_str()),
        Some(&AgentKind::Claude),
    )
    .expect("refresh");

    let usage = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(session_id)
        .copied()
        .expect("usage");
    assert_eq!(usage.input_tokens, 8000);
    assert_eq!(usage.output_tokens, 2000);

    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(&history_path);
    std::env::remove_var("ATOLL_TOKEN_HISTORY_PATH");
}

#[test]
fn auto_archive_retention_purge_preserves_session_token_usage() {
    let state = test_app_state();
    let session_id = "session-auto-archived".to_string();
    let token_usage = TokenUsage {
        input_tokens: 500,
        output_tokens: 120,
        cache_read_tokens: 0,
        cache_creation_tokens: 0,
    };

    {
        let mut usage = state.session_token_usage.lock().expect("lock");
        usage.insert(session_id.clone(), token_usage);
    }
    {
        let mut known = state.known_sessions.lock().expect("lock");
        known.insert(
            session_id.clone(),
            KnownSession {
                agent: AgentKind::Claude,
                cwd: "/tmp/project".into(),
                transcript_path: Some("/tmp/project/transcript.jsonl".into()),
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            },
        );
    }
    {
        let mut sticky = state.session_agent_map.lock().expect("lock");
        sticky.insert(session_id.clone(), "claude".to_string());
    }

    // Simulate auto-archive timer purging a retention-expired known session.
    purge_tracked_session(&state, &session_id, Some("/tmp/project/transcript.jsonl"));

    let usage_after = state
        .session_token_usage
        .lock()
        .expect("lock")
        .get(&session_id)
        .copied()
        .expect("token usage should survive retention purge");
    assert_eq!(usage_after.input_tokens, 500);
    assert_eq!(usage_after.output_tokens, 120);

    let known_after = state.known_sessions.lock().expect("lock");
    assert!(!known_after.contains_key(&session_id));

    let token_usage_map = state.session_token_usage.lock().expect("lock");
    let snapshot = snapshot_from(
        &[],
        &HashMap::new(),
        900,
        &token_usage_map,
        &HashMap::new(),
        &HashSet::new(),
        true,
        &HashSet::new(),
    );
    assert_eq!(snapshot.daily_tokens.input_tokens, 500);
    assert_eq!(snapshot.daily_tokens.output_tokens, 120);
    assert_eq!(snapshot.active_session_tokens.input_tokens, 0);
}
