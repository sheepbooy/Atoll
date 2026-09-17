use super::*;
use crate::core_tests::test_app_state;

#[test]
fn archived_requests_still_appear_in_session_list_until_retention_expires() {
    let requested_at = iso_timestamp_now();
    let requests = vec![PermissionRequest {
        id: "req-1".into(),
        tool_use_id: None,
        agent: AgentKind::Claude,
        session: "session-a".into(),
        command: "Bash: ls".into(),
        detail: "List files".into(),
        cwd: "/tmp/project".into(),
        requested_at,
        status: PermissionStatus::Approved,
        archived: true,
        supports_always: false,
        transcript_path: None,
        tool_input: None,
    }];

    let snapshot = snapshot_from(
        &requests,
        &HashMap::new(),
        900,
        &HashMap::new(),
        &HashMap::new(),
        &HashSet::new(),
        true,
        &HashSet::new(),
    );

    assert_eq!(snapshot.sessions.len(), 1);
}

#[test]
fn removed_session_requests_do_not_reappear_in_session_list() {
    let snapshot = snapshot_from(
        &[],
        &HashMap::new(),
        900,
        &HashMap::new(),
        &HashMap::new(),
        &HashSet::new(),
        true,
        &HashSet::new(),
    );

    assert!(snapshot.sessions.is_empty());
}

#[test]
fn codex_memories_background_session_is_ignored() {
    let memories_cwd = dirs::home_dir()
        .expect("home dir")
        .join(".codex")
        .join("memories")
        .to_string_lossy()
        .into_owned();
    let known_sessions = HashMap::from([
        (
            "memories-thread".into(),
            KnownSession {
                agent: AgentKind::Codex,
                cwd: memories_cwd.clone(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            },
        ),
        (
            "real-session".into(),
            KnownSession {
                agent: AgentKind::Codex,
                cwd: "/Users/test/project".into(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::Unknown,
                conversation_id: None,
            },
        ),
    ]);

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

    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].session_id, "real-session");
    assert!(is_codex_internal_session(
        &AgentKind::Codex,
        &memories_cwd,
        None,
    ));
    assert!(is_codex_internal_session(&AgentKind::Codex, ".", None));
    assert!(is_codex_internal_session(&AgentKind::Codex, "", None));
    assert!(!is_codex_internal_session(
        &AgentKind::Codex,
        "/Users/test/project",
        None,
    ));
    assert!(!is_codex_internal_session(
        &AgentKind::Codex,
        "/Users/test/code/Atoll/.codex",
        None,
    ));
    assert!(!is_codex_internal_session(
        &AgentKind::Codex,
        "/Users/test/.codex/sessions/2026/06/23/rollout.jsonl",
        None,
    ));
}

#[test]
fn session_host_for_summary_trusts_stored_host() {
    let known_sessions = HashMap::from([
        (
            "cli-session".into(),
            KnownSession {
                agent: AgentKind::Claude,
                cwd: "/tmp/project".into(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::ClaudeCli,
                conversation_id: None,
            },
        ),
        (
            "desktop-session".into(),
            KnownSession {
                agent: AgentKind::Claude,
                cwd: "/tmp/desktop".into(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::ClaudeDesktop,
                conversation_id: None,
            },
        ),
    ]);

    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "cli-session",
            "/tmp/project",
            &AgentKind::Claude,
        ),
        platform::SessionHost::ClaudeCli,
    );
    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "desktop-session",
            "/tmp/desktop",
            &AgentKind::Claude,
        ),
        platform::SessionHost::ClaudeDesktop,
    );
}

#[test]
fn session_host_from_transcript_path_when_stored_unknown() {
    let known_sessions = HashMap::from([
            (
                "cli-unknown".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some("/Users/test/.claude/projects/-tmp-project/abc.jsonl".into()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
            (
                "desktop-unknown".into(),
                KnownSession {
                    agent: AgentKind::Claude,
                    cwd: "/tmp/project".into(),
                    transcript_path: Some("/Users/test/Library/Application Support/Claude-3p/local-agent-mode-sessions/xyz.jsonl".into()),
                    last_activity: iso_timestamp_now(),
                    host: platform::SessionHost::Unknown,
                    conversation_id: None,
                },
            ),
        ]);

    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "cli-unknown",
            "/tmp/project",
            &AgentKind::Claude,
        ),
        platform::SessionHost::ClaudeCli,
    );
    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "desktop-unknown",
            "/tmp/project",
            &AgentKind::Claude,
        ),
        platform::SessionHost::ClaudeDesktop,
    );
}

