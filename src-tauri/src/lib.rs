use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::utils::config::Color;
use tauri::{AppHandle, Emitter, Manager};

mod approval_history;
mod approval_rules;
mod approval_spool;
mod capture;
mod clipboard_history;
mod clipboard_station;
mod debug_agent;
mod file_station;
mod hook_bridge;
mod hook_trust;
mod local_time;
mod lyrics;
#[cfg(target_os = "macos")]
mod media;
// Compiled on every platform so the pure-logic unit tests run on any host;
// the WinRT calls are cfg(windows) inside, hence the dead-code allowance on
// non-Windows targets where only the tests reference them.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
mod media_windows;
mod platform;
mod pricing;
mod risk_patterns;
mod shortcuts;
mod token_history;
mod transcript;

pub(crate) use approval_rules::*;

mod state;

pub(crate) use state::*;

mod settings;

pub(crate) use settings::*;

mod transcript_cache;

pub(crate) use transcript_cache::*;

mod tray;

pub(crate) use tray::*;

mod token_usage;

pub(crate) use token_usage::*;

mod monitors;

pub(crate) use monitors::*;

mod window;

pub(crate) use window::*;

mod session;

pub(crate) use session::*;

mod hooks;

pub(crate) use hooks::*;

mod commands;
pub(crate) use commands::*;

mod snapshot;

pub(crate) use snapshot::*;

pub(crate) use local_time::{format_unix_timestamp, iso_timestamp_now, parse_iso_timestamp_secs};

