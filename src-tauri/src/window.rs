//! Island window management: presentation modes (micro/compact/dormant/
//! expanded), native window sizing and animation, hover monitoring, island
//! reveal/exit, and approval notifications.

use std::sync::atomic::Ordering;
use std::time::Duration;

use serde_json::json;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalSize, State};

use super::*;

#[tauri::command]
pub(crate) fn uses_micro_island() -> bool {
    cfg!(target_os = "windows")
}
#[tauri::command]
pub(crate) async fn set_island_presentation(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: IslandWindowMode,
    compact_width: Option<f64>,
    compact_left_width: Option<f64>,
    expanded_idle: Option<bool>,
    expanded_plan: Option<bool>,
    expanded_settings: Option<bool>,
    animate: Option<bool>,
    snap: Option<bool>,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if let Some(width) = compact_width {
        if should_persist_compact_width(mode) {
            let mut saved_width = state
                .compact_width
                .lock()
                .map_err(|error| error.to_string())?;
            *saved_width = sanitize_compact_width(width);
        }
    }

    if let Some(left_width) = compact_left_width {
        let mut saved_left = state
            .compact_left_width
            .lock()
            .map_err(|error| error.to_string())?;
        *saved_left = if left_width.is_finite() {
            left_width.max(0.0)
        } else {
            0.0
        };
    }

    // Reduced-motion users get the snap path: no per-frame window resizing,
    // the island jumps straight to its target presentation.
    let reduced_motion = platform::prefers_reduced_motion();
    if animate == Some(false) || reduced_motion {
        if snap == Some(true) || reduced_motion {
            let saved_compact_width = *state
                .compact_width
                .lock()
                .map_err(|error| error.to_string())?;
            let presentation_width =
                resolve_presentation_width(mode, compact_width, saved_compact_width);
            let compact_left_width = *state
                .compact_left_width
                .lock()
                .map_err(|error| error.to_string())?;
            let expanded_idle = expanded_idle.unwrap_or(false);
            let expanded_plan = expanded_plan.unwrap_or(false);
            let expanded_settings = expanded_settings.unwrap_or(false);
            // apply_island_window_mode touches AppKit; must run on the main thread.
            let (sync_tx, sync_rx) =
                std::sync::mpsc::sync_channel::<Result<Option<HomeWindowBounds>, String>>(0);
            let frame_window = window.clone();
            window
                .run_on_main_thread(move || {
                    let result = apply_island_window_mode(
                        &frame_window,
                        mode,
                        presentation_width,
                        compact_left_width,
                        expanded_idle,
                        expanded_plan,
                        expanded_settings,
                    )
                    .map_err(|error| error.to_string());
                    let _ = sync_tx.send(result);
                })
                .map_err(|error| error.to_string())?;
            let home = sync_rx
                .recv_timeout(Duration::from_secs(2))
                .map_err(|error| format!("main-thread presentation timed out: {error}"))??;
            if let Some(home) = home {
                if let Ok(mut home_bounds) = state.home_bounds.lock() {
                    *home_bounds = Some(home);
                }
            }
            // The presentation has been applied synchronously; let the frontend
            // settle its phase immediately instead of waiting on the 2s fallback.
            // Note: the outer `animate: false, snap: false` path (fire-and-forget
            // metrics update) intentionally skips this emit — callers wanting a
            // settled event must pass `snap: true`.
            let _ = app.emit("island-presentation-settled", mode);
        }
        return Ok(());
    }

    let generation = state.presentation_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let presentation_generation = Arc::clone(&state.presentation_generation);
    let saved_compact_width = *state
        .compact_width
        .lock()
        .map_err(|error| error.to_string())?;
    let presentation_width = resolve_presentation_width(mode, compact_width, saved_compact_width);
    let compact_left_width = *state
        .compact_left_width
        .lock()
        .map_err(|error| error.to_string())?;
    let home_bounds = *state
        .home_bounds
        .lock()
        .map_err(|error| error.to_string())?;
    let expanded_idle = expanded_idle.unwrap_or(false);
    let expanded_plan = expanded_plan.unwrap_or(false);
    let expanded_settings = expanded_settings.unwrap_or(false);

    tauri::async_runtime::spawn_blocking(move || {
        animate_island_window_mode(
            &window,
            mode,
            generation,
            &presentation_generation,
            home_bounds,
            presentation_width,
            compact_left_width,
            expanded_idle,
            expanded_plan,
            expanded_settings,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
pub(crate) fn exit_atoll(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _ = token_history::sync_today_to_history(&state);
    app.cleanup_before_exit();
    std::process::exit(0);
}
pub(crate) fn show_main_window_with_focus(
    app: &AppHandle,
    request_focus: bool,
    open_source: IslandOpenSource,
) {
    if let Some(window) = app.get_webview_window("main") {
        platform::finish_show_for_approval(&window, app, request_focus);
        let _ = app.emit("island-open-requested", open_source);
    }
}
pub(crate) fn show_main_window(app: &AppHandle) {
    show_main_window_with_focus(app, false, IslandOpenSource::Focus);
}
pub(crate) fn show_main_window_for_approval(app: &AppHandle) {
    show_main_window_with_focus(app, true, IslandOpenSource::Focus);
}
/// Surface the island window without expanding it or taking focus — used when
/// a new approval arrives in notify mode and must not interrupt the user.
pub(crate) fn show_island_quietly(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        platform::finish_show_for_approval(&window, app, false);
    }
}
pub(crate) fn approval_notice_is_notify(state: &AppState) -> bool {
    lock_state(&state.approval_notice_mode).as_str() == APPROVAL_NOTICE_NOTIFY
}
pub(crate) fn approval_notice_has_pending(state: &AppState) -> bool {
    state
        .requests
        .lock()
        .map(|requests| {
            requests
                .iter()
                .any(|request| request.status == PermissionStatus::Pending && !request.archived)
        })
        .unwrap_or(false)
}
/// Expand the island onto a pending approval when the user reveals Atoll —
/// clicking its notification, the dock icon, or the island itself — in notify
/// mode. No-op in interrupt mode, which already expands on request arrival.
pub(crate) fn handle_island_reveal_request(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !approval_notice_is_notify(&state) {
        return;
    }
    if !approval_notice_has_pending(&state) {
        return;
    }
    show_main_window(app);
}
/// Post the system notification for a new pending approval (notify mode).
pub(crate) fn send_approval_notification(
    app: &AppHandle,
    agent_label: &str,
    command: &str,
    cwd: &str,
) {
    use tauri_plugin_notification::NotificationExt;
    let language = {
        let state = app.state::<AppState>();
        let value = lock_state(&state.notification_language).clone();
        value
    };
    let (title, body) = approval_notification_copy(agent_label, command, cwd, &language);
    if let Err(error) = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .sound("default")
        .show()
    {
        eprintln!("[Atoll] approval notification failed: {error}");
    }
}
pub(crate) fn start_island_hover_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_hovering = false;
        let mut last_cursor_over = false;
        let mut last_client: Option<(f64, f64)> = None;
        #[cfg(target_os = "windows")]
        let mut compact_hover_since: Option<Instant> = None;

        loop {
            #[cfg(target_os = "windows")]
            thread::sleep(Duration::from_millis(16));
            #[cfg(not(target_os = "windows"))]
            thread::sleep(Duration::from_millis(80));

            let Some(window) = app.get_webview_window("main") else {
                continue;
            };

            let cursor_over_window = is_cursor_over_window(&window).unwrap_or(false);
            #[cfg(target_os = "windows")]
            let hovering = if platform::is_island_expanded() {
                cursor_over_window
            } else if cursor_over_window {
                let now = Instant::now();
                if compact_hover_since.is_none() {
                    compact_hover_since = Some(now);
                }
                compact_hover_since.is_some_and(|since| {
                    now.duration_since(since) >= platform::compact_hover_expand_dwell()
                })
            } else {
                compact_hover_since = None;
                false
            };
            #[cfg(not(target_os = "windows"))]
            let hovering = cursor_over_window;

            #[cfg(target_os = "windows")]
            platform::sync_cursor_pass_through(&window, cursor_over_window);
            let client = if hovering {
                cursor_client_point(&window)
            } else {
                None
            };

            let cursor_over_changed = cursor_over_window != last_cursor_over;
            let hover_changed = hovering != last_hovering;
            let client_changed = hovering && client != last_client;
            if cursor_over_changed || hover_changed || client_changed {
                let _ = app.emit(
                    "island-hover-changed",
                    IslandHoverChanged {
                        hovering,
                        cursor_over_window,
                        client_x: if hovering {
                            client.map(|(x, _)| x)
                        } else {
                            None
                        },
                        client_y: if hovering {
                            client.map(|(_, y)| y)
                        } else {
                            None
                        },
                    },
                );
                last_cursor_over = cursor_over_window;
                last_hovering = hovering;
                last_client = if hovering { client } else { None };
            }
        }
    });
}
pub(crate) fn apply_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    compact_width: f64,
    compact_left_width: f64,
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> tauri::Result<Option<HomeWindowBounds>> {
    let monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return Ok(None);
    };

    platform::apply_island_window_style(window);
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));

    let scale_factor = monitor.scale_factor();
    let monitor_position = monitor.position().to_logical::<f64>(scale_factor);
    let monitor_size = monitor.size().to_logical::<f64>(scale_factor);
    let monitor_top = platform::monitor_top_y(window, &monitor);
    let notch = platform::detect_notch_metrics(window, monitor_position.x, monitor_size.width);

    window.set_size(island_window_logical_size(
        mode,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    ))?;
    platform::set_island_cursor_events_ignored(window, is_collapsed_pass_through_mode(mode));

    let window_size = island_window_physical_size(
        mode,
        scale_factor,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    );
    let logical_window_size = window_size.to_logical::<f64>(scale_factor);
    let left_pane_width = if compact_left_width > 0.0 {
        compact_left_width
    } else {
        default_compact_left_pane_width(logical_window_size.width, notch)
    };
    let centered_x = compact_window_origin_x(
        monitor_position.x + monitor_size.width / 2.0,
        logical_window_size.width,
        notch,
        left_pane_width,
        mode,
    );
    // Keep the window flush with the physical top edge so the capsule overlaps
    // the notch / menu-bar band; the actual content is pushed below the notch
    // height inside the web view. On non-notched screens this is unchanged.
    let centered_y = monitor_top;
    let position = LogicalPosition::new(centered_x, centered_y);
    let home = HomeWindowBounds {
        position,
        // Fixed reference size for animation scale-factor recovery:
        //   compact_size.width / COMPACT_WINDOW_WIDTH == scale_factor
        // Computed directly so the FALLBACK_NOTCH_WIDTH minimum width floor
        // that island_window_logical_size applies does not distort the ratio.
        compact_size: PhysicalSize::new(
            (COMPACT_WINDOW_WIDTH * scale_factor).round() as u32,
            (COMPACT_WINDOW_HEIGHT * scale_factor).round() as u32,
        ),
        monitor_top_y: monitor_top,
        monitor_center_x: monitor_position.x + monitor_size.width / 2.0,
        notch,
        screen_geometry: platform::screen_geometry_for_monitor(
            window,
            monitor_position.x,
            monitor_size.width,
        ),
    };

    platform::set_island_window_frame_now(
        window,
        position,
        logical_window_size,
        scale_factor,
        home,
    )?;
    platform::ensure_island_on_top(window);
    Ok(Some(home))
}
pub(crate) fn animate_island_window_mode(
    window: &tauri::WebviewWindow,
    mode: IslandWindowMode,
    generation: u64,
    presentation_generation: &Arc<AtomicU64>,
    home_bounds: Option<HomeWindowBounds>,
    compact_width: f64,
    compact_left_width: f64,
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> tauri::Result<()> {
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    platform::ensure_island_on_top(window);
    if matches!(mode, IslandWindowMode::Expanded) {
        platform::set_island_cursor_events_ignored(window, false);
    } else {
        #[cfg(target_os = "windows")]
        platform::set_island_cursor_events_ignored(window, true);
    }

    let scale_factor = home_bounds
        .map(|home| home.compact_size.width as f64 / COMPACT_WINDOW_WIDTH)
        .unwrap_or_else(|| window.scale_factor().unwrap_or(1.0));
    let start_position = window.outer_position()?.to_logical::<f64>(scale_factor);
    let start_size = window.outer_size()?;
    let start_logical_size = start_size.to_logical::<f64>(scale_factor);
    let notch = home_bounds.map(|home| home.notch).unwrap_or_default();
    let target_size = island_window_physical_size(
        mode,
        scale_factor,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    );
    let target_logical_size = target_size.to_logical::<f64>(scale_factor);
    // Center the target window on the screen center.  Using monitor_center_x
    // instead of deriving from home.position avoids mis-centering when the
    // initial home position was set from a narrower mode (e.g. dormant 260px
    // vs compact 460px).
    let (target_x, target_y) = home_bounds
        .map(|home| {
            let left_pane_width = if compact_left_width > 0.0 {
                compact_left_width
            } else {
                default_compact_left_pane_width(target_logical_size.width, home.notch)
            };
            (
                compact_window_origin_x(
                    home.monitor_center_x,
                    target_logical_size.width,
                    home.notch,
                    left_pane_width,
                    mode,
                ),
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
    // Frame pacing from the live display: ProMotion panels animate at 120 Hz
    // instead of the 60 Hz default, halving per-frame visual stepping.
    let animation_frame = platform::display_animation_frame_interval(window);
    let started_at = Instant::now();
    let mut next_frame_at = started_at;

    loop {
        if presentation_generation.load(Ordering::SeqCst) != generation {
            return Ok(());
        }

        let progress =
            (started_at.elapsed().as_secs_f64() / WINDOW_ANIMATION_DURATION.as_secs_f64()).min(1.0);
        // Overshoot only when growing out of the menu-bar pill. Resizing between
        // already-expanded sizes (idle → settings/tokens) stays cubic so AppKit
        // never has to grow past the target and shrink back — that path felt
        // stuttery under heavy WebView content.
        let from_collapsed = start_logical_size.height <= COMPACT_WINDOW_HEIGHT + 1.0;
        let expanding = target_logical_size.width > start_logical_size.width
            || target_logical_size.height > start_logical_size.height;
        let eased = if expanding && from_collapsed {
            ease_out_spring(progress)
        } else {
            ease_out_cubic(progress)
        };
        // Interpolate in logical points so 2× Retina displays move in
        // fractional 0.5 pt steps instead of quantized whole physical pixels.
        let size = LogicalSize::new(
            interpolate_f64(start_logical_size.width, target_logical_size.width, eased),
            interpolate_f64(start_logical_size.height, target_logical_size.height, eased),
        );
        let position = LogicalPosition::new(
            interpolate_f64(start_position.x, target_x, eased),
            interpolate_f64(start_position.y, target_y, eased),
        );

        platform::set_island_window_frame(window, position, size, scale_factor, home_bounds)?;
        #[cfg(target_os = "macos")]
        {
            // Back-pressure AppKit frame delivery. Without this acknowledgement,
            // rapid expand/collapse cycles can enqueue dozens of stale frames.
            let (frame_tx, frame_rx) = std::sync::mpsc::sync_channel::<()>(0);
            window.run_on_main_thread(move || {
                let _ = frame_tx.send(());
            })?;
            // A busy main thread used to abort the animation mid-flight, which
            // froze the island at a half-interpolated size. Instead keep waiting
            // for the acknowledgement in slices (checking for cancellation) —
            // the absolute-time progress catches up once the main thread drains.
            loop {
                if presentation_generation.load(Ordering::SeqCst) != generation {
                    return Ok(());
                }
                if frame_rx.recv_timeout(Duration::from_millis(250)).is_ok() {
                    break;
                }
                if started_at.elapsed() >= WINDOW_ANIMATION_DURATION {
                    break;
                }
            }
        }

        if progress >= 1.0 {
            // #region agent log
            #[cfg(target_os = "windows")]
            if matches!(mode, IslandWindowMode::Expanded) {
                crate::debug_agent::log(
                    "H-B",
                    "lib.rs:animate_island_window_mode",
                    "expand animation finished",
                    serde_json::json!({
                        "mode": format!("{:?}", mode),
                        "targetW": target_size.width,
                        "targetH": target_size.height,
                        "alwaysOnTopAtEnd": window.is_always_on_top().unwrap_or(false),
                    }),
                );
            }
            // #endregion
            platform::ensure_island_on_top(window);
            break;
        }

        next_frame_at += animation_frame;
        if let Some(delay) = next_frame_at.checked_duration_since(Instant::now()) {
            thread::sleep(delay);
        }
    }

    let (sync_tx, sync_rx) = std::sync::mpsc::sync_channel::<()>(0);
    let _ = window.run_on_main_thread(move || {
        let _ = sync_tx.send(());
    });
    let _ = sync_rx.recv_timeout(Duration::from_secs(2));

    // The animation loop aborts without an epilogue once superseded, but a
    // newer presentation can still start while we were waiting for the main
    // thread above. Guard the cursor-event toggle and the settled emit: a
    // stale collapse re-enabling click pass-through over a fresh expand is
    // what leaves an expanded island that ignores every pointer event.
    if presentation_generation.load(Ordering::SeqCst) != generation {
        return Ok(());
    }
    platform::set_island_cursor_events_ignored_if_current(
        window,
        is_collapsed_pass_through_mode(mode),
        Arc::clone(presentation_generation),
        generation,
    );
    let _ = window.emit("island-presentation-settled", mode);
    Ok(())
}
pub(crate) fn interpolate_f64(start: f64, end: f64, progress: f64) -> f64 {
    start + (end - start) * progress
}
pub(crate) fn expanded_window_width(expanded_plan: bool, expanded_settings: bool) -> f64 {
    if expanded_plan {
        EXPANDED_PLAN_WINDOW_WIDTH
    } else if expanded_settings {
        EXPANDED_SETTINGS_WINDOW_WIDTH
    } else {
        EXPANDED_WINDOW_WIDTH
    }
}
pub(crate) fn expanded_window_height(
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> f64 {
    if expanded_plan {
        EXPANDED_PLAN_WINDOW_HEIGHT
    } else if expanded_settings {
        EXPANDED_SETTINGS_WINDOW_HEIGHT
    } else if expanded_idle {
        EXPANDED_IDLE_WINDOW_HEIGHT
    } else {
        EXPANDED_WINDOW_HEIGHT
    }
}
pub(crate) fn island_window_logical_size(
    mode: IslandWindowMode,
    compact_width: f64,
    notch: NotchMetrics,
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> LogicalSize<f64> {
    let compact_width = sanitize_compact_width(compact_width);
    let extra_top = if notch.has_notch {
        notch.height + NOTCH_COVER_PADDING
    } else {
        0.0
    };
    let min_notch_width = if notch.has_notch { notch.width } else { 0.0 };
    match mode {
        // Windows-only super-collapsed strip; keeps a minimal top-edge footprint
        // so full-screen apps stay clickable underneath.
        IslandWindowMode::Micro => {
            let w = sanitize_micro_width(compact_width);
            LogicalSize::new(w, MICRO_WINDOW_HEIGHT)
        }
        // Dormant sits within the menu-bar band. On notched displays the pill
        // spans the notch plus padding on each side; logo is left-aligned inside
        // the pill so it stays in the left wing, not under the camera housing.
        IslandWindowMode::Dormant => {
            let reference_notch = if notch.has_notch {
                notch.width
            } else {
                FALLBACK_NOTCH_WIDTH
            };
            let w = reference_notch + 2.0 * DORMANT_NOTCH_PADDING;
            LogicalSize::new(w, DORMANT_WINDOW_HEIGHT)
        }
        IslandWindowMode::Compact => {
            // Compact sits in the menu-bar band (same as dormant) — no extra_top.
            // On notched displays the capsule must be at least as wide as the
            // camera housing so it visually fuses with it (Dynamic-Island style).
            let w = if notch.has_notch {
                compact_width.max(notch.width)
            } else {
                compact_width
            };
            LogicalSize::new(w, COMPACT_WINDOW_HEIGHT)
        }
        IslandWindowMode::Expanded => {
            let w = expanded_window_width(expanded_plan, expanded_settings).max(min_notch_width);
            LogicalSize::new(
                w,
                expanded_window_height(expanded_idle, expanded_plan, expanded_settings) + extra_top,
            )
        }
    }
}
pub(crate) fn island_window_physical_size(
    mode: IslandWindowMode,
    scale_factor: f64,
    compact_width: f64,
    notch: NotchMetrics,
    expanded_idle: bool,
    expanded_plan: bool,
    expanded_settings: bool,
) -> PhysicalSize<u32> {
    let logical_size = island_window_logical_size(
        mode,
        compact_width,
        notch,
        expanded_idle,
        expanded_plan,
        expanded_settings,
    );

    PhysicalSize::new(
        (logical_size.width * scale_factor).round() as u32,
        (logical_size.height * scale_factor).round() as u32,
    )
}
pub(crate) fn sanitize_compact_width(width: f64) -> f64 {
    if !width.is_finite() {
        return COMPACT_WINDOW_WIDTH;
    }
    width.clamp(MIN_COMPACT_WINDOW_WIDTH, EXPANDED_WINDOW_WIDTH)
}
pub(crate) fn sanitize_micro_width(width: f64) -> f64 {
    if !width.is_finite() {
        return MICRO_WINDOW_WIDTH;
    }
    width.clamp(MICRO_WINDOW_WIDTH, EXPANDED_WINDOW_WIDTH)
}
pub(crate) fn should_persist_compact_width(mode: IslandWindowMode) -> bool {
    !matches!(mode, IslandWindowMode::Micro)
}
pub(crate) fn resolve_presentation_width(
    mode: IslandWindowMode,
    width_param: Option<f64>,
    saved_compact_width: f64,
) -> f64 {
    if matches!(mode, IslandWindowMode::Micro) {
        return width_param
            .map(sanitize_micro_width)
            .unwrap_or(MICRO_WINDOW_WIDTH);
    }
    width_param
        .map(sanitize_compact_width)
        .unwrap_or(saved_compact_width)
}