#[test]
fn host_from_claude_transcript_path_patterns() {
    assert_eq!(
        host_from_claude_transcript_path("/Users/me/.claude/projects/-tmp-project/abc.jsonl"),
        Some(platform::SessionHost::ClaudeCli),
    );
    assert_eq!(
        host_from_claude_transcript_path(
            "/Users/me/Library/Application Support/Claude-3p/local-agent-mode-sessions/xyz.jsonl"
        ),
        Some(platform::SessionHost::ClaudeDesktop),
    );
    assert_eq!(
            host_from_claude_transcript_path("/Users/me/Library/Application Support/com.anthropic.claudefordesktop/agent-sessions/xyz.jsonl"),
            Some(platform::SessionHost::ClaudeDesktop),
        );
    assert_eq!(
        host_from_claude_transcript_path(
            "/Users/me/Library/Application Support/Claude/projects/xyz.jsonl"
        ),
        Some(platform::SessionHost::ClaudeDesktop),
    );
    assert_eq!(
        host_from_claude_transcript_path("/some/random/path/transcript.jsonl"),
        None,
    );
}

#[test]
fn host_from_codex_transcript_path_patterns() {
    assert_eq!(
        host_from_codex_transcript_path("/Users/me/.codex/sessions/2026/06/23/rollout.jsonl"),
        Some(platform::SessionHost::CodexCli),
    );
    assert_eq!(
        host_from_codex_transcript_path(
            "/Users/me/Library/Application Support/com.openai.codex/sessions/abc.jsonl"
        ),
        Some(platform::SessionHost::CodexDesktop),
    );
    assert_eq!(
        host_from_codex_transcript_path("/some/random/path/transcript.jsonl"),
        None,
    );
}

#[test]
fn session_host_for_summary_codex_trusts_stored_host() {
    let known_sessions = HashMap::from([
        (
            "codex-cli-session".into(),
            KnownSession {
                agent: AgentKind::Codex,
                cwd: "/tmp/codex-cli".into(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::CodexCli,
                conversation_id: None,
            },
        ),
        (
            "codex-desktop-session".into(),
            KnownSession {
                agent: AgentKind::Codex,
                cwd: "/tmp/codex-desktop".into(),
                transcript_path: None,
                last_activity: iso_timestamp_now(),
                host: platform::SessionHost::CodexDesktop,
                conversation_id: None,
            },
        ),
    ]);

    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "codex-cli-session",
            "/tmp/codex-cli",
            &AgentKind::Codex,
        ),
        platform::SessionHost::CodexCli,
    );
    assert_eq!(
        session_host_for_summary(
            &known_sessions,
            "codex-desktop-session",
            "/tmp/codex-desktop",
            &AgentKind::Codex,
        ),
        platform::SessionHost::CodexDesktop,
    );
}

