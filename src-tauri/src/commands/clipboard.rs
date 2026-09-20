// Clipboard history commands.
use std::path::Path;

use tauri::{AppHandle, Emitter, State};

use crate::*;

#[tauri::command]
pub(crate) fn get_clipboard_history(
    state: State<'_, AppState>,
) -> Vec<clipboard_history::ClipboardEntry> {
    let limit = *lock_state(&state.clipboard_history_limit);
    let mut entries = lock_state(&state.clipboard_history);
    let before = entries.len();
    clipboard_history::prune_expired(&mut entries, limit);
    let result = entries.clone();
    if result.len() != before {
        clipboard_history::save_history(&entries);
    }
    result
}

#[tauri::command]
pub(crate) fn copy_clipboard_entry(app: AppHandle, state: State<'_, AppState>, id: String) -> bool {
    let entry = lock_state(&state.clipboard_history)
        .iter()
        .find(|e| e.id == id)
        .cloned();
    let Some(entry) = entry else {
        return false;
    };
    let payload = match entry.kind {
        clipboard_history::EntryKind::Text => Some(clipboard_history::ClipboardPayload::Text(
            entry.content.clone(),
        )),
        clipboard_history::EntryKind::Image => clipboard_history::read_image_blob(&entry.id)
            .map(|png| clipboard_history::ClipboardPayload::Image { png }),
        clipboard_history::EntryKind::Files => Some(clipboard_history::ClipboardPayload::Files(
            entry.content.lines().map(str::to_string).collect(),
        )),
    };
    let Some(payload) = payload else {
        return false;
    };
    write_clipboard_payload(&app, &payload)
}

#[tauri::command]
pub(crate) fn clear_clipboard_history(app: AppHandle, state: State<'_, AppState>) {
    let mut entries = lock_state(&state.clipboard_history);
    clipboard_history::clear_unfavorited(&mut entries);
    clipboard_history::save_history(&entries);
    let snapshot = entries.clone();
    drop(entries);
    let _ = app.emit("clipboard-history-changed", &snapshot);
}

#[tauri::command]
pub(crate) fn toggle_clipboard_favorite(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> bool {
    let limit = *lock_state(&state.clipboard_history_limit);
    let mut entries = lock_state(&state.clipboard_history);
    let changed = clipboard_history::toggle_favorite(&mut entries, &id, limit);
    if changed {
        clipboard_history::save_history(&entries);
        let snapshot = entries.clone();
        drop(entries);
        let _ = app.emit("clipboard-history-changed", &snapshot);
    }
    changed
}

#[tauri::command]
pub(crate) fn get_clipboard_history_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.clipboard_history_enabled)
}

#[tauri::command]
pub(crate) fn set_clipboard_history_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    *lock_state(&state.clipboard_history_enabled) = enabled;
    persist_clipboard_history_enabled(enabled);
    if enabled {
        // Snapshot the current clipboard first (main-thread read), then
        // reload persisted history under the state lock so enabling feels
        // like it captured what was just copied. Never touch the main
        // thread while holding the history lock: sync Tauri commands run
        // on the main thread and take the same lock.
        let snapshot = read_clipboard_snapshot(&app);
        let limit = *lock_state(&state.clipboard_history_limit);
        let mut entries = lock_state(&state.clipboard_history);
        *entries = clipboard_history::load_history(limit);
        if let Some(payload) = snapshot {
            if clipboard_history::add_entry(&mut entries, payload, limit) {
                clipboard_history::save_history(&entries);
            }
        }
    }
    enabled
}

#[tauri::command]
pub(crate) fn get_clipboard_history_limit(state: State<'_, AppState>) -> usize {
    *lock_state(&state.clipboard_history_limit)
}

#[tauri::command]
pub(crate) fn get_clipboard_auto_stage(state: State<'_, AppState>) -> bool {
    *lock_state(&state.clipboard_auto_stage)
}

#[tauri::command]
pub(crate) fn set_clipboard_auto_stage(state: State<'_, AppState>, enabled: bool) -> bool {
    *lock_state(&state.clipboard_auto_stage) = enabled;
    persist_clipboard_auto_stage(enabled);
    enabled
}

#[tauri::command]
pub(crate) fn set_clipboard_history_limit(state: State<'_, AppState>, limit: usize) -> usize {
    let clamped = limit.clamp(
        clipboard_history::MIN_HISTORY_LIMIT,
        clipboard_history::MAX_HISTORY_LIMIT,
    );
    *lock_state(&state.clipboard_history_limit) = clamped;
    persist_clipboard_history_limit(clamped);
    // Shrinking the limit prunes immediately (and drops trimmed blobs).
    let mut entries = lock_state(&state.clipboard_history);
    let before = entries.len();
    clipboard_history::prune_expired(&mut entries, clamped);
    if entries.len() != before {
        clipboard_history::save_history(&entries);
    }
    clamped
}

#[tauri::command]
pub(crate) fn get_clipboard_entry_thumbnail(
    state: State<'_, AppState>,
    id: String,
) -> Option<String> {
    let is_image = lock_state(&state.clipboard_history)
        .iter()
        .any(|e| e.id == id && e.kind == clipboard_history::EntryKind::Image);
    if !is_image {
        return None;
    }
    clipboard_history::read_thumbnail_data_url(&id)
}
