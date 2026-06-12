use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::utils::config::Color;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalSize, State};

mod hook_bridge;

const COMPACT_WINDOW_WIDTH: f64 = 132.0;
const COMPACT_WINDOW_HEIGHT: f64 = 28.0;
const EXPANDED_WINDOW_WIDTH: f64 = 560.0;
const EXPANDED_WINDOW_HEIGHT: f64 = 320.0;
const WINDOW_ANIMATION_DURATION: Duration = Duration::from_millis(420);
const WINDOW_ANIMATION_FRAME: Duration = Duration::from_micros(16_667);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PermissionRequest {
    id: String,
    tool_use_id: Option<String>,
    agent: AgentKind,
    session: String,
    command: String,
    detail: String,
    cwd: String,
    requested_at: String,
    status: PermissionStatus,
    #[serde(default)]
    archived: bool,
    #[serde(default)]
    supports_always: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IslandSnapshot {
    online: bool,
    pending_count: usize,
    archived_count: usize,
    active_request: Option<PermissionRequest>,
    recent: Vec<PermissionRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum PermissionStatus {
    Pending,
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Decision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum IslandWindowMode {
    Compact,
    Expanded,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct IslandHoverChanged {
    hovering: bool,
}

struct AppState {
    requests: Mutex<Vec<PermissionRequest>>,
    hook_waiters: Mutex<HashMap<String, SyncSender<Decision>>>,
    auto_approve_sessions: Mutex<HashSet<String>>,
    presentation_generation: Arc<AtomicU64>,
    home_bounds: Mutex<Option<HomeWindowBounds>>,
}

#[derive(Debug, Clone, Copy)]
struct HomeWindowBounds {
    position: LogicalPosition<f64>,
    compact_size: PhysicalSize<u32>,
    monitor_top_y: f64,
    #[cfg(target_os = "macos")]
    screen_geometry: Option<MacosScreenGeometry>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct MacosScreenGeometry {
    origin_y: f64,
    height: f64,
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> IslandSnapshot {
    snapshot_from(&state.requests.lock().expect("state mutex poisoned"))
}

#[tauri::command]
fn resolve_permission_request(
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

    let waiter = state
        .hook_waiters
        .lock()
        .map_err(|error| error.to_string())?
        .remove(&id);
    if let Some(waiter) = waiter {
        let _ = waiter.send(decision);
    }

    let snapshot = snapshot_from(&requests);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
async fn set_island_presentation(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: IslandWindowMode,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    let generation = state.presentation_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let presentation_generation = Arc::clone(&state.presentation_generation);
    let home_bounds = *state
        .home_bounds
        .lock()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || {
        animate_island_window_mode(
            &window,
            mode,
            generation,
            &presentation_generation,
            home_bounds,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn set_session_auto_approve(
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
fn archive_request(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    if let Some(request) = requests.iter_mut().find(|r| r.id == id) {
        request.archived = true;
    }
    let snapshot = snapshot_from(&requests);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn archive_all_resolved(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<IslandSnapshot, String> {
    let mut requests = state.requests.lock().map_err(|error| error.to_string())?;
    for request in requests.iter_mut() {
        if request.status != PermissionStatus::Pending && !request.archived {
            request.archived = true;
        }
    }
    let snapshot = snapshot_from(&requests);
    app.emit("snapshot-changed", &snapshot)
        .map_err(|error| error.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn quit_atoll(app: AppHandle) {
    exit_atoll(&app);
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            requests: Mutex::new(Vec::new()),
            hook_waiters: Mutex::new(HashMap::new()),
            auto_approve_sessions: Mutex::new(HashSet::new()),
            presentation_generation: Arc::new(AtomicU64::new(0)),
            home_bounds: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            resolve_permission_request,
            set_session_auto_approve,
            archive_request,
            archive_all_resolved,
            set_island_presentation,
            quit_atoll
        ])
        .setup(|app| {
            build_tray(app.handle())?;
            hook_bridge::start_server(app.handle().clone());
            start_island_hover_monitor(app.handle().clone());
            start_auto_archive_timer(app.handle().clone());

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.set_shadow(false);
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
                apply_macos_island_window_style(&window);
                let _ = window.show();
                if let Ok(Some(home)) = apply_island_window_mode(&window, IslandWindowMode::Compact)
                {
                    let state = app.state::<AppState>();
                    if let Ok(mut home_bounds) = state.home_bounds.lock() {
                        *home_bounds = Some(home);
                    };
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atoll");
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let [(show_id, show_label), (quit_id, quit_label)] = tray_menu_entries();
    let show = MenuItem::with_id(app, show_id, show_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, quit_id, quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => exit_atoll(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            match event {
                TrayIconEvent::Enter { .. } => {
                    show_main_window(&app);
                }
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    show_main_window(&app);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}

fn tray_menu_entries() -> [(&'static str, &'static str); 2] {
    [("show", "Show Atoll"), ("quit", "Quit")]
}

const AUTO_ARCHIVE_INTERVAL: Duration = Duration::from_secs(10);
const AUTO_ARCHIVE_AGE: Duration = Duration::from_secs(60);

fn start_auto_archive_timer(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(AUTO_ARCHIVE_INTERVAL);

        let state = app.state::<AppState>();
        let changed = {
            let Ok(mut requests) = state.requests.lock() else {
                continue;
            };

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut changed = false;
            for request in requests.iter_mut() {
                if request.archived || request.status == PermissionStatus::Pending {
                    continue;
                }
                let requested_at_secs = parse_iso_timestamp_secs(&request.requested_at);
                if now.saturating_sub(requested_at_secs) >= AUTO_ARCHIVE_AGE.as_secs() {
                    request.archived = true;
                    changed = true;
                }
            }

            if changed {
                let snapshot = snapshot_from(&requests);
                let _ = app.emit("snapshot-changed", &snapshot);
            }
            changed
        };
        let _ = changed;
    });
}

fn parse_iso_timestamp_secs(iso: &str) -> u64 {
    // Parse "YYYY-MM-DDTHH:MM:SSZ" to unix seconds (simplified)
    let parts: Vec<&str> = iso.split('T').collect();
    if parts.len() != 2 {
        return 0;
    }
    let date_parts: Vec<u64> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    let time_str = parts[1].trim_end_matches('Z');
    let time_parts: Vec<u64> = time_str.split(':').filter_map(|s| s.parse().ok()).collect();

    if date_parts.len() != 3 || time_parts.len() < 3 {
        return 0;
    }

    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);

    // Approximate days-from-epoch calculation
    let mut days: u64 = 0;
    for y in 1970..year {
        days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    if month >= 1 && month <= 12 {
        days += month_days[(month - 1) as usize];
        if month > 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            days += 1;
        }
    }
    days += day.saturating_sub(1);

    days * 86400 + hour * 3600 + min * 60 + sec
}

fn exit_atoll(app: &AppHandle) {
    app.cleanup_before_exit();
    std::process::exit(0);
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit("island-open-requested", ());
    }
}

fn start_island_hover_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_hovering = false;

        loop {
            thread::sleep(Duration::from_millis(80));

            let hovering = app
                .get_webview_window("main")
                .and_then(|window| is_cursor_over_window(&window).ok())
                .unwrap_or(false);

            if hovering == last_hovering {
                continue;
            }

            last_hovering = hovering;
            let _ = app.emit("island-hover-changed", IslandHoverChanged { hovering });
        }
    });
}

fn is_cursor_over_window(window: &tauri::WebviewWindow) -> tauri::Result<bool> {
    if !window.is_visible()? {
        return Ok(false);
    }

    let cursor = window.cursor_position()?;
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let padding = 8.0;

    let left = position.x as f64 - padding;
    let top = position.y as f64 - padding;
    let right = position.x as f64 + size.width as f64 + padding;
    let bottom = position.y as f64 + size.height as f64 + padding;

    Ok(cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom)
}

fn apply_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
) -> tauri::Result<Option<HomeWindowBounds>> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(None);
    };

    apply_macos_island_window_style(window);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    window.set_size(island_window_logical_size(mode))?;
    let _ = window.set_ignore_cursor_events(matches!(mode, IslandWindowMode::Compact));

    let scale_factor = monitor.scale_factor();
    let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
    let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
    let window_size = island_window_physical_size(mode, monitor.scale_factor());
    let logical_window_size = window_size.to_logical::<f64>(scale_factor);
    let centered_x = monitor_position.x + (monitor_size.width - logical_window_size.width) / 2.0;
    let mut centered_y = island_top_y(window, monitor_position.y, monitor.work_area().position.y);
    centered_y += macos_camera_housing_top_clearance(
        window,
        monitor_position.x,
        monitor_size.width,
        centered_x,
        logical_window_size.width,
    );
    let position = LogicalPosition::new(centered_x, centered_y);
    let home = HomeWindowBounds {
        position,
        compact_size: island_window_physical_size(
            IslandWindowMode::Compact,
            monitor.scale_factor(),
        ),
        monitor_top_y: monitor_position.y,
        #[cfg(target_os = "macos")]
        screen_geometry: macos_screen_geometry(window, monitor_position.x, monitor_size.width),
    };

    set_island_window_frame_now(window, position, window_size, scale_factor, home)?;
    Ok(Some(home))
}

fn animate_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    generation: u64,
    presentation_generation: &AtomicU64,
    home_bounds: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    if matches!(mode, IslandWindowMode::Expanded) {
        let _ = window.set_ignore_cursor_events(false);
    }

    let scale_factor = home_bounds
        .map(|home| home.compact_size.width as f64 / COMPACT_WINDOW_WIDTH)
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));
    let start_position = window.outer_position()?.to_logical::<f64>(scale_factor);
    let start_size = window.outer_size()?;
    let target_size = island_window_physical_size(mode, scale_factor);
    let target_logical_size = target_size.to_logical::<f64>(scale_factor);
    let (target_x, target_y) = home_bounds
        .map(|home| {
            (
                home.position.x + (COMPACT_WINDOW_WIDTH - target_logical_size.width) / 2.0,
                home.position.y,
            )
        })
        .unwrap_or_else(|| {
            (
                start_position.x
                    + (start_size.width as f64 / scale_factor - target_logical_size.width) / 2.0,
                start_position.y,
            )
        });
    let started_at = Instant::now();
    let mut next_frame_at = started_at;

    loop {
        if presentation_generation.load(Ordering::SeqCst) != generation {
            return Ok(());
        }

        let progress =
            (started_at.elapsed().as_secs_f64() / WINDOW_ANIMATION_DURATION.as_secs_f64()).min(1.0);
        let eased = ease_out_cubic(progress);
        let size = PhysicalSize::new(
            interpolate_u32(start_size.width, target_size.width, eased),
            interpolate_u32(start_size.height, target_size.height, eased),
        );
        let position = LogicalPosition::new(
            interpolate_f64(start_position.x, target_x, eased),
            interpolate_f64(start_position.y, target_y, eased),
        );

        set_island_window_frame(window, position, size, scale_factor, home_bounds)?;

        if progress >= 1.0 {
            break;
        }

        next_frame_at += WINDOW_ANIMATION_FRAME;
        if let Some(delay) = next_frame_at.checked_duration_since(Instant::now()) {
            thread::sleep(delay);
        }
    }

    let _ = window.set_ignore_cursor_events(matches!(mode, IslandWindowMode::Compact));
    Ok(())
}

fn ease_out_cubic(progress: f64) -> f64 {
    1.0 - (1.0 - progress).powi(3)
}

fn interpolate_u32(start: u32, end: u32, progress: f64) -> u32 {
    (start as f64 + (end as f64 - start as f64) * progress).round() as u32
}

fn interpolate_f64(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}

fn island_window_logical_size(mode: IslandWindowMode) -> LogicalSize<f64> {
    match mode {
        IslandWindowMode::Compact => LogicalSize::new(COMPACT_WINDOW_WIDTH, COMPACT_WINDOW_HEIGHT),
        IslandWindowMode::Expanded => {
            LogicalSize::new(EXPANDED_WINDOW_WIDTH, EXPANDED_WINDOW_HEIGHT)
        }
    }
}

fn island_window_physical_size(mode: IslandWindowMode, scale_factor: f64) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(mode);

    PhysicalSize::new(
        (logical_size.width * scale_factor).round() as u32,
        (logical_size.height * scale_factor).round() as u32,
    )
}

fn island_top_y(window: &tauri::WebviewWindow, monitor_y: f64, work_area_y: i32) -> f64 {
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let work_area_y = work_area_y as f64 / scale_factor;
    let visible_offset = (work_area_y - monitor_y).max(0.0);
    let menu_offset = macos_menu_bar_offset_physical(window).unwrap_or(0) as f64 / scale_factor;

    island_top_y_from_offsets(monitor_y, visible_offset, menu_offset)
}

fn island_top_y_from_offsets(monitor_y: f64, visible_offset: f64, menu_offset: f64) -> f64 {
    monitor_y - (menu_offset.max(0.0) - visible_offset.max(0.0)).max(0.0)
}

fn has_camera_housing(frame_width: f64, aux_left_width: f64, aux_right_width: f64) -> bool {
    aux_left_width + aux_right_width < frame_width - 1.0
}

fn island_overlaps_camera_housing(
    island_x: f64,
    island_width: f64,
    housing_left: f64,
    housing_right: f64,
) -> bool {
    let island_right = island_x + island_width;
    island_right > housing_left && island_x < housing_right
}

fn camera_housing_top_clearance(
    has_camera_housing: bool,
    island_overlaps_housing: bool,
    safe_area_top: f64,
) -> f64 {
    if has_camera_housing && island_overlaps_housing {
        safe_area_top.max(0.0)
    } else {
        0.0
    }
}

#[cfg(target_os = "macos")]
fn with_nsscreen_for_monitor<R>(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
    inspect: impl FnOnce(&objc2_app_kit::NSScreen) -> R,
) -> Option<R> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSScreen, NSWindow};

    if let Some(main_thread_marker) = MainThreadMarker::new() {
        let screens = NSScreen::screens(main_thread_marker);
        if let Some(screen) = screens.iter().find(|screen| {
            let frame = screen.frame();
            (frame.origin.x - monitor_x).abs() < 1.0
                && (frame.size.width - monitor_width).abs() < 1.0
        }) {
            return Some(inspect(&screen));
        }
    }

    let ns_window = window.ns_window().ok()?;
    if ns_window.is_null() {
        return None;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        ns_window.screen().map(|screen| inspect(&screen))
    }
}

#[cfg(target_os = "macos")]
fn macos_camera_housing_top_clearance(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
    island_x: f64,
    island_width: f64,
) -> f64 {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let frame = screen.frame();
        let aux_left = screen.auxiliaryTopLeftArea();
        let aux_right = screen.auxiliaryTopRightArea();
        let has_housing = has_camera_housing(
            frame.size.width,
            aux_left.size.width,
            aux_right.size.width,
        );
        let housing_left = aux_left.origin.x + aux_left.size.width;
        let housing_right = aux_right.origin.x;
        let overlaps = island_overlaps_camera_housing(
            island_x,
            island_width,
            housing_left,
            housing_right,
        );

        camera_housing_top_clearance(has_housing, overlaps, screen.safeAreaInsets().top)
    })
    .unwrap_or(0.0)
}

#[cfg(not(target_os = "macos"))]
fn macos_camera_housing_top_clearance(
    _window: &tauri::WebviewWindow,
    _monitor_x: f64,
    _monitor_width: f64,
    _island_x: f64,
    _island_width: f64,
) -> f64 {
    0.0
}

#[cfg(target_os = "macos")]
fn macos_screen_geometry(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> Option<MacosScreenGeometry> {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let frame = screen.frame();
        MacosScreenGeometry {
            origin_y: frame.origin.y,
            height: frame.size.height,
        }
    })
}

