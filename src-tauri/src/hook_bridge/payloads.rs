use serde_json::Value;

use super::*;

pub(crate) fn permission_request_from_claude_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if !matches!(event_name, "PreToolUse" | "PermissionRequest") {
        return None;
    }

    permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Claude, true)
}

pub(crate) fn permission_request_from_codex_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if event_name != "PermissionRequest" {
        return None;
    }

    let transcript_path = payload_transcript_path(&payload);
    let cwd = resolve_codex_session_cwd(
        payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
        transcript_path.as_deref(),
    );
    if is_codex_internal_session(&AgentKind::Codex, &cwd, None) {
        return None;
    }

    let mut request =
        permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Codex, false)?;
    request.cwd = cwd;
    Some(request)
}

pub(crate) fn permission_request_from_zcode_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if !matches!(event_name, "PreToolUse" | "PermissionRequest") {
        return None;
    }

    permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Zcode, true)
}

/// Gemini CLI fires `BeforeTool` for every tool call; only payloads forwarded by
/// the Atoll hook script (side-effect tools, see `atoll-gemini-hook.mjs`) reach
/// this bridge as blocking approval requests.
pub(crate) fn permission_request_from_gemini_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload.get("hook_event_name")?.as_str()?;
    if event_name != "BeforeTool" {
        return None;
    }

    permission_request_from_tool_payload(id, payload, requested_at, AgentKind::Gemini, false)
}

pub(crate) fn permission_request_from_cursor_payload(
    id: String,
    payload: Value,
    requested_at: String,
) -> Option<PermissionRequest> {
    let event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("preToolUse");
    if event_name != "preToolUse" {
        return None;
    }

    let resolved_cwd = resolve_cursor_cwd(&payload);
    let mut request = permission_request_from_tool_payload(
        id,
        payload.clone(),
        requested_at,
        AgentKind::Cursor,
        false,
    )?;
    request.cwd = resolved_cwd;
    request.session = crate::payload_cursor_session_id(&payload)
        .unwrap_or("cursor")
        .to_string();
    Some(request)
}

