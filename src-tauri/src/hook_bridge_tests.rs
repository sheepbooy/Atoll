use serde_json::json;

#[test]
fn maps_claude_pre_tool_use_payload_to_permission_request() {
    let payload = json!({
        "session_id": "session-123",
        "cwd": "/tmp/project",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "npm install",
            "description": "Install dependencies"
        },
        "tool_use_id": "tool-123"
    });

    let request = crate::hook_bridge::permission_request_from_claude_payload(
        "request-123".into(),
        payload,
        "2026-06-09T09:00:00Z".into(),
    )
    .expect("payload should map to a request");

    assert_eq!(request.id, "request-123");
    assert!(matches!(request.agent, crate::AgentKind::Claude));
    assert_eq!(request.session, "session-123");
    assert_eq!(request.command, "Bash: npm install");
    assert_eq!(request.detail, "Install dependencies");
    assert_eq!(request.cwd, "/tmp/project");
    assert_eq!(request.tool_use_id.as_deref(), Some("tool-123"));
    assert_eq!(request.status, crate::PermissionStatus::Pending);
}

#[test]
fn maps_claude_permission_request_payload_to_permission_request() {
    let payload = json!({
        "session_id": "session-123",
        "cwd": "/tmp/project",
        "hook_event_name": "PermissionRequest",
        "tool_name": "Bash",
        "tool_input": {
            "command": "npm install",
            "description": "Install dependencies"
        },
        "tool_use_id": "tool-123"
    });

    let request = crate::hook_bridge::permission_request_from_claude_payload(
        "request-123".into(),
        payload,
        "2026-06-09T09:00:00Z".into(),
    )
    .expect("payload should map to a request");

    assert_eq!(request.command, "Bash: npm install");
    assert_eq!(request.detail, "Install dependencies");
    assert_eq!(request.tool_use_id.as_deref(), Some("tool-123"));
    assert!(!request.supports_always);
}

#[test]
fn supports_always_from_permission_suggestions() {
    let payload = json!({
        "session_id": "session-123",
        "cwd": "/tmp/project",
        "hook_event_name": "PermissionRequest",
        "tool_name": "Bash",
        "tool_input": {
            "command": "npm install",
            "description": "Install dependencies"
        },
        "permission_suggestions": [
            {
                "type": "addRules",
                "rules": [{"toolName": "Bash", "ruleContent": "npm install"}],
                "behavior": "allow",
                "destination": "localSettings"
            }
        ]
    });

    let request = crate::hook_bridge::permission_request_from_claude_payload(
        "request-456".into(),
        payload,
        "2026-06-09T09:00:00Z".into(),
    )
    .expect("payload should map to a request");

    assert!(request.supports_always);
}

#[test]
fn marks_pending_request_complete_from_claude_post_tool_use() {
    let mut requests = vec![crate::PermissionRequest {
        id: "request-123".into(),
        tool_use_id: Some("tool-123".into()),
        agent: crate::AgentKind::Claude,
        session: "session-123".into(),
        command: "Bash: npm install".into(),
        detail: "Install dependencies".into(),
        cwd: "/tmp/project".into(),
        requested_at: "2026-06-09T09:00:00Z".into(),
        status: crate::PermissionStatus::Pending,
        archived: false,
        supports_always: false,
        transcript_path: None,
        tool_input: None,
    }];

    let payload = json!({
        "session_id": "session-123",
        "hook_event_name": "PostToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "npm install"
        },
        "tool_use_id": "tool-123"
    });

    let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
        &mut requests,
        &payload,
        "Completed in Claude.",
    );

    assert_eq!(completed_id.as_deref(), Some("request-123"));
    assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
    assert!(requests[0].detail.contains("Completed in Claude."));
}

#[test]
fn marks_only_pending_request_complete_when_post_tool_use_has_no_match_fields() {
    let mut requests = vec![crate::PermissionRequest {
        id: "request-123".into(),
        tool_use_id: None,
        agent: crate::AgentKind::Claude,
        session: "session-123".into(),
        command: "Bash: curl -s https://httpbin.org/ip".into(),
        detail: "Curl to external API to trigger permission request".into(),
        cwd: "/tmp/project".into(),
        requested_at: "2026-06-09T09:00:00Z".into(),
        status: crate::PermissionStatus::Pending,
        archived: false,
        supports_always: false,
        transcript_path: None,
        tool_input: None,
    }];

    let payload = json!({
        "session_id": "session-123",
        "hook_event_name": "PostToolUse"
    });

    let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
        &mut requests,
        &payload,
        "Completed in Claude.",
    );

    assert_eq!(completed_id.as_deref(), Some("request-123"));
    assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
}