#[cfg(target_os = "macos")]
fn set_island_window_frame_now(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: HomeWindowBounds,
) -> tauri::Result<()> {
    use objc2_app_kit::NSWindow;

    let Some(screen_geometry) = home.screen_geometry else {
        window.set_size(size)?;
        return window.set_position(position);
    };
    let ns_window = window.ns_window()?;
    if ns_window.is_null() {
        return Ok(());
    }

    let logical_size = size.to_logical::<f64>(scale_factor);
    let origin_y = appkit_window_origin_y(
        screen_geometry.origin_y,
        screen_geometry.height,
        logical_size.height,
        position.y,
        home.monitor_top_y,
    );

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        let mut frame = ns_window.frame();
        frame.origin.x = position.x;
        frame.origin.y = origin_y;
        frame.size.width = logical_size.width;
        frame.size.height = logical_size.height;
        ns_window.setFrame_display(frame, true);
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_island_window_frame_now(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    _scale_factor: f64,
    _home: HomeWindowBounds,
) -> tauri::Result<()> {
    window.set_size(size)?;
    window.set_position(position)
}

#[cfg(target_os = "macos")]
fn set_island_window_frame(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    let Some(home) = home else {
        window.set_size(size)?;
        return window.set_position(position);
    };

    let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(0);
    let window = window.clone();
    let frame_window = window.clone();
    window.run_on_main_thread(move || {
        let _ = set_island_window_frame_now(&frame_window, position, size, scale_factor, home);
        let _ = completed_tx.send(());
    })?;
    let _ = completed_rx.recv();

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_island_window_frame(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    _scale_factor: f64,
    _home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    window.set_size(size)?;
    window.set_position(position)
}

fn appkit_window_origin_y(
    screen_origin_y: f64,
    screen_height: f64,
    window_height: f64,
    desired_top_y: f64,
    monitor_top_y: f64,
) -> f64 {
    screen_origin_y + screen_height - (desired_top_y - monitor_top_y) - window_height
}

#[cfg(target_os = "macos")]
fn apply_macos_island_window_style(window: &tauri::WebviewWindow) {
    use objc2_app_kit::{
        NSColor, NSMainMenuWindowLevel, NSWindow, NSWindowAnimationBehavior,
        NSWindowCollectionBehavior,
    };

    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    if ns_window.is_null() {
        return;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        apply_macos_unconstrained_window_class(ns_window);
        let clear = NSColor::clearColor();
        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&clear));
        ns_window.setHasShadow(false);
        ns_window.setMovable(false);
        ns_window.setMovableByWindowBackground(false);
        ns_window.setCanHide(false);
        ns_window.setAnimationBehavior(NSWindowAnimationBehavior::None);
        ns_window.setAllowsToolTipsWhenApplicationIsInactive(true);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.setLevel(NSMainMenuWindowLevel + 3);
    }
}

