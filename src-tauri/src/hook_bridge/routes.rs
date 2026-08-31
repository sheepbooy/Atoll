use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use super::*;

pub(crate) fn route_claude_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(strip_utf8_bom(&request.body))
        .map_err(|error| format!("Invalid Claude hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PreToolUse")
        .to_string();

    crate::debug_agent::log(
        "H-C",
        "hook_bridge.rs:route_claude_request",
        "claude hook received",
        json!({
            "event": hook_event_name,
            "toolName": payload.get("tool_name"),
            "sessionId": payload.get("session_id"),
            "cwd": payload.get("cwd"),
        }),
    );

    match hook_event_name.as_str() {
        "PreToolUse" | "PermissionRequest" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_claude_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::ClaudeCodex,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::ClaudeCodex,
                &hook_event_name,
                &error,
            ))
        }),
        _ => {
            enqueue_observer(ObserverJob {
                app,
                hook_event_name,
                payload,
                kind: ObserverKind::Claude,
            })?;
            Ok(json!({}))
        }
    }
}

pub(crate) fn process_claude_observer_event(
    app: AppHandle,
    hook_event_name: String,
    payload: Value,
) -> Result<(), String> {
    match hook_event_name.as_str() {
        "PostToolUse" | "PostToolUseFailure" => {
            sync_tool_completion(app, payload, AgentKind::Claude, None)
        }
        "Stop" | "StopFailure" => {
            if payload
                .get("agent_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                let state = app.state::<AppState>();
                complete_subagent(&state, &payload);
            }
            sync_turn_completion(app, payload, AgentKind::Claude, true, None)
        }
        "SubagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Claude);
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("claude-code");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(&state, session_id, AgentKind::Claude, cwd, None);
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(())
        }
        "SubagentStop" => {
            let state = app.state::<AppState>();
            complete_subagent(&state, &payload);
            sync_turn_completion(app, payload, AgentKind::Claude, false, None)
        }
        _ => Ok(()),
    }
}

pub(crate) fn route_codex_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(strip_utf8_bom(&request.body))
        .map_err(|error| format!("Invalid Codex hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PermissionRequest")
        .to_string();

    crate::debug_agent::log(
        "H-C",
        "hook_bridge.rs:route_codex_request",
        "codex hook received",
        json!({
            "event": hook_event_name,
            "sessionId": payload.get("session_id"),
            "cwd": payload.get("cwd"),
        }),
    );

    match hook_event_name.as_str() {
        "PermissionRequest" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_codex_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::ClaudeCodex,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::ClaudeCodex,
                &hook_event_name,
                &error,
            ))
        }),
        _ => {
            enqueue_observer(ObserverJob {
                app,
                hook_event_name,
                payload,
                kind: ObserverKind::Codex,
            })?;
            Ok(json!({}))
        }
    }
}

pub(crate) fn process_codex_observer_event(
    app: AppHandle,
    hook_event_name: String,
    payload: Value,
) -> Result<(), String> {
    match hook_event_name.as_str() {
        "PostToolUse" => sync_tool_completion(app, payload, AgentKind::Codex, None),
        "Stop" => {
            if payload
                .get("agent_id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.is_empty())
            {
                let state = app.state::<AppState>();
                complete_subagent(&state, &payload);
            }
            sync_turn_completion(app, payload, AgentKind::Codex, true, None)
        }
        "SubagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Codex);
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("codex");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(&state, session_id, AgentKind::Codex, cwd, None);
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(())
        }
        "SubagentStop" => {
            let state = app.state::<AppState>();
            complete_subagent(&state, &payload);
            sync_turn_completion(app, payload, AgentKind::Codex, false, None)
        }
        _ => Ok(()),
    }
}

