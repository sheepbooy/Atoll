use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use super::*;

pub(crate) fn record_and_prune_request(
    state: &AppState,
    requests: &mut Vec<PermissionRequest>,
    session: &str,
) {
    if let Ok(mut totals) = state.session_request_totals.lock() {
        let total = totals.entry(session.to_string()).or_default();
        *total = total.saturating_add(1);
    }

    prune_request_history(requests);
}

pub(crate) fn prune_request_history(requests: &mut Vec<PermissionRequest>) {
    let mut resolved_total = 0usize;
    let mut resolved_by_session: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    requests.retain(|request| {
        if request.status == PermissionStatus::Pending {
            return true;
        }
        let session_count = resolved_by_session
            .entry(request.session.clone())
            .or_default();
        let keep = resolved_total < MAX_RESOLVED_REQUESTS
            && *session_count < MAX_RESOLVED_REQUESTS_PER_SESSION;
        if keep {
            resolved_total += 1;
            *session_count += 1;
        }
        keep
    });
}

pub(crate) fn submit_blocking_permission_request(
    app: AppHandle,
    payload: Value,
    stream: &TcpStream,
    build_request: impl FnOnce(String, Value, String) -> Option<PermissionRequest>,
    hook_event_name: &str,
    response_style: PermissionResponseStyle,
) -> Result<Value, String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let request = build_request(request_id.clone(), payload, iso_timestamp_now())
        .ok_or_else(|| "Unsupported hook event".to_string())?;
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&request.agent);

    let is_auto_approved = state
        .auto_approve_sessions
        .lock()
        .map(|sessions| sessions.contains(&request.session))
        .unwrap_or(false);

    if is_auto_approved {
        let mut auto_request = request;
        auto_request.status = PermissionStatus::Approved;
        auto_request.detail = format!("{} Auto-approved.", auto_request.detail);
        let session_id = auto_request.session.clone();
        touch_session_activity(&state, &session_id);
        // Record outside the requests lock: history writes do disk I/O.
        approval_history::record_outcome(
            &state,
            &auto_request,
            approval_history::HistoryStatus::Approved,
        );
        {
            let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
            requests.insert(0, auto_request);
            record_and_prune_request(&state, &mut requests, &session_id);
        }
        roll_over_token_usage_if_needed(&state);
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
        return Ok(build_permission_response(
            response_style,
            hook_event_name,
            Decision::Approved,
            "",
            None,
        ));
    }

    let (sender, receiver) = mpsc::sync_channel(1);

    {
        let mut waiters = state
            .hook_waiters
            .lock()
            .map_err(|error| error.to_string())?;
        if waiters.len() >= MAX_HOOK_WAITERS {
            return Err("Too many pending Atoll hook requests".into());
        }
        waiters.insert(request_id.clone(), sender);
    }

    let session_id = request.session.clone();
    let session_cwd = request.cwd.clone();
    let session_agent = request.agent.clone();
    let request_command = request.command.clone();
    let request_transcript_path = request.transcript_path.clone();

    touch_session_activity(&state, &request.session);
    // Record outside the requests lock: history writes do disk I/O.
    approval_history::record_pending(&state, &request);
    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        requests.insert(0, request);
        record_and_prune_request(&state, &mut requests, &session_id);
    }
    roll_over_token_usage_if_needed(&state);
    if matches!(session_agent, AgentKind::Claude) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            None,
        );
        let host =
            detect_host_for_claude_hook(&state, stream, &session_cwd, &request_transcript_path);
        if host != platform::SessionHost::Unknown {
            crate::store_session_host(&state, &session_id, host);
        }
    }
    if matches!(session_agent, AgentKind::Codex) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            request_transcript_path.as_deref(),
        );
        let host =
            detect_host_for_codex_hook(&state, stream, &session_cwd, &request_transcript_path);
        if host != platform::SessionHost::Unknown {
            crate::store_session_host(&state, &session_id, host);
        }
    }
    if matches!(session_agent, AgentKind::Cursor) {
        register_known_session(
            &state,
            &session_id,
            session_agent.clone(),
            &session_cwd,
            request_transcript_path.as_deref(),
        );
        if get_stored_session_host(&state, &session_id) == platform::SessionHost::Unknown {
            let host = detect_host_for_cursor_hook(stream);
            if host != platform::SessionHost::Unknown {
                crate::store_session_host(&state, &session_id, host);
            }
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;

    if approval_notice_is_notify(&state) {
        show_island_quietly(&app);
        send_approval_notification(&app, agent_label, &request_command, &session_cwd);
    } else {
        show_main_window_for_approval(&app);
    }

    let deadline = Instant::now() + HOOK_RESPONSE_TIMEOUT;
    loop {
        match receiver.recv_timeout(HOOK_POLL_INTERVAL) {
            Ok(DecisionWithNote {
                decision,
                note,
                updated_input,
            }) => {
                return Ok(build_permission_response(
                    response_style,
                    hook_event_name,
                    decision,
                    &note,
                    updated_input,
                ))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                remove_pending_waiter(&state, &request_id);
                return Ok(build_hook_defer_response(
                    response_style,
                    hook_event_name,
                    "Atoll internal error",
                ));
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if is_peer_disconnected(stream) {
                    remove_pending_waiter(&state, &request_id);
                    mark_request_completed_externally(&state, &app, &request_id, agent_label);
                    return Ok(json!({}));
                }
                if Instant::now() >= deadline {
                    remove_pending_waiter(&state, &request_id);
                    mark_request_denied(
                        &state,
                        &app,
                        &request_id,
                        "Timed out waiting for Atoll approval.",
                    );
                    return Ok(build_hook_defer_response(
                        response_style,
                        hook_event_name,
                        "Atoll approval timed out",
                    ));
                }
            }
        }
    }
}

pub(crate) fn agent_resolved_label(agent: &AgentKind) -> &'static str {
    match agent {
        AgentKind::Codex => "Codex",
        AgentKind::Claude => "Claude",
        AgentKind::Cursor => "Cursor",
        AgentKind::Zcode => "ZCode",
        AgentKind::Gemini => "Gemini",
        _ => "Agent",
    }
}

