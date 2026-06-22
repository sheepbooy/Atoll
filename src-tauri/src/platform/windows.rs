use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::{AppHandle, LogicalPosition, Manager, Monitor, PhysicalSize, WebviewWindow};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow};

use crate::{AppState, HomeWindowBounds};

struct InstanceMutex(#[allow(dead_code)] HANDLE);

unsafe impl Send for InstanceMutex {}
unsafe impl Sync for InstanceMutex {}

static SINGLE_INSTANCE_MUTEX: OnceLock<InstanceMutex> = OnceLock::new();

/// When true, keep the island interactive even if the cursor is outside its bounds
/// (e.g. approval focus while the pointer is still over another app).
static FORCE_INTERACTIVE: AtomicBool = AtomicBool::new(false);
static CURSOR_CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Compact/dormant vs expanded — Windows only captures mouse input while expanded.
static IS_EXPANDED: AtomicBool = AtomicBool::new(false);

/// Dwell time before hover-to-expand on Windows compact/dormant. Keeps quick
/// passes through the top edge from stealing clicks or popping the island open.
pub const COMPACT_HOVER_EXPAND_DWELL_MS: u64 = 350;

/// Returns false when another Atoll instance already holds the global mutex.
pub fn ensure_single_instance() -> bool {
    let name: Vec<u16> = "Global\\com.atoll.agentisland\0"
        .encode_utf16()
        .collect();

    unsafe {
        let handle = match CreateMutexW(None, true, PCWSTR(name.as_ptr())) {
            Ok(handle) => handle,
            Err(error) => {
                eprintln!("Atoll single-instance mutex failed: {error}");
                return true;
            }
        };

        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("Atoll is already running.");
            return false;
        }

        let _ = SINGLE_INSTANCE_MUTEX.set(InstanceMutex(handle));
    }

    true
}

pub fn apply_island_window_style(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
    // Start pass-through by default; the hover monitor re-enables capture on demand.
    let _ = window.set_ignore_cursor_events(true);
    CURSOR_CAPTURE_ACTIVE.store(false, Ordering::Release);
}

/// Windows WebView2 does not pass clicks through transparent pixels the way macOS
/// WebKit does. Compact/dormant islands always pass clicks through because Windows
/// apps (browsers, etc.) extend under the top edge unlike the macOS menu bar.
/// Only the expanded panel captures input, and only while the cursor is over it.
pub fn set_island_cursor_events_ignored(window: &WebviewWindow, ignore: bool) {
    IS_EXPANDED.store(!ignore, Ordering::Release);
    if ignore {
        FORCE_INTERACTIVE.store(false, Ordering::Release);
        if CURSOR_CAPTURE_ACTIVE.swap(false, Ordering::AcqRel) {
            let _ = window.set_ignore_cursor_events(true);
        }
    }
}

pub fn is_island_expanded() -> bool {
    IS_EXPANDED.load(Ordering::Acquire)
}

pub fn sync_cursor_pass_through(window: &WebviewWindow, hovering: bool) {
    if hovering {
        FORCE_INTERACTIVE.store(false, Ordering::Release);
    }

    let should_capture = if IS_EXPANDED.load(Ordering::Acquire) {
        hovering || FORCE_INTERACTIVE.load(Ordering::Acquire)
    } else {
        false
    };

    let was_capturing = CURSOR_CAPTURE_ACTIVE.load(Ordering::Acquire);
    if was_capturing == should_capture {
        return;
    }
    CURSOR_CAPTURE_ACTIVE.store(should_capture, Ordering::Release);
    let _ = window.set_ignore_cursor_events(!should_capture);
}

fn set_force_interactive(window: &WebviewWindow, force: bool) {
    FORCE_INTERACTIVE.store(force, Ordering::Release);
    if force && !CURSOR_CAPTURE_ACTIVE.load(Ordering::Acquire) {
        CURSOR_CAPTURE_ACTIVE.store(true, Ordering::Release);
        let _ = window.set_ignore_cursor_events(false);
    }
}

pub fn monitor_work_area_top(window: &WebviewWindow, monitor: &Monitor) -> f64 {
    if let Some(top) = work_area_top_from_hwnd(window) {
        return top;
    }

    let scale_factor = monitor.scale_factor();
    monitor.position().to_logical::<f64>(scale_factor).y
}

pub fn set_island_window_frame_now(
    window: &WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    _scale_factor: f64,
    _home: HomeWindowBounds,
) -> tauri::Result<()> {
    window.set_size(size)?;
    window.set_position(position)
}

pub fn set_island_window_frame(
    window: &WebviewWindow,
    position: LogicalPosition<f64>,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    home: Option<HomeWindowBounds>,
) -> tauri::Result<()> {
    let Some(home) = home else {
        window.set_size(size)?;
        return window.set_position(position);
    };
    set_island_window_frame_now(window, position, size, scale_factor, home)
}

pub fn remember_foreground_window(app: &AppHandle) {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return;
    }

    if let Ok(mut guard) = app.state::<AppState>().previous_app_pid.lock() {
        *guard = Some(hwnd.0 as i64);
    }
}

pub fn restore_foreground_window(state: &AppState) {
    let previous = state
        .previous_app_pid
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    if let Some(raw) = previous {
        let hwnd = windows::Win32::Foundation::HWND(raw as *mut _);
        if !hwnd.0.is_null() {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
        }
    }
}

pub fn finish_show_for_approval(window: &WebviewWindow, app: &AppHandle, request_focus: bool) {
    let window = window.clone();
    let app = app.clone();
    let _ = window.clone().run_on_main_thread(move || {
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        if request_focus {
            remember_foreground_window(&app);
            set_force_interactive(&window, true);
            let _ = window.set_focus();
        }
    });
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if try_windows_terminal(cwd) {
        return Ok(());
    }

    Command::new("cmd")
        .args(["/C", "start", "", "cmd", "/k", &format!("cd /d \"{cwd}\"")])
        .spawn()
        .map_err(|error| format!("Failed to open terminal: {error}"))?;
    Ok(())
}

fn try_windows_terminal(cwd: &str) -> bool {
    Command::new("wt.exe")
        .args(["-w", "0", "nt", "-d", cwd])
        .spawn()
        .is_ok()
}

fn work_area_top_from_hwnd(window: &WebviewWindow) -> Option<f64> {
    let hwnd = window_hwnd(window)?;
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return None;
        }

        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if !GetMonitorInfoW(monitor, &mut info.monitorInfo).as_bool() {
            return None;
        }

        let scale_factor = window.scale_factor().ok()?;
        Some(info.monitorInfo.rcWork.top as f64 / scale_factor)
    }
}

fn window_hwnd(window: &WebviewWindow) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(raw) => Some(windows::Win32::Foundation::HWND(
            raw.hwnd.get() as *mut _,
        )),
        _ => None,
    }
}
