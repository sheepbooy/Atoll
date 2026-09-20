// File staging station commands.
use std::path::Path;

use tauri::{AppHandle, Emitter, State};

use crate::*;

// ─── File staging station ───────────────────────────────────────────────────

#[tauri::command]
pub(crate) fn get_staged_files(state: State<'_, AppState>) -> Vec<file_station::StagedFileView> {
    file_station::views(&lock_state(&state.file_station))
}

#[tauri::command]
pub(crate) fn stage_files(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> file_station::StageFilesResult {
    let mut entries = lock_state(&state.file_station);
    let result = file_station::stage_paths(&mut entries, &paths, file_station::STAGED_FILES_LIMIT);
    file_station::save_history(&entries);
    drop(entries);
    let _ = app.emit("file-station-changed", &result.files);
    result
}

#[tauri::command]
pub(crate) fn remove_staged_file(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Vec<file_station::StagedFileView> {
    let mut entries = lock_state(&state.file_station);
    file_station::remove_entry(&mut entries, &id);
    let views = file_station::views(&entries);
    file_station::save_history(&entries);
    drop(entries);
    let _ = app.emit("file-station-changed", &views);
    views
}

#[tauri::command]
pub(crate) fn clear_staged_files(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Vec<file_station::StagedFileView> {
    let mut entries = lock_state(&state.file_station);
    file_station::clear_entries(&mut entries);
    file_station::save_history(&entries);
    drop(entries);
    let _ = app.emit(
        "file-station-changed",
        Vec::<file_station::StagedFileView>::new(),
    );
    Vec::new()
}

/// Copy referenced files (by id, existing on disk only) to the clipboard as a
/// file list so the user can paste them into the target app.
#[tauri::command]
pub(crate) fn copy_staged_files_to_clipboard(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<usize, String> {
    // Collect paths under the lock, then release it: the clipboard write
    // marshals to the main thread on macOS, which also runs sync commands
    // that take this lock.
    let entries = lock_state(&state.file_station);
    let paths: Vec<String> = entries
        .iter()
        .filter(|e| ids.contains(&e.id) && Path::new(&e.path).exists())
        .map(|e| e.path.clone())
        .collect();
    drop(entries);
    if paths.is_empty() {
        return Ok(0);
    }
    let payload = clipboard_history::ClipboardPayload::Files(paths.clone());
    if write_clipboard_payload(&app, &payload) {
        Ok(paths.len())
    } else {
        Err("clipboard write failed".into())
    }
}

/// Copy referenced files (by id) to the clipboard as newline-joined path
/// text — pasteable into terminals and chat boxes; with clipboard history
/// enabled the write also lands there as a text entry.
#[tauri::command]
pub(crate) fn copy_staged_paths_to_clipboard(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<usize, String> {
    // Same lock discipline as copy_staged_files_to_clipboard.
    let entries = lock_state(&state.file_station);
    let paths: Vec<String> = entries
        .iter()
        .filter(|e| ids.contains(&e.id) && Path::new(&e.path).exists())
        .map(|e| e.path.clone())
        .collect();
    drop(entries);
    if paths.is_empty() {
        return Ok(0);
    }
    let payload = clipboard_history::ClipboardPayload::Text(paths.join("\n"));
    if write_clipboard_payload(&app, &payload) {
        Ok(paths.len())
    } else {
        Err("clipboard write failed".into())
    }
}

/// Stage clipboard history entries (by id) into the file station. Images and
/// path-less text are materialized under `~/.atoll/station/` first; see
/// `clipboard_station`.
#[tauri::command]
pub(crate) fn stage_clipboard_entries(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> file_station::StageFilesResult {
    // Clone the requested entries under the clipboard lock, then release it
    // before touching the station: materialization does fs work and the
    // emit below fans out to the frontend.
    let clip_entries: Vec<clipboard_history::ClipboardEntry> =
        lock_state(&state.clipboard_history)
            .iter()
            .filter(|e| ids.contains(&e.id))
            .cloned()
            .collect();
    if clip_entries.is_empty() {
        return file_station::StageFilesResult {
            files: file_station::views(&lock_state(&state.file_station)),
            added: 0,
            skipped: 0,
            evicted: 0,
        };
    }
    let mut entries = lock_state(&state.file_station);
    let result = clipboard_station::stage_from_clipboard_entries(
        &mut entries,
        &clip_entries,
        file_station::STAGED_FILES_LIMIT,
    );
    file_station::save_history(&entries);
    drop(entries);
    let _ = app.emit("file-station-changed", &result.files);
    result
}

/// Begin a native drag of staged files (by id) out of the island. Only files
/// still on disk are included; returns false (frontend stays silent) when
/// nothing remains or the platform cannot anchor a drag session.
#[tauri::command]
pub(crate) fn begin_staged_files_drag(
    state: State<'_, AppState>,
    window: tauri::WebviewWindow,
    ids: Vec<String>,
) -> bool {
    let paths: Vec<String> = lock_state(&state.file_station)
        .iter()
        .filter(|e| ids.contains(&e.id) && Path::new(&e.path).exists())
        .map(|e| e.path.clone())
        .collect();
    if paths.is_empty() {
        return false;
    }
    platform::begin_staged_files_drag(&window, &paths)
}
