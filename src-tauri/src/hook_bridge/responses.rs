use serde_json::Value;

use super::*;

pub(crate) fn permission_hook_response(
    hook_event_name: &str,
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    if hook_event_name == "PermissionRequest" {
        let decision = match decision {
            Decision::Approved => {
                if let Some(input) = updated_input {
                    json!({ "behavior": "allow", "updatedInput": input })
                } else {
                    json!({ "behavior": "allow" })
                }
            }
            Decision::Denied => {
                let message = if note.is_empty() {
                    "Denied from Atoll".to_string()
                } else {
                    format!("Denied from Atoll: {note}")
                };
                json!({
                    "behavior": "deny",
                    "message": message
                })
            }
        };

        return json!({
            "hookSpecificOutput": {
                "hookEventName": hook_event_name,
                "decision": decision
            }
        });
    }

    let (permission_decision, reason) = match decision {
        Decision::Approved => ("allow", "Approved from Atoll".to_string()),
        Decision::Denied => {
            let reason = if note.is_empty() {
                "Denied from Atoll".to_string()
            } else {
                format!("Denied from Atoll: {note}")
            };
            ("deny", reason)
        }
    };
    let mut output = json!({
        "hookEventName": hook_event_name,
        "permissionDecision": permission_decision,
        "permissionDecisionReason": reason
    });
    if matches!(decision, Decision::Approved) {
        if let Some(input) = updated_input {
            output
                .as_object_mut()
                .unwrap()
                .insert("updatedInput".to_string(), input);
        }
    }
    json!({ "hookSpecificOutput": output })
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PermissionResponseStyle {
    ClaudeCodex,
    #[allow(dead_code)]
    Cursor,
    Gemini,
}

pub(crate) fn cursor_permission_hook_response(
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    match decision {
        Decision::Approved => {
            if let Some(input) = updated_input {
                json!({ "permission": "allow", "updated_input": input })
            } else {
                json!({ "permission": "allow" })
            }
        }
        Decision::Denied => {
            let message = if note.is_empty() {
                "Denied from Atoll".to_string()
            } else {
                format!("Denied from Atoll: {note}")
            };
            json!({
                "permission": "deny",
                "user_message": message,
                "agent_message": message
            })
        }
    }
}

pub(crate) fn cursor_hook_defer_response(hook_event_name: &str, reason: &str) -> Value {
    if matches!(
        hook_event_name,
        "postToolUse" | "postToolUseFailure" | "stop" | "subagentStart" | "subagentStop"
    ) {
        return json!({});
    }

    json!({
        "permission": "deny",
        "user_message": reason,
        "agent_message": reason
    })
}

/// Gemini CLI hook output schema (docs/hooks/reference): a top-level
/// `decision` of `deny`/`block` prevents tool execution and feeds `reason`
/// back to the model; `allow` continues into Gemini's own policy flow.
/// `hookSpecificOutput.tool_input` optionally rewrites the tool arguments.
pub(crate) fn gemini_permission_hook_response(
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    match decision {
        Decision::Approved => {
            let mut response = json!({ "decision": "allow" });
            if let Some(input) = updated_input {
                response.as_object_mut().unwrap().insert(
                    "hookSpecificOutput".to_string(),
                    json!({
                        "hookEventName": "BeforeTool",
                        "tool_input": input
                    }),
                );
            }
            response
        }
        Decision::Denied => {
            let reason = if note.is_empty() {
                "Denied from Atoll".to_string()
            } else {
                format!("Denied from Atoll: {note}")
            };
            json!({
                "decision": "deny",
                "reason": reason
            })
        }
    }
}

/// Atoll failures must never brick a Gemini session: Gemini defaults to
/// "Allow" when a hook prints nothing, so the defer response is empty output.
pub(crate) fn gemini_hook_defer_response() -> Value {
    json!({})
}

pub(crate) fn build_permission_response(
    style: PermissionResponseStyle,
    hook_event_name: &str,
    decision: Decision,
    note: &str,
    updated_input: Option<Value>,
) -> Value {
    match style {
        PermissionResponseStyle::ClaudeCodex => {
            permission_hook_response(hook_event_name, decision, note, updated_input)
        }
        PermissionResponseStyle::Cursor => {
            cursor_permission_hook_response(decision, note, updated_input)
        }
        PermissionResponseStyle::Gemini => {
            gemini_permission_hook_response(decision, note, updated_input)
        }
    }
}

pub(crate) fn build_hook_defer_response(
    style: PermissionResponseStyle,
    hook_event_name: &str,
    reason: &str,
) -> Value {
    match style {
        PermissionResponseStyle::ClaudeCodex => hook_defer_response(hook_event_name, reason),
        PermissionResponseStyle::Cursor => cursor_hook_defer_response(hook_event_name, reason),
        PermissionResponseStyle::Gemini => gemini_hook_defer_response(),
    }
}

pub(crate) fn hook_defer_response(hook_event_name: &str, reason: &str) -> Value {
    if matches!(
        hook_event_name,
        "PermissionRequest"
            | "PostToolUse"
            | "PostToolUseFailure"
            | "Stop"
            | "StopFailure"
            | "SubagentStart"
            | "SubagentStop"
    ) {
        return json!({});
    }

    json!({
        "hookSpecificOutput": {
            "hookEventName": hook_event_name,
            "permissionDecision": "ask",
            "permissionDecisionReason": reason
        }
    })
}

pub(crate) fn fallback_hook_response(hook_event_name: &str, reason: &str) -> Value {
    hook_defer_response(hook_event_name, reason)
}
