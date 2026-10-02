//! Background monitors and timers: Now Playing polling, clipboard watching,
//! lyrics lookup, auto-archiving, token-history persistence, and the periodic
//! token-refresh sweep.
const TOKEN_REFRESH_INTERVAL_ACTIVE: Duration = Duration::from_millis(900);
const TOKEN_REFRESH_INTERVAL_IDLE: Duration = Duration::from_secs(5);
const HOOK_ACTIVITY_IDLE_THRESHOLD: Duration = Duration::from_secs(30);

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use super::*;

pub(crate) const AUTO_ARCHIVE_INTERVAL: Duration = Duration::from_secs(10);
pub(crate) const TOKEN_SNAPSHOT_MIN_INTERVAL: Duration = Duration::from_secs(2);
pub(crate) const TOKEN_HISTORY_WRITE_INTERVAL: Duration = Duration::from_secs(2);

/// How long to wait before retrying lyrics lookup for a track that recently
/// had no lyrics anywhere — retrying every poll would hammer the lyrics APIs
/// (and risk rate limits) for lyric-less tracks.
pub(crate) const LYRICS_MISS_RETRY_AFTER: Duration = Duration::from_secs(30);

/// Position to report for the `now-playing-position` event, with paused
/// creep removed. Some players (QQ Music) keep advancing elapsedTime while
/// paused and snap it back on resume; while paused, hold the last adopted
/// position so the progress bar and lyrics don't creep forward and jump
/// back. Real jumps (backward, or forward by ≥2s in one poll — seeks) are
/// adopted; `None` passes through (the frontend ignores null positions).
pub(crate) fn sanitize_paused_position(
    raw: Option<f64>,
    playing: bool,
    prev_raw: Option<f64>,
    held: &mut Option<f64>,
) -> Option<f64> {
    if playing {
        *held = raw;
        return raw;
    }
    match (raw, prev_raw, *held) {
        (Some(r), Some(pr), Some(h)) if r >= pr && r - pr < 2.0 => Some(h),
        _ => {
            *held = raw;
            raw
        }
    }
}

pub(crate) fn start_media_monitor(app: AppHandle) {
    crate::shared_media::start(app);
}

pub(crate) const BLUETOOTH_BATTERY_POLL_INTERVAL: Duration = Duration::from_secs(60);

/// Polls connected Bluetooth devices' battery levels every
/// [`BLUETOOTH_BATTERY_POLL_INTERVAL`] and emits `bluetooth-battery-changed`
/// only when the report actually changes. While enabled, posts one
/// low-battery notification per device per discharge below the alert
/// threshold (recovery re-arms the alert). Disabling the card clears the
/// frontend state with a `null` payload.
pub(crate) fn start_bluetooth_battery_monitor(app: AppHandle) {
    thread::spawn(move || {
        // Let the app settle before the first fetch (system_profiler is slow).
        thread::sleep(Duration::from_secs(5));
        let mut last: Option<bluetooth_battery::BluetoothBatteryReport> = None;
        let mut alerted: HashSet<String> = HashSet::new();
        let gatt_app = app.clone();
        loop {
            let (enabled, alert_enabled, threshold, language) = {
                let state = app.state::<AppState>();
                let enabled = *lock_state(&state.bluetooth_battery_card_enabled);
                let alert_enabled = *lock_state(&state.bluetooth_battery_alert_enabled);
                let threshold = *lock_state(&state.bluetooth_battery_alert_threshold);
                let language = lock_state(&state.notification_language).clone();
                (enabled, alert_enabled, threshold, language)
            };
            if !enabled {
                if last.is_some() {
                    last = None;
                    alerted.clear();
                    let _ = app.emit(
                        "bluetooth-battery-changed",
                        Option::<bluetooth_battery::BluetoothBatteryReport>::None,
                    );
                }
                thread::sleep(BLUETOOTH_BATTERY_POLL_INTERVAL);
                continue;
            }
            let mut report = bluetooth_battery::fetch_bluetooth_battery();
            // Second pass: for devices the OS doesn't report a level for,
            // read the standard GATT Battery Service ourselves (may trigger
            // the one-time Bluetooth permission prompt on first use).
            #[cfg(target_os = "macos")]
            let mut gatt_probe = |name: &str| gatt_battery::probe_device(&gatt_app, name);
            #[cfg(not(target_os = "macos"))]
            let mut gatt_probe = |_: &str| gatt_battery::ProbeOutcome::NoData;
            {
                let state = app.state::<AppState>();
                lock_state(&state.gatt_battery_cache).fill_missing(
                    &mut report.devices,
                    Instant::now(),
                    &mut gatt_probe,
                );
            }
            if alert_enabled {
                for (name, percent) in bluetooth_battery::collect_low_battery_alerts(
                    &report.devices,
                    threshold,
                    &mut alerted,
                ) {
                    send_low_battery_notification(&app, &name, percent, &language);
                }
            } else {
                alerted.clear();
            }
            if last.as_ref() != Some(&report) {
                last = Some(report.clone());
                let _ = app.emit("bluetooth-battery-changed", &report);
            }
            thread::sleep(BLUETOOTH_BATTERY_POLL_INTERVAL);
        }
    });
}