pub(crate) fn permission_request_from_tool_payload(
    id: String,
    payload: Value,
    requested_at: String,
    agent: AgentKind,
    supports_always_from_suggestions: bool,
) -> Option<PermissionRequest> {
    let tool_name = payload.get("tool_name")?.as_str()?.to_string();
    let tool_input = payload.get("tool_input").cloned().unwrap_or(Value::Null);
    if serde_json::to_vec(&tool_input).ok()?.len() > MAX_PERMISSION_TOOL_INPUT_BYTES {
        return None;
    }
    let truncate_label = |label: String| {
        if label.chars().count() <= MAX_PERMISSION_LABEL_CHARS {
            label
        } else {
            let mut value: String = label.chars().take(MAX_PERMISSION_LABEL_CHARS).collect();
            value.push_str("… [truncated]");
            value
        }
    };
    let command = truncate_label(command_label(&tool_name, &tool_input));
    let detail = truncate_label(detail_label(&tool_name, &tool_input));
    let default_session = match agent {
        AgentKind::Codex => "codex",
        AgentKind::Cursor => "cursor",
        AgentKind::Zcode => "zcode",
        AgentKind::Gemini => "gemini",
        _ => "claude-code",
    };

    let supports_always = if supports_always_from_suggestions {
        payload
            .get("permission_suggestions")
            .and_then(Value::as_array)
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    Some(PermissionRequest {
        id,
        tool_use_id: payload
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        agent,
        session: payload_session_id(&payload)
            .unwrap_or(default_session)
            .to_string(),
        command,
        detail,
        cwd: payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
        requested_at,
        status: PermissionStatus::Pending,
        archived: false,
        supports_always,
        transcript_path: payload_transcript_path(&payload),
        tool_input: payload.get("tool_input").and_then(|value| {
            if value.is_null() {
                None
            } else {
                Some(value.clone())
            }
        }),
    })
}

pub(crate) fn payload_session_id(payload: &Value) -> Option<&str> {
    payload
        .get("session_id")
        .and_then(Value::as_str)
        .or_else(|| payload.get("sessionId").and_then(Value::as_str))
        .or_else(|| payload.get("conversation_id").and_then(Value::as_str))
        .or_else(|| payload.get("conversationId").and_then(Value::as_str))
}

#[cfg(test)]
mod payload_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exec_command_uses_bash_label() {
        let input = json!({"command": "printf hello"});
        assert_eq!(command_label("exec_command", &input), "Bash: printf hello");
    }

    #[test]
    fn bash_command_label_unchanged() {
        let input = json!({"command": "ls -la"});
        assert_eq!(command_label("Bash", &input), "Bash: ls -la");
    }

    #[test]
    fn shell_command_label_matches_bash() {
        let input = json!({"command": "npm test"});
        assert_eq!(command_label("Shell", &input), "Bash: npm test");
    }

    #[test]
    fn cursor_permission_response_allow() {
        let response = cursor_permission_hook_response(Decision::Approved, "", None);
        assert_eq!(
            response.get("permission").and_then(Value::as_str),
            Some("allow")
        );
    }

    #[test]
    fn cursor_permission_response_deny() {
        let response = cursor_permission_hook_response(Decision::Denied, "blocked", None);
        assert_eq!(
            response.get("permission").and_then(Value::as_str),
            Some("deny")
        );
    }

    #[test]
    fn cursor_payload_builds_permission_request() {
        let payload = json!({
            "hook_event_name": "preToolUse",
            "conversation_id": "conv-123",
            "cwd": "/tmp/project",
            "tool_name": "Shell",
            "tool_input": { "command": "echo hi" },
            "tool_use_id": "tool-1"
        });
        let request = permission_request_from_cursor_payload(
            "req-1".into(),
            payload,
            "2026-01-01T00:00:00Z".into(),
        )
        .expect("cursor request");
        assert_eq!(request.session, "conv-123");
        assert!(matches!(request.agent, AgentKind::Cursor));
    }

    #[test]
    fn gemini_before_tool_payload_builds_permission_request() {
        let payload = json!({
            "session_id": "session-gemini-1",
            "transcript_path": "/tmp/gemini/transcript.json",
            "cwd": "/tmp/project",
            "hook_event_name": "BeforeTool",
            "timestamp": "2026-08-30T00:00:00Z",
            "tool_name": "run_shell_command",
            "tool_input": { "command": "echo hi", "description": "Echo hi" }
        });
        let request = permission_request_from_gemini_payload(
            "req-1".into(),
            payload,
            "2026-08-30T00:00:00Z".into(),
        )
        .expect("gemini request");
        assert!(matches!(request.agent, AgentKind::Gemini));
        assert_eq!(request.session, "session-gemini-1");
        assert_eq!(request.command, "Bash: echo hi");
        assert_eq!(request.detail, "Echo hi");
        assert_eq!(request.cwd, "/tmp/project");
        assert!(!request.supports_always);
    }

    #[test]
    fn gemini_non_before_tool_payload_is_ignored() {
        let payload = json!({
            "session_id": "session-gemini-1",
            "cwd": "/tmp/project",
            "hook_event_name": "SessionStart",
            "source": "startup"
        });
        assert!(permission_request_from_gemini_payload(
            "req-1".into(),
            payload,
            "2026-08-30T00:00:00Z".into(),
        )
        .is_none());
    }

    #[test]
    fn gemini_payload_without_session_id_falls_back_to_default_session() {
        let payload = json!({
            "hook_event_name": "BeforeTool",
            "cwd": "/tmp/project",
            "tool_name": "write_file",
            "tool_input": { "file_path": "/tmp/a.txt", "content": "hi" }
        });
        let request = permission_request_from_gemini_payload(
            "req-1".into(),
            payload,
            "2026-08-30T00:00:00Z".into(),
        )
        .expect("gemini request");
        assert_eq!(request.session, "gemini");
    }

    #[test]
    fn gemini_permission_response_allow() {
        let response = gemini_permission_hook_response(Decision::Approved, "", None);
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("allow")
        );
        assert!(response.get("reason").is_none());
    }

    #[test]
    fn gemini_permission_response_allow_with_updated_input() {
        let response = gemini_permission_hook_response(
            Decision::Approved,
            "",
            Some(json!({ "command": "echo safe" })),
        );
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("allow")
        );
        let specific = response
            .get("hookSpecificOutput")
            .expect("hookSpecificOutput");
        assert_eq!(
            specific.get("hookEventName").and_then(Value::as_str),
            Some("BeforeTool")
        );
        assert_eq!(
            specific
                .get("tool_input")
                .and_then(|input| input.get("command"))
                .and_then(Value::as_str),
            Some("echo safe")
        );
    }

    #[test]
    fn gemini_permission_response_deny_with_note() {
        let response = gemini_permission_hook_response(Decision::Denied, "not today", None);
        assert_eq!(
            response.get("decision").and_then(Value::as_str),
            Some("deny")
        );
        assert_eq!(
            response.get("reason").and_then(Value::as_str),
            Some("Denied from Atoll: not today")
        );
    }

    #[test]
    fn gemini_defer_response_is_empty_allow() {
        let response = build_hook_defer_response(
            PermissionResponseStyle::Gemini,
            "BeforeTool",
            "Atoll approval timed out",
        );
        assert_eq!(response, json!({}));
    }
}