#[cfg(target_os = "macos")]
fn apply_macos_unconstrained_window_class(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::{NSScreen, NSWindow};
    use objc2_foundation::NSRect;

    extern "C-unwind" fn unconstrained_frame(
        _window: *mut NSWindow,
        _selector: Sel,
        frame: NSRect,
        _screen: *mut NSScreen,
    ) -> NSRect {
        frame
    }

    static WINDOW_CLASS_PATCHED: OnceLock<()> = OnceLock::new();
    WINDOW_CLASS_PATCHED.get_or_init(|| {
        let selector = sel!(constrainFrameRect:toScreen:);
        let class = ns_window.class();
        let method = class
            .instance_method(selector)
            .expect("NSWindow constrainFrameRect:toScreen: should exist");
        unsafe {
            let implementation: Imp = std::mem::transmute(
                unconstrained_frame
                    as extern "C-unwind" fn(*mut NSWindow, Sel, NSRect, *mut NSScreen) -> NSRect,
            );
            objc2::ffi::class_replaceMethod(
                class as *const AnyClass as *mut AnyClass,
                selector,
                implementation,
                objc2::ffi::method_getTypeEncoding(method),
            );
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn apply_macos_island_window_style(_window: &tauri::WebviewWindow) {}

#[cfg(target_os = "macos")]
fn macos_menu_bar_offset_physical(window: &tauri::WebviewWindow) -> Option<i32> {
    use objc2_app_kit::NSWindow;

    let ns_window = window.ns_window().ok()?;
    if ns_window.is_null() {
        return None;
    }

    unsafe {
        let ns_window = &*(ns_window.cast::<NSWindow>());
        let screen = ns_window.screen()?;
        let frame = screen.frame();
        let visible_frame = screen.visibleFrame();
        let scale = screen.backingScaleFactor();
        let offset = (frame.origin.y + frame.size.height)
            - (visible_frame.origin.y + visible_frame.size.height);

        Some((offset.max(0.0) * scale).round() as i32)
    }
}

#[cfg(not(target_os = "macos"))]
fn macos_menu_bar_offset_physical(_window: &tauri::WebviewWindow) -> Option<i32> {
    None
}

fn snapshot_from(requests: &[PermissionRequest]) -> IslandSnapshot {
    let visible: Vec<&PermissionRequest> = requests
        .iter()
        .filter(|request| !request.archived)
        .collect();
    let pending_count = visible
        .iter()
        .filter(|request| request.status == PermissionStatus::Pending)
        .count();
    let active_request = visible
        .iter()
        .find(|request| request.status == PermissionStatus::Pending)
        .cloned()
        .cloned();
    let archived_count = requests.iter().filter(|r| r.archived).count();

    IslandSnapshot {
        online: true,
        pending_count,
        archived_count,
        active_request,
        recent: visible.into_iter().take(12).cloned().collect(),
    }
}

fn iso_timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    format_unix_timestamp(duration.as_secs())
}

fn format_unix_timestamp(timestamp: u64) -> String {
    // Compact UTC formatter to avoid pulling in a full time crate for the MVP.
    const SECONDS_PER_DAY: u64 = 86_400;
    let days = timestamp / SECONDS_PER_DAY;
    let seconds_of_day = timestamp % SECONDS_PER_DAY;
    let (year, day_of_year) = civil_year_and_day(days);
    let (month, day) = month_and_day(year, day_of_year);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_year_and_day(days_since_epoch: u64) -> (i32, u64) {
    let mut year = 1970;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining_days < days_in_year {
            return (year, remaining_days);
        }

        remaining_days -= days_in_year;
        year += 1;
    }
}

fn month_and_day(year: i32, day_of_year: u64) -> (u64, u64) {
    let month_lengths = [
        31,
        if is_leap_year(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut remaining = day_of_year;

    for (index, days) in month_lengths.iter().enumerate() {
        if remaining < *days {
            return (index as u64 + 1, remaining + 1);
        }
        remaining -= days;
    }

    (12, 31)
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[cfg(test)]
mod core_tests {
    use super::*;

    #[test]
    fn expanded_window_is_560_by_320() {
        let size = island_window_logical_size(IslandWindowMode::Expanded);

        assert_eq!(size, LogicalSize::new(560.0, 320.0));
    }

    #[test]
    fn tray_contains_only_show_and_quit() {
        assert_eq!(
            tray_menu_entries(),
            [("show", "Show Atoll"), ("quit", "Quit")]
        );
    }

    #[test]
    fn window_animation_interpolates_to_exact_endpoints() {
        assert_eq!(interpolate_u32(132, 560, ease_out_cubic(0.0)), 132);
        assert_eq!(interpolate_u32(132, 560, ease_out_cubic(1.0)), 560);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(0.0)), 100.0);
        assert_eq!(interpolate_f64(100.0, -20.0, ease_out_cubic(1.0)), -20.0);
    }

    #[test]
    fn island_uses_the_historical_menu_bar_anchor() {
        assert_eq!(island_top_y_from_offsets(0.0, 30.0, 30.0), 0.0);
        assert_eq!(island_top_y_from_offsets(-180.0, 30.0, 30.0), -180.0);
    }

    #[test]
    fn camera_housing_clearance_applies_only_when_centered_island_overlaps() {
        assert_eq!(camera_housing_top_clearance(true, true, 38.0), 38.0);
        assert_eq!(camera_housing_top_clearance(true, false, 38.0), 0.0);
        assert_eq!(camera_housing_top_clearance(false, true, 38.0), 0.0);
        assert_eq!(camera_housing_top_clearance(true, true, -4.0), 0.0);
    }

    #[test]
    fn camera_housing_is_detected_from_auxiliary_top_areas() {
        assert!(has_camera_housing(1512.0, 700.0, 700.0));
        assert!(!has_camera_housing(1512.0, 756.0, 756.0));
    }

    #[test]
    fn centered_island_overlaps_camera_housing() {
        let housing_left = 700.0;
        let housing_right = 812.0;
        assert!(island_overlaps_camera_housing(
            690.0,
            132.0,
            housing_left,
            housing_right
        ));
        assert!(!island_overlaps_camera_housing(
            500.0,
            132.0,
            housing_left,
            housing_right
        ));
    }

    #[test]
    fn appkit_frame_places_the_window_at_the_screen_top() {
        assert_eq!(appkit_window_origin_y(0.0, 1260.0, 28.0, 0.0, 0.0), 1232.0);
        assert_eq!(appkit_window_origin_y(0.0, 1260.0, 320.0, 0.0, 0.0), 940.0);
    }
}

#[cfg(test)]
mod hook_bridge_tests {
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

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

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
        }];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

        assert_eq!(completed_id.as_deref(), Some("request-123"));
        assert_eq!(requests[0].status, crate::PermissionStatus::Approved);
    }

    #[test]
    fn leaves_ambiguous_pending_requests_when_post_tool_use_has_no_match_fields() {
        let mut requests = vec![
            crate::PermissionRequest {
                id: "request-1".into(),
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
            },
            crate::PermissionRequest {
                id: "request-2".into(),
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
            },
        ];

        let payload = json!({
            "session_id": "session-123",
            "hook_event_name": "PostToolUse"
        });

        let completed_id =
            crate::hook_bridge::mark_matching_pending_request_complete(&mut requests, &payload);

        assert_eq!(completed_id, None);
        assert_eq!(requests[0].status, crate::PermissionStatus::Pending);
        assert_eq!(requests[1].status, crate::PermissionStatus::Pending);
    }

    #[test]
    fn encodes_hook_decision_for_claude_hook_event() {
        let approved = crate::hook_bridge::claude_hook_response(
            "PermissionRequest",
            crate::Decision::Approved,
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

        let denied =
            crate::hook_bridge::claude_hook_response("PermissionRequest", crate::Decision::Denied);
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
    fn encodes_hook_decision_for_claude_pre_tool_use() {
        let approved =
            crate::hook_bridge::claude_hook_response("PreToolUse", crate::Decision::Approved);
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
    fn encodes_permission_request_ask_as_empty_response() {
        let ask =
            crate::hook_bridge::claude_hook_ask_response("PermissionRequest", "Atoll unavailable");

        assert_eq!(ask, json!({}));
    }

    #[test]
    fn encodes_pre_tool_use_ask_response() {
        let ask = crate::hook_bridge::claude_hook_ask_response("PreToolUse", "Atoll unavailable");

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
}
