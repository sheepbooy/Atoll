// Permission-request resolution, archiving and pinning commands.
use serde_json::Value;
use tauri::{AppHandle, Emitter, State};

use crate::*;

#[tauri::command]
pub(crate) fn resolve_permission_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    decision: Decision,
    note: String,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let status = match decision {
        Decision::Approved => PermissionStatus::Approved,
        Decision::Denied => PermissionStatus::Denied,
    };

    let Some(request) = requests.iter_mut().find(|request| request.id == id) else {
        return Err(format!("Permission request not found: {id}"));
    };

    request.status = status;
    if !note.trim().is_empty() {
        request.detail = format!("{} Note: {}", request.detail, note.trim());
    }

    let session_id = request.session.clone();
    let resolved_request = request.clone();

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(DecisionWithNote {
            decision,
            note: note.clone(),
            updated_input: None,
        });
    }

    touch_session_activity(&state, &session_id);
    drop(requests);
    approval_history::record_outcome(
        &state,
        &resolved_request,
        match decision {
            Decision::Approved => approval_history::HistoryStatus::Approved,
            Decision::Denied => approval_history::HistoryStatus::Denied,
        },
    );
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn resolve_permission_with_input(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    decision: Decision,
    note: String,
    updated_input: Option<Value>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let status = match decision {
        Decision::Approved => PermissionStatus::Approved,
        Decision::Denied => PermissionStatus::Denied,
    };

    let Some(request) = requests.iter_mut().find(|request| request.id == id) else {
        return Err(format!("Permission request not found: {id}"));
    };

    request.status = status;
    if !note.trim().is_empty() {
        request.detail = format!("{} Note: {}", request.detail, note.trim());
    }

    let session_id = request.session.clone();
    let resolved_request = request.clone();

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(DecisionWithNote {
            decision,
            note: note.clone(),
            updated_input,
        });
    }

    touch_session_activity(&state, &session_id);
    drop(requests);
    approval_history::record_outcome(
        &state,
        &resolved_request,
        match decision {
            Decision::Approved => approval_history::HistoryStatus::Approved,
            Decision::Denied => approval_history::HistoryStatus::Denied,
        },
    );
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn set_session_auto_approve(
    state: State<'_, AppState>,
    session: String,
    enabled: bool,
) -> Result<(), String> {
    let mut sessions = state
        .auto_approve_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    if enabled {
        sessions.insert(session);
    } else {
        sessions.remove(&session);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn archive_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    if let Some(request) = requests.iter_mut().find(|r| r.id == id) {
        request.archived = true;
    }
    drop(requests);
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn archive_all_resolved(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    let pinned = state
        .pinned_sessions
        .lock()
        .map_err(|error| error.to_string())?;
    // Archive-all: keep pending requests and requests belonging to pinned sessions.
    requests.retain(|request| {
        request.status == PermissionStatus::Pending || pinned.contains(&request.session)
    });
    // Also remove non-pinned known sessions.
    {
        let mut known = state
            .known_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        known.retain(|session_id, _| pinned.contains(session_id));
    }
    drop(requests);
    drop(pinned);
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn archive_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<IslandSnapshot, String> {
    let removed_pending_ids: Vec<String> = {
        let requests = state.requests.lock().map_err(|error| error.to_string())?;
        requests
            .iter()
            .filter(|request| {
                request.session == session_id && request.status == PermissionStatus::Pending
            })
            .map(|request| request.id.clone())
            .collect()
    };

    for request_id in removed_pending_ids {
        if let Ok(mut waiters) = state.hook_waiters.lock() {
            if let Some(waiter) = waiters.remove(&request_id) {
                let _ = waiter.send(DecisionWithNote {
                    decision: Decision::Denied,
                    note: "Session archived in Atoll.".into(),
                    updated_input: None,
                });
            }
        }
    }

    {
        let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
        // Remove session data outright so retention replay does not keep it visible.
        requests.retain(|request| request.session != session_id);
    }
    {
        let mut known = state
            .known_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        known.remove(&session_id);
    }
    {
        let mut pinned = state
            .pinned_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        pinned.remove(&session_id);
    }
    if let Ok(mut last_seen) = state.session_last_seen.lock() {
        last_seen.remove(&session_id);
    }
    if let Ok(mut totals) = state.session_request_totals.lock() {
        totals.remove(&session_id);
    }
    // Keep session_token_usage so archived sessions still count toward daily totals.
    roll_over_token_usage_if_needed(&state);
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn pin_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    pinned: bool,
) -> Result<IslandSnapshot, String> {
    {
        let mut pinned_set = state
            .pinned_sessions
            .lock()
            .map_err(|error| error.to_string())?;
        if pinned {
            pinned_set.insert(session_id);
        } else {
            pinned_set.remove(&session_id);
        }
    }
    let snapshot = build_snapshot(&app, &state);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}