#[test]
fn falls_back_to_newest_session_pending_when_post_tool_use_has_no_match_fields() {
    // Requests are stored newest-first (see `requests.insert(0, …)`), so a
    // PostToolUse that carries no match fields falls back to completing the
    // session's newest pending request and leaves the older ones pending.
    let mut requests = vec![
        crate::PermissionRequest {
            id: "request-newer".into(),
            tool_use_id: None,
            agent: crate::AgentKind::Claude,
            session: "session-123".into(),
            command: "Bash: echo two".into(),
            detail: "two".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-09T09:00:01Z".into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        },
        crate::PermissionRequest {
            id: "request-older".into(),
            tool_use_id: None,
            agent: crate::AgentKind::Claude,
            session: "session-123".into(),
            command: "Bash: echo one".into(),
            detail: "one".into(),
            cwd: "/tmp/project".into(),
            requested_at: "2026-06-09T09:00:00Z".into(),
            status: crate::PermissionStatus::Pending,
            archived: false,
            supports_always: false,
            transcript_path: None,
            tool_input: None,
        },
    ];

    let payload = json!({
        "session_id": "session-123",
        "hook_event_name": "PostToolUse"
    });

    let completed_id = crate::hook_bridge::mark_matching_pending_request_complete(
        &mut requests,
        &payload,
        "Completed in Claude.",
    );

    assert_eq!(completed_id.as_deref(), Some("request-newer"));
    assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
    assert_eq!(requests[1].status, crate::PermissionStatus::Pending);
}

#[test]
fn encodes_hook_decision_for_claude_hook_event() {
    let approved = crate::hook_bridge::permission_hook_response(
        "PermissionRequest",
        crate::Decision::Approved,
        "",
        None,
    );
    assert_eq!(
        approved,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "allow"
                }
            }
        })
    );

    let denied = crate::hook_bridge::permission_hook_response(
        "PermissionRequest",
        crate::Decision::Denied,
        "",
        None,
    );
    assert_eq!(
        denied,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "deny",
                    "message": "Denied from Atoll"
                }
            }
        })
    );
}

#[test]
fn encodes_hook_decision_with_note_for_claude_hook_event() {
    let denied = crate::hook_bridge::permission_hook_response(
        "PermissionRequest",
        crate::Decision::Denied,
        "Please use a safer command",
        None,
    );
    assert_eq!(
        denied,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "deny",
                    "message": "Denied from Atoll: Please use a safer command"
                }
            }
        })
    );
}

#[test]
fn encodes_hook_decision_for_claude_pre_tool_use() {
    let approved = crate::hook_bridge::permission_hook_response(
        "PreToolUse",
        crate::Decision::Approved,
        "",
        None,
    );
    assert_eq!(
        approved,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "allow",
                "permissionDecisionReason": "Approved from Atoll"
            }
        })
    );
}

#[test]
fn maps_codex_permission_request_payload_to_permission_request() {
    let payload = json!({
        "session_id": "codex-session-1",
        "cwd": "/Users/test/project",
        "hook_event_name": "PermissionRequest",
        "tool_name": "exec_command",
        "tool_input": {
            "command": "npm test",
            "description": "Run tests"
        },
        "tool_use_id": "tool-codex-1"
    });

    let request = crate::hook_bridge::permission_request_from_codex_payload(
        "request-codex-1".into(),
        payload,
        "2026-06-19T09:00:00Z".into(),
    )
    .expect("payload should map to a request");

    assert_eq!(request.id, "request-codex-1");
    assert!(matches!(request.agent, crate::AgentKind::Codex));
    assert_eq!(request.session, "codex-session-1");
    assert_eq!(request.command, "Bash: npm test");
    assert_eq!(request.detail, "Run tests");
    assert_eq!(request.cwd, "/Users/test/project");
    assert_eq!(request.tool_use_id.as_deref(), Some("tool-codex-1"));
    assert!(!request.supports_always);
}

#[test]
fn encodes_codex_permission_allow_and_deny_responses() {
    let approved = crate::hook_bridge::permission_hook_response(
        "PermissionRequest",
        crate::Decision::Approved,
        "",
        None,
    );
    assert_eq!(
        approved,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "allow"
                }
            }
        })
    );

    let denied = crate::hook_bridge::permission_hook_response(
        "PermissionRequest",
        crate::Decision::Denied,
        "too risky",
        None,
    );
    assert_eq!(
        denied,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": {
                    "behavior": "deny",
                    "message": "Denied from Atoll: too risky"
                }
            }
        })
    );
}

#[test]
fn codex_internal_permission_request_is_ignored() {
    let payload = json!({
        "session_id": "internal-thread",
        "hook_event_name": "PermissionRequest",
        "tool_name": "exec_command",
        "tool_input": {
            "command": "echo hi",
            "description": "internal"
        }
    });

    let request = crate::hook_bridge::permission_request_from_codex_payload(
        "request-internal".into(),
        payload,
        "2026-06-19T09:00:00Z".into(),
    );

    assert!(request.is_none());
}

#[test]
fn encodes_permission_request_ask_as_empty_response() {
    let ask = crate::hook_bridge::hook_defer_response("PermissionRequest", "Atoll unavailable");

    assert_eq!(ask, json!({}));
}

#[test]
fn encodes_pre_tool_use_ask_response() {
    let ask = crate::hook_bridge::hook_defer_response("PreToolUse", "Atoll unavailable");

    assert_eq!(
        ask,
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "ask",
                "permissionDecisionReason": "Atoll unavailable"
            }
        })
    );
}
