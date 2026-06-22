use std::process::Command;

use tauri::{AppHandle, LogicalPosition, Manager, Monitor, PhysicalSize, State, WebviewWindow};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO, MONITORINFOEXW};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, MonitorFromWindow, SetForegroundWindow, MONITOR_DEFAULTTONEAREST,
};

use crate::{AppState, HomeWindowBounds};

pub fn apply_island_window_style(window: &WebviewWindow) {
    let _ = window.set_always_on_top(true);
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
        let hwnd = HWND(raw as *mut _);
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
    let _ = window.run_on_main_thread(move || {
        let _ = window.show();
        let _ = window.set_always_on_top(true);
        if request_focus {
            remember_foreground_window(&app);
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

fn window_hwnd(window: &WebviewWindow) -> Option<HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(raw) => Some(HWND(raw.hwnd.get() as *mut _)),
        _ => None,
    }
}