#[cfg(test)]
mod claude_hooks_tests;
#[cfg(test)]
mod codex_hooks_tests;
#[cfg(test)]
mod core_tests;
#[cfg(test)]
mod cursor_hooks_tests;
#[cfg(test)]
mod cursor_subagent_tests;
#[cfg(test)]
mod gemini_hooks_tests;
#[cfg(test)]
mod hook_bridge_tests;
#[cfg(test)]
mod hook_script_path_tests;
#[cfg(test)]
mod hooks_asset_tests;
#[cfg(test)]
mod monitors_tests;
#[cfg(test)]
mod session_tests;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod token_usage_tests;
#[cfg(test)]
mod transcript_cache_tests;
#[cfg(test)]
mod tray_tests;
#[cfg(test)]
mod window_tests;
#[cfg(test)]
mod zcode_chat_tests;
#[cfg(test)]
mod zcode_hooks_tests;
#[cfg(test)]
mod zcode_token_tests;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(Vec::new()),
            session_request_totals: Mutex::new(HashMap::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            approval_rules: Mutex::new(approval_rules::load_rules_from_disk()),
            risk_guard_enabled: Mutex::new(load_risk_guard_enabled()),
            compact_width: Mutex::new(COMPACT_WINDOW_WIDTH),
            compact_left_width: Mutex::new(0.0),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
            notch_metrics: Mutex::new(NotchMetrics::default()),
            session_last_seen: Mutex::new(HashMap::new()),
            session_retention_secs: Mutex::new(DEFAULT_SESSION_RETENTION_SECS),
            subagent_retention_secs: Mutex::new(DEFAULT_SUBAGENT_RETENTION_SECS),
            session_token_usage: Mutex::new(HashMap::new()),
            session_token_usage_by_model: Mutex::new(HashMap::new()),
            session_agent_map: Mutex::new(HashMap::new()),
            token_usage_file_offsets: Mutex::new(HashMap::new()),
            token_usage_day: Mutex::new(current_local_day_key()),
            startup_daily_floor: Mutex::new(token_history::load_today_baseline()),
            startup_daily_floor_by_model: Mutex::new(token_history::load_today_by_model_baseline()),
            absolute_token_sessions: Mutex::new(HashSet::new()),
            daily_tokens_baseline: Mutex::new(token_history::load_today_baseline()),
            known_sessions: Mutex::new(HashMap::new()),
            pinned_sessions: Mutex::new(HashSet::new()),
            previous_app_pid: Mutex::new(None),
            last_listening_online: Mutex::new(None),
            last_hook_health: Mutex::new(None),
            bridge_port: AtomicU16::new(0),
            bridge_auth_token: Mutex::new(uuid::Uuid::new_v4().to_string()),
            last_bridge_reachable: Mutex::new(None),
            active_subagents: Mutex::new(Vec::new()),
            cursor_subagent_conversations: Mutex::new(HashMap::new()),
            cursor_lifecycle_token_sessions: Mutex::new(HashSet::new()),
            last_subagent_snapshot_emit: Mutex::new(Instant::now() - Duration::from_secs(10)),
            snapshot_debounce_generation: AtomicU64::new(0),
            snapshot_debounce_worker_running: AtomicBool::new(false),
            last_subagent_reconcile: Mutex::new(Instant::now() - Duration::from_secs(10)),
            last_hook_activity: Mutex::new(Instant::now()),
            token_history_dirty: AtomicBool::new(false),
            transcript_cache: Mutex::new(TranscriptCache::default()),
            media_card_enabled: Mutex::new(load_media_card_enabled()),
            artwork_backdrop_enabled: Mutex::new(load_artwork_backdrop_enabled()),
            clipboard_history_limit: Mutex::new(load_clipboard_history_limit()),
            clipboard_history: Mutex::new(clipboard_history::load_history(
                load_clipboard_history_limit(),
            )),
            clipboard_history_enabled: Mutex::new(load_clipboard_history_enabled()),
            clipboard_auto_stage: Mutex::new(load_clipboard_auto_stage()),
            file_station: Mutex::new(file_station::load_history()),
            lyrics_enabled: Mutex::new(load_lyrics_enabled()),
            lyrics: Mutex::new(None),
            lyrics_track_key: Mutex::new(String::new()),
            approval_notice_mode: Mutex::new(load_approval_notice_mode()),
            notification_language: Mutex::new(load_notification_language()),
            preferred_monitor: Mutex::new(load_preferred_monitor_name()),
            global_shortcuts: Mutex::new(shortcuts::GlobalShortcutsState::default()),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            get_session_requests,
            get_session_transcript,
            get_session_chat,
            resolve_permission_request,
            resolve_permission_with_input,
            set_session_auto_approve,
            get_approval_rules,
            save_approval_rules,
            create_approval_rule_from_request,
            get_risk_guard_enabled,
            set_risk_guard_enabled,
            archive_request,
            archive_all_resolved,
            archive_session,
            pin_session,
            set_island_presentation,
            list_monitors,
            get_preferred_monitor,
            set_preferred_monitor,
            get_notch_metrics,
            set_ime_active,
            uses_micro_island,
            get_claude_hook_status,
            install_claude_hooks,
            uninstall_claude_hooks,
            remove_competing_claude_hooks,
            get_codex_hook_status,
            install_codex_hooks,
            uninstall_codex_hooks,
            get_zcode_hook_status,
            install_zcode_hooks,
            uninstall_zcode_hooks,
            get_gemini_hook_status,
            install_gemini_hooks,
            uninstall_gemini_hooks,
            get_opencode_hook_status,
            install_opencode_hooks,
            uninstall_opencode_hooks,
            get_cursor_hook_status,
            install_cursor_hooks,
            uninstall_cursor_hooks,
            get_session_retention,
            set_session_retention,
            get_subagent_retention,
            set_subagent_retention,
            get_now_playing,
            send_media_command,
            get_media_card_enabled,
            set_media_card_enabled,
            get_approval_notice_mode,
            set_approval_notice_mode,
            set_notification_language,
            get_global_shortcut_config,
            set_global_shortcut_config,
            get_artwork_backdrop_enabled,
            set_artwork_backdrop_enabled,
            get_lyrics_enabled,
            set_lyrics_enabled,
            get_current_lyrics,
            get_clipboard_history,
            copy_clipboard_entry,
            clear_clipboard_history,
            get_approval_history,
            export_approval_history,
            clear_approval_history,
            reveal_path,
            get_clipboard_history_enabled,
            set_clipboard_history_enabled,
            get_clipboard_history_limit,
            set_clipboard_history_limit,
            get_clipboard_auto_stage,
            set_clipboard_auto_stage,
            toggle_clipboard_favorite,
            get_clipboard_entry_thumbnail,
            get_staged_files,
            stage_files,
            remove_staged_file,
            clear_staged_files,
            copy_staged_files_to_clipboard,
            copy_staged_paths_to_clipboard,
            stage_clipboard_entries,
            begin_staged_files_drag,
            archive_subagent,
            archive_completed_subagents,
            get_token_history,
            get_pricing,
            set_model_rate,
            reset_model_rate,
            hide_model,
            unhide_model,
            refresh_pricing,
            open_in_terminal,
            open_agent_app,
            focus_claude_app,
            open_url,
            is_autostart_enabled,
            set_autostart_enabled,
            quit_atoll,
            deactivate_atoll,
            capture::capture_provide_screenshot
        ])
        .setup(|app| {
            #[cfg(desktop)]
            {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
                app.handle().plugin(tauri_plugin_process::init())?;
                app.handle().plugin(tauri_plugin_notification::init())?;
                app.handle()
                    .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
            }

            if !platform::setup_app(app) {
                std::process::exit(0);
            }

            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            // Import permission requests spooled by hooks while Atoll was not
            // running (they resolved in the agents' own UIs, so they land as
            // answered_elsewhere history rows). Off the setup path: the DB
            // write must not delay window creation.
            std::thread::spawn(|| {
                let imported = approval_spool::import_spooled_requests();
                if imported > 0 {
                    eprintln!("Atoll approval spool: imported {imported} offline request(s)");
                }
            });
            #[cfg(desktop)]
            shortcuts::startup(app.handle());
            start_island_hover_monitor(app.handle().clone());
            platform::start_activation_observer(app.handle().clone());
            if let Some(window) = app.get_webview_window("main") {
                let reveal_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::Focused(true) = event {
                        handle_island_reveal_request(&reveal_handle);
                    }
                });
            }
            {
                let state = app.state::<AppState>();
                let retention = load_persisted_retention_secs();
                *lock_state(&state.session_retention_secs) = retention;
                let sub_retention = load_persisted_subagent_retention_secs();
                *lock_state(&state.subagent_retention_secs) = sub_retention;
            }
            start_auto_archive_timer(app.handle().clone());
            start_token_refresh_timer(app.handle().clone());
            start_token_history_writer(app.handle().clone());
            start_media_monitor(app.handle().clone());
            start_clipboard_monitor(app.handle().clone());
            start_lyrics_monitor(app.handle().clone());
            start_initial_maintenance(app.handle().clone());
            std::thread::spawn(|| {
                pricing::maybe_refresh_pricing_catalog_on_startup();
            });

            if capture::enabled() {
                let state = app.state::<AppState>();
                capture::seed_approval_demo(app.handle(), &state);
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                #[cfg(not(target_os = "windows"))]
                {
                    let _ = window.show();
                    // Apply island style AFTER show() so the window number is
                    // assigned and the NSPanel promotion takes effect on macOS.
                }
                platform::apply_island_window_style(&window);
                eprintln!("[Atoll] step: island style applied, now applying mode...");
                let initial_mode = if cfg!(target_os = "windows") {
                    IslandWindowMode::Micro
                } else {
                    IslandWindowMode::Compact
                };
                let state = app.state::<AppState>();
                let preferred_monitor = lock_state(&state.preferred_monitor).clone();
                if let Ok(Some(home)) = apply_island_window_mode(
                    &window,
                    preferred_monitor.as_deref(),
                    initial_mode,
                    COMPACT_WINDOW_WIDTH,
                    0.0,
                    false,
                    false,
                    false,
                ) {
                    eprintln!("[Atoll] step: island window mode applied");
                    if let Ok(mut home_bounds) = state.home_bounds.lock() {
                        *home_bounds = Some(home);
                    };
                    if let Ok(mut notch_metrics) = state.notch_metrics.lock() {
                        *notch_metrics = home.notch;
                    };
                }
                #[cfg(target_os = "windows")]
                platform::show_island_on_top(&window);
                eprintln!("[Atoll] step: setup window complete");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atoll");
}