pub(crate) fn route_zcode_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(strip_utf8_bom(&request.body))
        .map_err(|error| format!("Invalid ZCode hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("PermissionRequest")
        .to_string();

    crate::debug_agent::log(
        "H-C",
        "hook_bridge.rs:route_zcode_request",
        "zcode hook received",
        json!({
            "event": hook_event_name,
            "sessionId": payload.get("session_id"),
            "cwd": payload.get("cwd"),
        }),
    );

    match hook_event_name.as_str() {
        "PreToolUse" | "PermissionRequest" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_zcode_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::ClaudeCodex,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::ClaudeCodex,
                &hook_event_name,
                &error,
            ))
        }),
        _ => {
            enqueue_observer(ObserverJob {
                app,
                hook_event_name,
                payload,
                kind: ObserverKind::Zcode,
            })?;
            Ok(json!({}))
        }
    }
}

pub(crate) fn process_zcode_observer_event(
    app: AppHandle,
    hook_event_name: String,
    payload: Value,
) -> Result<(), String> {
    match hook_event_name.as_str() {
        "PostToolUse" | "PostToolUseFailure" => {
            sync_tool_completion(app, payload, AgentKind::Zcode, None)
        }
        "Stop" => sync_turn_completion(app, payload, AgentKind::Zcode, true, None),
        "SessionStart" | "UserPromptSubmit" => {
            let state = app.state::<AppState>();
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("zcode");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(
                &state,
                session_id,
                AgentKind::Zcode,
                cwd,
                payload_transcript_path(&payload).as_deref(),
            );
            touch_session_activity(&state, session_id);
            schedule_observer_snapshot_emit(&app);
            Ok(())
        }
        _ => Ok(()),
    }
}