#[test]
fn codex_missing_cwd_is_resolved_from_transcript() {
    let dir = std::env::temp_dir().join(format!("atoll-codex-session-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let transcript_path = dir.join("rollout-test.jsonl");
    std::fs::write(
        &transcript_path,
        r#"{"type":"session_meta","payload":{"id":"session-app","cwd":"C:/Users/test/project"}}"#,
    )
    .expect("write transcript");
    let transcript = transcript_path.to_string_lossy().into_owned();

    assert!(!is_codex_internal_session(
        &AgentKind::Codex,
        ".",
        Some(&transcript),
    ));
    assert_eq!(
        resolve_codex_session_cwd(".", Some(&transcript)),
        "C:/Users/test/project"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn codex_session_summary_exposes_known_or_request_transcript_path() {
    let requested_at = iso_timestamp_now();
    let request_path = "/tmp/atoll-codex-request.jsonl".to_string();
    let known_path = "/tmp/atoll-codex-known.jsonl".to_string();
    let requests = vec![PermissionRequest {
        id: "req-codex".into(),
        tool_use_id: None,
        agent: AgentKind::Codex,
        session: "codex-request-session".into(),
        command: "Bash: ls".into(),
        detail: "List files".into(),
        cwd: "/tmp/request-project".into(),
        requested_at: requested_at.clone(),
        status: PermissionStatus::Approved,
        archived: false,
        supports_always: false,
        transcript_path: Some(request_path.clone()),
        tool_input: None,
    }];
    let known_sessions = HashMap::from([(
        "codex-known-session".into(),
        KnownSession {
            agent: AgentKind::Codex,
            cwd: "/tmp/known-project".into(),
            transcript_path: Some(known_path.clone()),
            last_activity: requested_at,
            host: platform::SessionHost::Unknown,
            conversation_id: None,
        },
    )]);

    let snapshot = snapshot_from(
        &requests,
        &HashMap::new(),
        900,
        &HashMap::new(),
        &known_sessions,
        &HashSet::new(),
        true,
        &HashSet::new(),
    );

    let request_session = snapshot
        .sessions
        .iter()
        .find(|session| session.session_id == "codex-request-session")
        .expect("request session");
    assert_eq!(
        request_session.transcript_path.as_deref(),
        Some(request_path.as_str())
    );

    let known_session = snapshot
        .sessions
        .iter()
        .find(|session| session.session_id == "codex-known-session")
        .expect("known session");
    assert_eq!(
        known_session.transcript_path.as_deref(),
        Some(known_path.as_str())
    );
}

#[test]
fn cursor_composer_modes_all_register_in_snapshot() {
    for (mode, session_id, cwd) in [
        ("ask", "conv-mode-ask", "/tmp/ask"),
        ("agent", "conv-mode-agent", "/tmp/agent"),
        ("edit", "conv-mode-edit", "/tmp/edit"),
        ("debug", "conv-mode-debug", "/tmp/debug"),
    ] {
        let state = test_app_state();
        register_known_session(&state, session_id, AgentKind::Cursor, cwd, None);
        touch_session_activity(&state, session_id);

        let known = state.known_sessions.lock().expect("lock");
        let last_seen = state.session_last_seen.lock().expect("lock");
        let token_usage = state.session_token_usage.lock().expect("lock");
        let pinned = state.pinned_sessions.lock().expect("lock");
        let snapshot = snapshot_from(
            &[],
            &last_seen,
            DEFAULT_SESSION_RETENTION_SECS,
            &token_usage,
            &known,
            &pinned,
            true,
            &HashSet::new(),
        );

        assert_eq!(
            snapshot.sessions.len(),
            1,
            "composer_mode={mode} should produce one session"
        );
        assert_eq!(snapshot.sessions[0].session_id, session_id);
        assert_eq!(snapshot.sessions[0].cwd, cwd);
    }
}

#[test]
fn cursor_before_submit_prompt_refreshes_session_activity() {
    let state = test_app_state();
    let session_id = "conv-submit-prompt";
    register_known_session(&state, session_id, AgentKind::Cursor, "/tmp/project", None);
    touch_session_activity(&state, session_id);

    let activity_after = {
        let known = state.known_sessions.lock().expect("lock");
        known
            .get(session_id)
            .map(|entry| entry.last_activity.clone())
            .expect("session")
    };
    assert!(!activity_after.is_empty());
}

#[test]
fn cursor_ask_session_start_appears_in_snapshot() {
    let state = test_app_state();
    register_known_session(
        &state,
        "conv-ask-1",
        AgentKind::Cursor,
        "/tmp/ask-project",
        None,
    );
    touch_session_activity(&state, "conv-ask-1");

    let known = state.known_sessions.lock().expect("lock");
    let last_seen = state.session_last_seen.lock().expect("lock");
    let token_usage = state.session_token_usage.lock().expect("lock");
    let pinned = state.pinned_sessions.lock().expect("lock");
    let snapshot = snapshot_from(
        &[],
        &last_seen,
        DEFAULT_SESSION_RETENTION_SECS,
        &token_usage,
        &known,
        &pinned,
        true,
        &HashSet::new(),
    );

    assert_eq!(snapshot.sessions.len(), 1);
    assert_eq!(snapshot.sessions[0].session_id, "conv-ask-1");
    assert!(matches!(snapshot.sessions[0].agent, AgentKind::Cursor));
    assert_eq!(snapshot.sessions[0].cwd, "/tmp/ask-project");
}

#[test]
fn decode_cursor_project_slug_recovers_workspace_path() {
    let home = dirs::home_dir().expect("home");
    let home_str = home.to_string_lossy();
    #[cfg(not(windows))]
    {
        let suffix = home_str
            .strip_prefix("/Users/")
            .unwrap_or(home_str.as_ref());
        let slug = format!("Users-{}", suffix.replace('/', "-"));
        let decoded = decode_cursor_project_slug(&slug).expect("decoded");
        assert_eq!(decoded, *home_str);
    }
    #[cfg(windows)]
    {
        let drive = home_str.chars().next().unwrap_or('C');
        let rest = &home_str[3..]; // skip "C:\"
        let slug = format!("{}-{}", drive, rest.replace('\\', "-"));
        let decoded = decode_cursor_project_slug(&slug).expect("decoded");
        assert_eq!(decoded, *home_str);
    }
}

#[test]
fn discover_cursor_agent_transcript_finds_workspace_and_path() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let projects = home.join(".cursor").join("projects");
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let transcripts = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() {
                continue;
            }
            let (path, workspace) = discover_cursor_agent_transcript(&conv_id).expect("discovered");
            assert_eq!(path, jsonl.to_string_lossy());
            if let Some(expected) = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
            {
                assert_eq!(workspace, expected);
            }
            return;
        }
    }
}

#[test]
fn discover_cursor_agent_transcript_matches_short_session_id_prefix() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let projects = home.join(".cursor").join("projects");
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let transcripts = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() || conv_id.len() <= CURSOR_TRANSCRIPT_PREFIX_MIN_LEN {
                continue;
            }
            let short_prefix = &conv_id[..CURSOR_TRANSCRIPT_PREFIX_MIN_LEN];
            let (path, _workspace) =
                discover_cursor_agent_transcript(short_prefix).expect("prefix discover");
            assert_eq!(path, jsonl.to_string_lossy());
            return;
        }
    }
}

