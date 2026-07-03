use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

use tauri::{AppHandle, LogicalPosition, Manager, Monitor, PhysicalSize, WebviewWindow};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow, SetWindowPos,
    HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

use super::SessionHost;
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
    let name: Vec<u16> = "Global\\com.atoll.agentisland\0".encode_utf16().collect();

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

/// Re-assert topmost on Windows without moving or resizing the window. Full-screen
/// and other topmost apps (Codex Desktop, Cursor, etc.) can demote the island even
/// while `is_always_on_top()` still reads true, which makes it look like it vanished.
pub fn ensure_island_on_top(window: &WebviewWindow) {
    if let Some(hwnd) = window_hwnd(window) {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        return;
    }
    let _ = window.set_always_on_top(true);
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

pub fn try_restore_foreground_window(state: &AppState) -> bool {
    let previous = state
        .previous_app_pid
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    let Some(raw) = previous else {
        return false;
    };

    let hwnd = windows::Win32::Foundation::HWND(raw as *mut _);
    if hwnd.0.is_null() {
        return false;
    }

    unsafe { SetForegroundWindow(hwnd).as_bool() }
}

pub fn detect_claude_session_host(cwd: &str) -> SessionHost {
    let _ = cwd;
    if foreground_process_name()
        .map(|name| name.eq_ignore_ascii_case("Claude.exe"))
        .unwrap_or(false)
    {
        return SessionHost::ClaudeDesktop;
    }
    if foreground_is_terminal_process() {
        return SessionHost::ClaudeCli;
    }
    if try_focus_claude_process() {
        return SessionHost::ClaudeDesktop;
    }
    SessionHost::Unknown
}

pub fn detect_codex_session_host(cwd: &str) -> SessionHost {
    let _ = cwd;
    if foreground_process_name()
        .map(|name| name.eq_ignore_ascii_case("Codex.exe"))
        .unwrap_or(false)
    {
        return SessionHost::CodexDesktop;
    }
    if foreground_is_terminal_process() {
        return SessionHost::CodexCli;
    }
    if try_focus_codex_process() {
        return SessionHost::CodexDesktop;
    }
    SessionHost::Unknown
}

pub fn detect_claude_session_host_from_peer_pid(pid: u32) -> SessionHost {
    let _ = pid;
    if foreground_is_terminal_process() {
        return SessionHost::ClaudeCli;
    }
    if try_focus_claude_process() {
        return SessionHost::ClaudeDesktop;
    }
    SessionHost::Unknown
}

pub fn detect_codex_session_host_from_peer_pid(pid: u32) -> SessionHost {
    let _ = pid;
    if foreground_is_terminal_process() {
        return SessionHost::CodexCli;
    }
    if try_focus_codex_process() {
        return SessionHost::CodexDesktop;
    }
    SessionHost::Unknown
}

pub fn is_codex_desktop_app_running() -> bool {
    try_focus_codex_process()
}

fn foreground_process_name() -> Option<String> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        process_name_for_hwnd(hwnd)
    }
}

fn foreground_is_terminal_process() -> bool {
    foreground_process_name()
        .map(|name| is_terminal_process_name(&name))
        .unwrap_or(false)
}

fn is_terminal_hwnd(hwnd: windows::Win32::Foundation::HWND) -> bool {
    process_name_for_hwnd(hwnd)
        .map(|name| is_terminal_process_name(&name))
        .unwrap_or(false)
}

fn is_terminal_process_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "windowsterminal.exe" | "wt.exe" | "cmd.exe" | "powershell.exe" | "pwsh.exe"
    )
}

fn process_name_for_hwnd(hwnd: windows::Win32::Foundation::HWND) -> Option<String> {
    use windows::Win32::System::ProcessStatus::K32GetProcessImageFileNameW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0u16; 260];
        let len = K32GetProcessImageFileNameW(process, &mut buffer);
        if len == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        Some(path.rsplit(['\\', '/']).next().unwrap_or(&path).to_string())
    }
}

pub fn focus_claude_app() -> Result<(), String> {
    if try_focus_claude_process() {
        return Ok(());
    }

    hidden_command("cmd")
        .args(["/C", "start", "", "Claude"])
        .spawn()
        .map_err(|error| format!("Failed to focus Claude: {error}"))?;
    Ok(())
}

pub fn focus_codex_app() -> Result<(), String> {
    if try_focus_codex_process() {
        return Ok(());
    }

    hidden_command("cmd")
        .args(["/C", "start", "", "Codex"])
        .spawn()
        .map_err(|error| format!("Failed to focus Codex: {error}"))?;
    Ok(())
}

pub fn focus_cursor_app() -> Result<(), String> {
    if try_focus_cursor_process() {
        return Ok(());
    }

    hidden_command("cmd")
        .args(["/C", "start", "", "Cursor"])
        .spawn()
        .map_err(|error| format!("Failed to focus Cursor: {error}"))?;
    Ok(())
}

pub fn detect_cursor_session_host_from_peer_pid(pid: u32) -> SessionHost {
    if is_cursor_process_pid(pid) {
        return SessionHost::CursorIde;
    }
    SessionHost::Unknown
}

pub fn is_cursor_app_running() -> bool {
    hidden_command("tasklist")
        .args(["/FI", "IMAGENAME eq Cursor.exe", "/NH"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.contains("Cursor.exe"))
        })
        .unwrap_or(false)
}

fn is_cursor_process_pid(pid: u32) -> bool {
    hidden_command("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .contains("Cursor.exe")
        })
        .unwrap_or(false)
}

fn try_focus_claude_process() -> bool {
    try_focus_process_by_title("Claude")
}

fn try_focus_codex_process() -> bool {
    try_focus_process_by_title("Codex")
}

fn try_focus_cursor_process() -> bool {
    try_focus_process_by_title("Cursor")
}

fn try_focus_process_by_title(app_name: &'static str) -> bool {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    struct Search {
        app_name: &'static str,
        found: Option<HWND>,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut Search);
        if !IsWindowVisible(hwnd).as_bool() {
            return BOOL(1);
        }
        let mut buffer = [0u16; 256];
        let length = GetWindowTextW(hwnd, &mut buffer);
        if length > 0 {
            let title = String::from_utf16_lossy(&buffer[..length as usize]);
            if title.contains(search.app_name) {
                search.found = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let mut search = Search {
        app_name,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut search as *mut _ as isize));
        if let Some(hwnd) = search.found {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            return SetForegroundWindow(hwnd).as_bool();
        }
    }
    false
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

    hidden_command("cmd")
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
        RawWindowHandle::Win32(raw) => {
            Some(windows::Win32::Foundation::HWND(raw.hwnd.get() as *mut _))
        }
        _ => None,
    }
}