/// Post the low-battery system notification; copy follows the UI language
/// (same convention as the approval notifications).
fn send_low_battery_notification(app: &AppHandle, device_name: &str, percent: u8, language: &str) {
    use tauri_plugin_notification::NotificationExt;
    let (title, body) = if language == "zh-CN" {
        (
            "蓝牙设备电量低".to_string(),
            format!("{device_name} 电量仅剩 {percent}%"),
        )
    } else {
        (
            "Bluetooth device low on battery".to_string(),
            format!("{device_name} battery is at {percent}%"),
        )
    };
    if let Err(error) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .sound("default")
        .show()
    {
        eprintln!("[Atoll] low battery notification failed: {error}");
    }
}

/// Run a closure on the app's main thread and wait briefly for its result.
/// NSPasteboard must only be touched from the main thread on macOS, so every
/// pasteboard read/write is marshaled through this. Other platforms call
/// the closure directly (Win32 clipboard APIs are thread-safe).
pub(crate) fn call_on_main_thread<T, F>(app: &AppHandle, f: F) -> Option<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(f());
        })
        .ok()?;
        rx.recv_timeout(Duration::from_millis(2000)).ok()
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Some(f())
    }
}

pub(crate) fn clipboard_sequence(app: &AppHandle) -> u64 {
    call_on_main_thread(app, clipboard_history::clipboard_sequence).unwrap_or(0)
}

pub(crate) fn read_clipboard_snapshot(
    app: &AppHandle,
) -> Option<clipboard_history::ClipboardPayload> {
    call_on_main_thread(app, clipboard_history::read_clipboard_snapshot).flatten()
}

pub(crate) fn write_clipboard_payload(
    app: &AppHandle,
    payload: &clipboard_history::ClipboardPayload,
) -> bool {
    let payload = payload.clone();
    call_on_main_thread(app, move || {
        clipboard_history::write_clipboard_payload(&payload)
    })
    .unwrap_or(false)
}

pub(crate) fn start_clipboard_monitor(app: AppHandle) {
    thread::spawn(move || {
        // Let the app settle before the first poll.
        thread::sleep(Duration::from_secs(2));
        // Baseline the clipboard sequence counter: the content already on
        // the clipboard at startup is not treated as a fresh copy. Only
        // sequence changes that happen while monitoring get recorded.
        let mut last_seq: Option<u64> = None;
        loop {
            thread::sleep(Duration::from_millis(1000));
            let state = app.state::<AppState>();
            let enabled = *lock_state(&state.clipboard_history_enabled);
            if !enabled {
                last_seq = None;
                continue;
            }
            let seq = clipboard_sequence(&app);
            if seq == 0 {
                // Sequence numbers are unavailable on this platform.
                continue;
            }
            if last_seq == Some(seq) {
                continue;
            }
            let had_baseline = last_seq.is_some();
            last_seq = Some(seq);
            if !had_baseline {
                continue;
            }
            // Read the snapshot before taking the history lock: the main
            // thread runs sync commands that lock the same state.
            let Some(payload) = read_clipboard_snapshot(&app) else {
                continue;
            };
            let copied_files = match &payload {
                clipboard_history::ClipboardPayload::Files(paths) => paths.clone(),
                _ => Vec::new(),
            };
            let limit = *lock_state(&state.clipboard_history_limit);
            let mut entries = lock_state(&state.clipboard_history);
            if clipboard_history::add_entry(&mut entries, payload, limit) {
                clipboard_history::save_history(&entries);
                let snapshot = entries.clone();
                drop(entries);
                let _ = app.emit("clipboard-history-changed", &snapshot);
                // Opt-in auto-stage: copied files land in the file station.
                // The station lock is only taken after the history lock is
                // released; staging reuses the drop pipeline (dedup/cap), so
                // a copy made out of the station merely refreshes those rows.
                if !copied_files.is_empty() && *lock_state(&state.clipboard_auto_stage) {
                    let mut staged = lock_state(&state.file_station);
                    let result = file_station::stage_paths_sourced(
                        &mut staged,
                        &copied_files,
                        file_station::STAGED_FILES_LIMIT,
                        true,
                    );
                    file_station::save_history(&staged);
                    drop(staged);
                    let _ = app.emit("file-station-changed", &result.files);
                }
            }
        }
    });
}