#[test]
fn ghost_cursor_sessions_with_dot_cwd_are_hidden_from_snapshot() {
    let state = test_app_state();
    register_known_session(&state, "ghost-conv", AgentKind::Cursor, ".", None);
    touch_session_activity(&state, "ghost-conv");

    let known = state.known_sessions.lock().expect("lock");
    let last_seen = state.session_last_seen.lock().expect("lock");
    let token_usage = state.session_token_usage.lock().expect("lock");
    let pinned = state.pinned_sessions.lock().expect("lock");
    let snapshot = snapshot_from(
        &[],
        &last_seen,
        DEFAULT_SESSION_RETENTION_SECS,
        &token_usage,
        &known,
        &pinned,
        true,
        &HashSet::new(),
    );

    assert!(snapshot.sessions.is_empty());
}

#[test]
fn backfill_cursor_session_metadata_links_on_disk_transcript() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let projects = home.join(".cursor").join("projects");
    let Ok(entries) = std::fs::read_dir(&projects) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let transcripts = entry.path().join("agent-transcripts");
        let Ok(conv_entries) = std::fs::read_dir(&transcripts) else {
            continue;
        };
        for conv in conv_entries.flatten() {
            if !conv.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let conv_id = conv.file_name().to_string_lossy().into_owned();
            let jsonl = conv.path().join(format!("{conv_id}.jsonl"));
            if !jsonl.is_file() {
                continue;
            }
            let workspace = decode_cursor_project_slug(&entry.file_name().to_string_lossy())
                .unwrap_or_else(|| "/tmp/unknown".to_string());

            let state = test_app_state();
            register_known_session(&state, &conv_id, AgentKind::Cursor, &workspace, None);
            backfill_cursor_session_metadata(&state);

            let known = state.known_sessions.lock().expect("lock");
            let session = known.get(&conv_id).expect("session");
            assert_eq!(
                session.transcript_path.as_deref(),
                Some(jsonl.to_string_lossy().as_ref())
            );
            return;
        }
    }
}