pub(crate) fn route_gemini_request(
    app: AppHandle,
    request: HttpRequest,
    stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(strip_utf8_bom(&request.body))
        .map_err(|error| format!("Invalid Gemini hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("BeforeTool")
        .to_string();

    crate::debug_agent::log(
        "H-C",
        "hook_bridge.rs:route_gemini_request",
        "gemini hook received",
        json!({
            "event": hook_event_name,
            "toolName": payload.get("tool_name"),
            "sessionId": payload.get("session_id"),
            "cwd": payload.get("cwd"),
        }),
    );

    match hook_event_name.as_str() {
        // Gemini CLI gates tool execution on the BeforeTool hook response, so
        // this is the blocking approval path (see atoll-gemini-hook.mjs for why
        // read-only tools never reach the bridge).
        "BeforeTool" => submit_blocking_permission_request(
            app,
            payload,
            stream,
            |id, payload, at| permission_request_from_gemini_payload(id, payload, at),
            &hook_event_name,
            PermissionResponseStyle::Gemini,
        )
        .or_else(|error| {
            Ok(build_hook_defer_response(
                PermissionResponseStyle::Gemini,
                &hook_event_name,
                &error,
            ))
        }),
        _ => {
            enqueue_observer(ObserverJob {
                app,
                hook_event_name,
                payload,
                kind: ObserverKind::Gemini,
            })?;
            Ok(json!({}))
        }
    }
}

pub(crate) fn process_gemini_observer_event(
    app: AppHandle,
    hook_event_name: String,
    payload: Value,
) -> Result<(), String> {
    match hook_event_name.as_str() {
        "AfterTool" => sync_tool_completion(app, payload, AgentKind::Gemini, None),
        "AfterAgent" => sync_turn_completion(app, payload, AgentKind::Gemini, true, None),
        "SessionStart" | "SessionEnd" | "Notification" => {
            let state = app.state::<AppState>();
            let session_id = payload
                .get("session_id")
                .and_then(Value::as_str)
                .unwrap_or("gemini");
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or(".");
            register_known_session(
                &state,
                session_id,
                AgentKind::Gemini,
                cwd,
                payload_transcript_path(&payload).as_deref(),
            );
            touch_session_activity(&state, session_id);
            schedule_observer_snapshot_emit(&app);
            Ok(())
        }
        _ => Ok(()),
    }
}

/// Cursor fires `preToolUse` for *every* tool call (unlike Claude Code which
/// only fires `PermissionRequest` for dangerous ones).  Since Cursor already
/// has its own permission management UI (auto-approve / ask settings), Atoll
/// should not duplicate that gating.  All Cursor preToolUse events are
/// auto-approved so Atoll acts as an observer (session tracking, token usage)
/// rather than a secondary permission gate.

/// Resolve parent session for Cursor subagent tool/stop events so they are not
/// registered as independent sessions.
pub(crate) fn resolve_cursor_subagent_parent(state: &AppState, payload: &Value) -> Option<String> {
    resolve_cursor_session_for_payload(state, payload)
}

pub(crate) fn cursor_observer_session_id(state: &AppState, payload: &Value) -> String {
    resolve_cursor_subagent_parent(state, payload)
        .or_else(|| crate::payload_cursor_session_id(payload).map(str::to_string))
        .unwrap_or_else(|| "cursor".to_string())
}

pub(crate) fn cursor_stop_should_ingest_tokens(state: &AppState, payload: &Value) -> bool {
    let session_id = cursor_observer_session_id(state, payload);
    !cursor_lifecycle_token_seen(state, &session_id)
}

/// Attribute a Cursor permission request to its parent session when the event
/// belongs to a subagent (avoids duplicate top-level session rows).
pub(crate) fn attribute_cursor_request_to_parent_session(
    state: &AppState,
    payload: &Value,
    request: &mut PermissionRequest,
) {
    if let Some(parent_id) = resolve_cursor_subagent_parent(state, payload) {
        request.session = parent_id;
    }
}

/// Register or refresh a Cursor session from observer hooks and optionally ingest tokens.
pub(crate) fn observe_cursor_session(
    app: &AppHandle,
    state: &AppState,
    payload: &Value,
    stream: Option<&TcpStream>,
    ingest_tokens: bool,
    token_source: &str,
) -> Result<(), String> {
    let parent_session = resolve_cursor_subagent_parent(state, payload);
    let session_id = cursor_observer_session_id(state, payload);
    let mut cwd = resolve_cursor_cwd(payload);
    let mut transcript_path = payload_transcript_path(payload);
    // Cursor on Windows may report a transcript_path with a URI prefix or GBK
    // mojibake; even after normalization it can point at a missing file, so drop
    // invalid paths and let on-disk discovery recover the real transcript.
    if let Some(ref candidate) = transcript_path {
        if !std::path::Path::new(candidate).is_file() {
            transcript_path = None;
        }
    }
    if crate::is_unresolved_cursor_cwd(&cwd) || transcript_path.is_none() {
        if let Some(lookup_id) = crate::payload_cursor_lookup_id(payload) {
            if let Some((path, workspace)) = crate::discover_cursor_agent_transcript(lookup_id) {
                if transcript_path.is_none() {
                    transcript_path = Some(path);
                }
                if crate::is_unresolved_cursor_cwd(&cwd)
                    && !crate::is_unresolved_cursor_cwd(&workspace)
                {
                    cwd = workspace;
                }
            }
        }
    }
    if parent_session.is_none() {
        register_known_session(
            state,
            &session_id,
            AgentKind::Cursor,
            &cwd,
            transcript_path.as_deref(),
        );
        if let Some(conv_id) = crate::payload_conversation_id(payload) {
            if let Ok(mut known) = state.known_sessions.lock() {
                if let Some(entry) = known.get_mut(&session_id) {
                    entry.conversation_id = Some(conv_id.to_string());
                }
            }
        } else if session_id.len() >= crate::CURSOR_TRANSCRIPT_PREFIX_MIN_LEN {
            if let Some((path, _workspace)) = crate::discover_cursor_agent_transcript(&session_id) {
                if let Ok(mut known) = state.known_sessions.lock() {
                    if let Some(entry) = known.get_mut(&session_id) {
                        if entry.transcript_path.is_none() {
                            entry.transcript_path = Some(path.clone());
                        }
                        if entry.conversation_id.is_none() {
                            if let Some(stem) = std::path::Path::new(&path)
                                .parent()
                                .and_then(|p| p.file_name())
                                .and_then(|name| name.to_str())
                            {
                                entry.conversation_id = Some(stem.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    // #region agent log
    crate::debug_agent::log(
        "H-E",
        "hook_bridge.rs:observe_cursor_session",
        "cursor session observed",
        json!({
            "sessionId": session_id,
            "cwd": cwd,
            "hasTranscript": transcript_path.is_some(),
            "tokenSource": token_source,
            "parentSession": parent_session,
        }),
    );
    // #endregion
    if let Some(mode) = payload
        .get("composer_mode")
        .or_else(|| payload.get("composerMode"))
        .and_then(Value::as_str)
    {
        eprintln!("Atoll Cursor {token_source}: composer_mode={mode} session={session_id}");
    }
    touch_session_activity(state, &session_id);
    if let Some(stream) = stream {
        maybe_detect_and_store_cursor_host(state, &session_id, Some(stream));
    } else if get_stored_session_host(state, &session_id) == platform::SessionHost::Unknown {
        crate::store_session_host(state, &session_id, platform::SessionHost::CursorIde);
    }
    if ingest_tokens {
        let has_tokens = cursor_payload_has_token_usage(payload);
        ingest_cursor_token_usage_from_payload(state, &session_id, payload, token_source)?;
        if has_tokens && matches!(token_source, "afterAgentResponse" | "sessionEnd") {
            remember_cursor_lifecycle_token_session(state, &session_id);
        }
    }
    schedule_observer_snapshot_emit(app);
    Ok(())
}

pub(crate) fn route_cursor_request(
    app: AppHandle,
    request: HttpRequest,
    _stream: &TcpStream,
) -> Result<Value, String> {
    let payload: Value = serde_json::from_slice(strip_utf8_bom(&request.body))
        .map_err(|error| format!("Invalid Cursor hook payload: {error}"))?;

    let hook_event_name = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("preToolUse")
        .to_string();

    // #region agent log
    crate::debug_agent::log(
        "H-D",
        "hook_bridge.rs:route_cursor_request",
        "cursor hook received",
        json!({
            "event": hook_event_name,
            "hasWorkspaceRoots": payload.get("workspace_roots").is_some(),
            "conversationId": payload.get("conversation_id"),
            "sessionId": payload.get("session_id"),
            "cwd": payload.get("cwd"),
        }),
    );
    // #endregion

    enqueue_observer(ObserverJob {
        app,
        hook_event_name: hook_event_name.clone(),
        payload,
        kind: ObserverKind::Cursor,
    })?;

    match hook_event_name.as_str() {
        "beforeSubmitPrompt" => Ok(json!({ "continue": true })),
        "preToolUse" => Ok(json!({ "permission": "allow" })),
        "sessionStart" | "afterAgentResponse" | "sessionEnd" | "afterAgentThought"
        | "postToolUse" | "postToolUseFailure" | "stop" | "subagentStart" | "subagentStop" => {
            Ok(json!({}))
        }
        _ => Ok(json!({})),
    }
}

pub(crate) fn process_cursor_observer_event(
    app: AppHandle,
    hook_event_name: String,
    payload: Value,
) -> Result<(), String> {
    match hook_event_name.as_str() {
        "sessionStart" | "afterAgentResponse" | "sessionEnd" | "afterAgentThought" => {
            let state = app.state::<AppState>();
            let ingest_tokens = matches!(
                hook_event_name.as_str(),
                "afterAgentResponse" | "sessionEnd"
            );
            observe_cursor_session(
                &app,
                &state,
                &payload,
                None,
                ingest_tokens,
                hook_event_name.as_str(),
            )
        }
        "beforeSubmitPrompt" => {
            let state = app.state::<AppState>();
            observe_cursor_session(&app, &state, &payload, None, false, "beforeSubmitPrompt")
        }
        "preToolUse" => {
            let state = app.state::<AppState>();
            observe_cursor_session(&app, &state, &payload, None, false, "preToolUse")?;
            if let Some(mut request) = permission_request_from_cursor_payload(
                uuid::Uuid::new_v4().to_string(),
                payload.clone(),
                crate::iso_timestamp_now(),
            ) {
                // Subagent tool events carry their own conversation_id; attribute
                // them to the parent session so they do not appear as a duplicate
                // top-level row in the active session list.
                attribute_cursor_request_to_parent_session(&state, &payload, &mut request);
                request.status = PermissionStatus::Approved;
                request.detail = format!("{} Auto-approved.", request.detail);
                let session_id = request.session.clone();
                touch_session_activity(&state, &session_id);
                approval_history::record_outcome(
                    &state,
                    &request,
                    approval_history::HistoryStatus::Approved,
                );
                if let Ok(mut requests) = state.requests.lock() {
                    requests.insert(0, request);
                    record_and_prune_request(&state, &mut requests, &session_id);
                }
                roll_over_token_usage_if_needed(&state);
                schedule_observer_snapshot_emit(&app);
            }
            Ok(())
        }
        "postToolUse" | "postToolUseFailure" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload);
            drop(state);
            let payload = if let Some(parent_id) = parent_session {
                let mut p = payload;
                p.as_object_mut()
                    .unwrap()
                    .insert("session_id".to_string(), Value::String(parent_id));
                p
            } else {
                payload
            };
            sync_tool_completion(app, payload, AgentKind::Cursor, None)
        }
        "stop" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload);
            if payload_subagent_id(&payload).is_some() {
                complete_subagent(&state, &payload);
            }
            drop(state);
            let payload = if let Some(parent_id) = parent_session {
                let mut p = payload;
                p.as_object_mut()
                    .unwrap()
                    .insert("session_id".to_string(), Value::String(parent_id));
                p
            } else {
                payload
            };
            let state = app.state::<AppState>();
            let ingest_tokens = cursor_stop_should_ingest_tokens(&state, &payload);
            observe_cursor_session(&app, &state, &payload, None, ingest_tokens, "stop")?;
            sync_turn_completion(app, payload, AgentKind::Cursor, true, None)
        }
        "subagentStart" => {
            let state = app.state::<AppState>();
            register_subagent_start(&state, &payload, AgentKind::Cursor);
            let session_id = payload_subagent_parent_session_id(&payload)
                .or_else(|| payload_session_id(&payload))
                .unwrap_or("cursor");
            let cwd = resolve_cursor_cwd(&payload);
            let transcript_path = payload_transcript_path(&payload);
            register_known_session(
                &state,
                session_id,
                AgentKind::Cursor,
                &cwd,
                transcript_path.as_deref(),
            );
            if get_stored_session_host(&state, session_id) == platform::SessionHost::Unknown {
                crate::store_session_host(&state, session_id, platform::SessionHost::CursorIde);
            }
            touch_session_activity(&state, session_id);
            emit_subagent_snapshot(&app, &state);
            Ok(())
        }
        "subagentStop" => {
            let state = app.state::<AppState>();
            let parent_session = resolve_cursor_subagent_parent(&state, &payload)
                .or_else(|| payload_subagent_parent_session_id(&payload).map(str::to_string));
            complete_subagent(&state, &payload);
            drop(state);
            let payload = if let Some(parent_id) = parent_session {
                let mut p = payload;
                p.as_object_mut()
                    .unwrap()
                    .insert("session_id".to_string(), Value::String(parent_id));
                p
            } else {
                payload
            };
            sync_turn_completion(app, payload, AgentKind::Cursor, false, None)
        }
        _ => Ok(()),
    }
}