/// Polls the current media track, fetches synced lyrics from LRCLIB on track
/// change, and emits `lyrics-changed` (full payload) + `lyrics-line-changed`
/// (current index + next line time). No-op when lyrics are disabled or no
/// media is playing.
pub(crate) fn start_lyrics_monitor(app: AppHandle) {
    crate::shared_media::start_lyrics(app);
}

pub(crate) fn start_token_history_writer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(TOKEN_HISTORY_WRITE_INTERVAL);
        let state = app.state::<AppState>();
        if !state.token_history_dirty.swap(false, Ordering::AcqRel) {
            continue;
        }
        if let Err(error) = token_history::sync_today_to_history(&state) {
            state.token_history_dirty.store(true, Ordering::Release);
            eprintln!("Atoll token history sync failed: {error}");
        }
    });
}

pub(crate) fn start_initial_maintenance(app: AppHandle) {
    thread::spawn(move || {
        let state = app.state::<AppState>();
        reconcile_incomplete_subagents_now(&state);
        backfill_cursor_session_metadata(&state);
        refresh_unknown_session_hosts(&state);
        refresh_hook_health_cache(&app, &state);
        let online = compute_listening_online(&app);
        if let Ok(mut last) = state.last_listening_online.lock() {
            *last = Some(online);
        }
        let snapshot = build_snapshot(&app, &state);
        let _ = app.emit("snapshot-changed", &snapshot);
    });
}

pub(crate) fn start_auto_archive_timer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(AUTO_ARCHIVE_INTERVAL);

        let state = app.state::<AppState>();
        let (changed, expired, stale_pending_requests) = {
            let Ok(mut requests) = state.requests.lock() else {
                continue;
            };
            let retention_secs = *state
                .session_retention_secs
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let pinned = state
                .pinned_sessions
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let last_seen_map = state
                .session_last_seen
                .lock()
                .unwrap_or_else(|e| e.into_inner());

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut changed = false;
            let mut stale_pending_requests: Vec<PermissionRequest> = Vec::new();
            for request in requests.iter_mut() {
                if request.archived {
                    continue;
                }
                // Skip pinned sessions from auto-archive.
                if pinned.contains(&request.session) {
                    continue;
                }
                let last_seen_ts = last_seen_map
                    .get(&request.session)
                    .copied()
                    .unwrap_or_else(|| parse_iso_timestamp_secs(&request.requested_at));
                if now.saturating_sub(last_seen_ts) < retention_secs {
                    continue;
                }
                if request.status == PermissionStatus::Pending {
                    request.status = PermissionStatus::Denied;
                    if !request.detail.contains("Auto-archived") {
                        request.detail =
                            format!("{} Auto-archived after idle timeout.", request.detail);
                    }
                    stale_pending_requests.push(request.clone());
                }
                request.archived = true;
                changed = true;
            }
            drop(last_seen_map);

            // Collect expired known sessions while locks are held; purge after dropping
            // all guards so purge_tracked_session can acquire them independently.
            let expired = {
                let last_seen = state
                    .session_last_seen
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let mut known = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let mut expired: Vec<(String, Option<String>)> = Vec::new();
                known.retain(|session_id, info| {
                    if pinned.contains(session_id) {
                        return true;
                    }
                    let last_seen_ts = last_seen
                        .get(session_id)
                        .copied()
                        .unwrap_or_else(|| parse_iso_timestamp_secs(&info.last_activity));
                    if now.saturating_sub(last_seen_ts) >= retention_secs {
                        expired.push((session_id.clone(), info.transcript_path.clone()));
                        false
                    } else {
                        true
                    }
                });
                expired
            };
            if !expired.is_empty() {
                changed = true;
            }

            (changed, expired, stale_pending_requests)
        };

        for request in &stale_pending_requests {
            approval_history::record_outcome(
                &state,
                request,
                approval_history::HistoryStatus::Expired,
            );
        }

        if !stale_pending_requests.is_empty() {
            if let Ok(mut waiters) = state.hook_waiters.lock() {
                for request in &stale_pending_requests {
                    if let Some(waiter) = waiters.remove(&request.id) {
                        let _ = waiter.send(DecisionWithNote {
                            decision: Decision::Denied,
                            note: "Auto-archived after idle timeout.".into(),
                            updated_input: None,
                        });
                    }
                }
            }
        }

        for (session_id, transcript_path) in expired {
            purge_tracked_session(&state, &session_id, transcript_path.as_deref());
        }

        reconcile_incomplete_subagents_if_due(&state);
        backfill_cursor_session_metadata(&state);
        refresh_unknown_session_hosts(&state);

        if changed {
            roll_over_token_usage_if_needed(&state);
            let snapshot = build_snapshot(&app, &state);
            let _ = app.emit("snapshot-changed", &snapshot);
        }
        sync_hook_health_snapshot(&app, &state);
        sync_listening_online_snapshot(&app, &state);
        #[cfg(target_os = "windows")]
        maybe_reassert_island_on_top(&app);
    });
}