pub(crate) fn remove_pending_waiter(state: &AppState, request_id: &str) {
    if let Ok(mut waiters) = state.hook_waiters.lock() {
        waiters.remove(request_id);
    }
}

pub(crate) fn mark_request_completed_externally(
    state: &AppState,
    app: &AppHandle,
    request_id: &str,
    agent_label: &str,
) {
    let resolved_suffix = format!("Resolved in {agent_label}.");
    let (resolved_session_id, resolved_transcript_path, resolved_agent, resolved_request) = {
        let Ok(mut requests) = state.requests.lock() else {
            return;
        };

        let mut resolved_session_id: Option<String> = None;
        let mut resolved_transcript_path: Option<String> = None;
        let mut resolved_agent: Option<AgentKind> = None;
        let mut resolved_request: Option<PermissionRequest> = None;
        if let Some(request) = requests.iter_mut().find(|r| r.id == request_id) {
            if request.status == PermissionStatus::Pending {
                request.status = PermissionStatus::Approved;
                if !request.detail.contains(&resolved_suffix) {
                    request.detail = format!("{} {resolved_suffix}", request.detail);
                }
                touch_session_activity(state, &request.session);
                resolved_session_id = Some(request.session.clone());
                resolved_transcript_path = request.transcript_path.clone();
                resolved_agent = Some(request.agent.clone());
                resolved_request = Some(request.clone());
            }
        }
        (
            resolved_session_id,
            resolved_transcript_path,
            resolved_agent,
            resolved_request,
        )
    };
    if let Some(request) = &resolved_request {
        approval_history::record_outcome(
            state,
            request,
            approval_history::HistoryStatus::AnsweredElsewhere,
        );
    }
    roll_over_token_usage_if_needed(state);

    if let Some(session_id) = resolved_session_id.as_deref() {
        if let Err(error) = refresh_session_token_usage(
            state,
            session_id,
            resolved_transcript_path.as_deref(),
            resolved_agent.as_ref(),
        ) {
            eprintln!("Atoll token usage refresh failed: {error}");
        }
    }

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

pub(crate) fn mark_request_denied(state: &AppState, app: &AppHandle, request_id: &str, note: &str) {
    let denied_request = {
        let Ok(mut requests) = state.requests.lock() else {
            return;
        };

        let mut denied_request: Option<PermissionRequest> = None;
        if let Some(request) = requests.iter_mut().find(|request| request.id == request_id) {
            request.status = PermissionStatus::Denied;
            request.detail = format!("{} {note}", request.detail);
            touch_session_activity(state, &request.session);
            denied_request = Some(request.clone());
        }
        denied_request
    };
    if let Some(request) = &denied_request {
        approval_history::record_outcome(state, request, approval_history::HistoryStatus::Expired);
    }
    roll_over_token_usage_if_needed(state);

    let snapshot = build_snapshot(app, state);
    let _ = app.emit("snapshot-changed", &snapshot);
}

pub(crate) fn sync_tool_completion(
    app: AppHandle,
    payload: Value,
    agent: AgentKind,
    stream: Option<&TcpStream>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let mut completed_session_id = payload_session_id(&payload).map(str::to_string);
    let mut completed_transcript_path = payload_transcript_path(&payload);
    let transcript_path = payload_transcript_path(&payload);
    let cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(
            payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
            transcript_path.as_deref(),
        ),
        AgentKind::Cursor => resolve_cursor_cwd(&payload),
        _ => payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
    };
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, &cwd, None);

    if let Some(session_id) = completed_session_id.as_deref() {
        if codex_internal {
            purge_tracked_session(&state, session_id, completed_transcript_path.as_deref());
        } else {
            register_known_session(
                &state,
                session_id,
                agent.clone(),
                &cwd,
                completed_transcript_path.as_deref(),
            );
            if matches!(agent, AgentKind::Claude) {
                let host = detect_host_for_claude_non_permission_hook(
                    stream,
                    &cwd,
                    completed_transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Codex) {
                let host = detect_host_for_codex_non_permission_hook(
                    stream,
                    &cwd,
                    completed_transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Cursor) {
                maybe_detect_and_store_cursor_host(&state, session_id, stream);
            }
        }
    }

    let (completed_request_id, completed_request) = {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        let completed_request_id =
            mark_matching_pending_request_complete(&mut requests, &payload, &completed_suffix);
        let mut completed_request: Option<PermissionRequest> = None;

        if let Some(request_id) = completed_request_id.as_deref() {
            if let Some(matched_request) = requests.iter().find(|request| request.id == request_id)
            {
                if completed_session_id.is_none() {
                    completed_session_id = Some(matched_request.session.clone());
                }
                if completed_transcript_path.is_none() {
                    completed_transcript_path = matched_request.transcript_path.clone();
                }
                completed_request = Some(matched_request.clone());
            }
        }

        if let Some(session_id) = completed_session_id.as_deref() {
            if completed_transcript_path.is_none() {
                completed_transcript_path = requests
                    .iter()
                    .filter(|request| request.session == session_id)
                    .find_map(|request| request.transcript_path.clone());
            }
        }
        (completed_request_id, completed_request)
    };

    // The tool ran while the request was still pending, i.e. it was resolved
    // in the agent's own flow rather than through an Atoll click.
    if let Some(request) = &completed_request {
        approval_history::record_outcome(
            &state,
            request,
            approval_history::HistoryStatus::AnsweredElsewhere,
        );
    }

    if let Some(session_id) = completed_session_id.as_deref() {
        if !codex_internal {
            if let Err(error) = refresh_session_token_usage(
                &state,
                session_id,
                completed_transcript_path.as_deref(),
                Some(&agent),
            ) {
                eprintln!("Atoll token usage refresh failed: {error}");
            }
        }
    }

    if let Some(request_id) = completed_request_id.as_deref() {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Approved,
                    note: String::new(),
                    updated_input: None,
                });
            }
        }
    }

    roll_over_token_usage_if_needed(&state);
    if matches!(agent, AgentKind::Cursor) {
        schedule_observer_snapshot_emit(&app);
        return Ok(());
    }

    let snapshot = build_snapshot(&app, &state);

    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn sync_turn_completion(
    app: AppHandle,
    payload: Value,
    agent: AgentKind,
    touch_activity: bool,
    stream: Option<&TcpStream>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let agent_label = agent_resolved_label(&agent);
    let completed_suffix = format!("Completed in {agent_label}.");
    let session_id = payload_session_id(&payload).map(str::to_string);
    let transcript_path = payload_transcript_path(&payload);
    let cwd = match agent {
        AgentKind::Codex => resolve_codex_session_cwd(
            payload.get("cwd").and_then(Value::as_str).unwrap_or("."),
            transcript_path.as_deref(),
        ),
        AgentKind::Cursor => resolve_cursor_cwd(&payload),
        _ => payload
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string(),
    };
    let mut completed_request_id: Option<String> = None;
    let mut completed_request: Option<PermissionRequest> = None;
    let codex_internal =
        matches!(agent, AgentKind::Codex) && is_codex_internal_session(&agent, &cwd, None);

    if let Some(session_id) = session_id.as_deref() {
        {
            let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
            if let Some(index) = latest_pending_request_index(&requests, Some(session_id)) {
                let request = requests
                    .get_mut(index)
                    .expect("index from latest_pending_request_index should be valid");
                request.status = PermissionStatus::Approved;
                if !request.detail.contains(&completed_suffix) {
                    request.detail = format!("{} {completed_suffix}", request.detail);
                }
                completed_request_id = Some(request.id.clone());
                completed_request = Some(request.clone());
            }
        }
        // The tool ran while the request was still pending, i.e. it was
        // resolved in the agent's own flow rather than through an Atoll click.
        if let Some(request) = &completed_request {
            approval_history::record_outcome(
                &state,
                request,
                approval_history::HistoryStatus::AnsweredElsewhere,
            );
        }

        if codex_internal {
            purge_tracked_session(&state, session_id, transcript_path.as_deref());
        } else {
            if touch_activity {
                touch_session_activity(&state, session_id);
            }
            register_known_session(
                &state,
                session_id,
                agent.clone(),
                &cwd,
                transcript_path.as_deref(),
            );
            if matches!(agent, AgentKind::Claude) {
                let host = detect_host_for_claude_non_permission_hook(
                    stream,
                    &cwd,
                    transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Codex) {
                let host = detect_host_for_codex_non_permission_hook(
                    stream,
                    &cwd,
                    transcript_path.as_deref(),
                );
                if host != platform::SessionHost::Unknown {
                    crate::store_session_host(&state, session_id, host);
                }
            }
            if matches!(agent, AgentKind::Cursor) {
                maybe_detect_and_store_cursor_host(&state, session_id, stream);
            }
            // Cursor tokens are ingested from afterAgentResponse/sessionEnd; stop
            // would double-count the same turn when both hooks fire.
            if !matches!(agent, AgentKind::Cursor) {
                if let Err(error) = refresh_session_token_usage(
                    &state,
                    session_id,
                    transcript_path.as_deref(),
                    Some(&agent),
                ) {
                    eprintln!("Atoll token usage refresh failed: {error}");
                }
            }
        }
    }

    if let Some(request_id) = completed_request_id.as_deref() {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Approved,
                    note: String::new(),
                    updated_input: None,
                });
            }
        }
    }

    roll_over_token_usage_if_needed(&state);
    if matches!(agent, AgentKind::Cursor) {
        schedule_observer_snapshot_emit(&app);
        return Ok(());
    }

    let snapshot = build_snapshot(&app, &state);

    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn mark_matching_pending_request_complete(
    requests: &mut [PermissionRequest],
    payload: &Value,
    completed_suffix: &str,
) -> Option<String> {
    let payload_tool_use_id = payload.get("tool_use_id").and_then(Value::as_str);
    let payload_session = payload.get("session_id").and_then(Value::as_str);
    let payload_tool_name = payload.get("tool_name").and_then(Value::as_str);
    let payload_tool_input = payload.get("tool_input").cloned().unwrap_or(Value::Null);
    let payload_command =
        payload_tool_name.map(|tool_name| command_label(tool_name, &payload_tool_input));

    let matched_index = requests.iter().position(|request| {
        if request.status != PermissionStatus::Pending {
            return false;
        }

        if let (Some(request_tool_use_id), Some(payload_tool_use_id)) =
            (request.tool_use_id.as_deref(), payload_tool_use_id)
        {
            return request_tool_use_id == payload_tool_use_id;
        }

        let session_matches = payload_session
            .map(|session| request.session == session)
            .unwrap_or(false);
        let command_matches = payload_command
            .as_ref()
            .map(|command| request.command == *command)
            .unwrap_or(false);

        session_matches && command_matches
    });

    let fallback_index = matched_index
        .or_else(|| unique_pending_request_index(requests, payload_session))
        .or_else(|| latest_pending_request_index(requests, payload_session));
    let request = requests.get_mut(fallback_index?)?;

    request.status = PermissionStatus::Approved;
    if !request.detail.contains(completed_suffix) {
        request.detail = format!("{} {completed_suffix}", request.detail);
    }
    Some(request.id.clone())
}

pub(crate) fn unique_pending_request_index(
    requests: &[PermissionRequest],
    payload_session: Option<&str>,
) -> Option<usize> {
    let mut candidates = requests
        .iter()
        .enumerate()
        .filter(|(_, request)| request.status == PermissionStatus::Pending)
        .filter(|(_, request)| {
            payload_session
                .map(|session| request.session == session)
                .unwrap_or(true)
        });

    let (index, _) = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }

    Some(index)
}

pub(crate) fn latest_pending_request_index(
    requests: &[PermissionRequest],
    payload_session: Option<&str>,
) -> Option<usize> {
    let session = payload_session?;
    requests
        .iter()
        .enumerate()
        .find(|(_, request)| {
            request.status == PermissionStatus::Pending && request.session == session
        })
        .map(|(index, _)| index)
}