#[cfg(target_os = "windows")]
pub(crate) fn maybe_reassert_island_on_top(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let window_for_thread = window.clone();
    let window_for_closure = window.clone();
    let _ = window_for_thread.run_on_main_thread(move || {
        platform::ensure_island_on_top(&window_for_closure);
    });
}

pub(crate) fn start_token_refresh_timer(app: AppHandle) {
    thread::spawn(move || {
        let mut last_snapshot_emit = Instant::now() - TOKEN_SNAPSHOT_MIN_INTERVAL;

        loop {
            let state = app.state::<AppState>();
            let tracked_sessions = {
                let requests = state.requests.lock().unwrap_or_else(|e| e.into_inner());
                let known_sessions = state
                    .known_sessions
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                collect_session_transcript_paths(&requests, &known_sessions)
            };

            let sleep_duration = if tracked_sessions.is_empty() {
                TOKEN_REFRESH_INTERVAL_IDLE
            } else {
                let recently_active = state
                    .last_hook_activity
                    .lock()
                    .map(|last| last.elapsed() < HOOK_ACTIVITY_IDLE_THRESHOLD)
                    .unwrap_or(true);
                if recently_active {
                    TOKEN_REFRESH_INTERVAL_ACTIVE
                } else {
                    TOKEN_REFRESH_INTERVAL_IDLE
                }
            };
            thread::sleep(sleep_duration);

            if tracked_sessions.is_empty() {
                continue;
            }

            let usage_before = state
                .session_token_usage
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();

            for (session_id, transcript_path, agent) in tracked_sessions {
                if let Err(error) = refresh_session_token_usage(
                    &state,
                    &session_id,
                    Some(transcript_path.as_str()),
                    Some(&agent),
                ) {
                    eprintln!("Atoll token usage refresh failed: {error}");
                }
                if matches!(agent, AgentKind::Zcode) {
                    // ZCode has no subagent hooks: derive subagent chips and
                    // their token attribution from on-disk metadata instead.
                    let today_key = current_local_day_key();
                    refresh_zcode_subagents(&app, &state, &session_id, &today_key);
                }
            }

            let usage_after = state
                .session_token_usage
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone();
            if usage_after == usage_before {
                continue;
            }

            roll_over_token_usage_if_needed(&state);
            let now = Instant::now();
            if now.duration_since(last_snapshot_emit) < TOKEN_SNAPSHOT_MIN_INTERVAL {
                continue;
            }
            last_snapshot_emit = now;
            let snapshot = build_snapshot(&app, &state);
            let _ = app.emit("snapshot-changed", &snapshot);
        }
    });
}
