use std::process::Command;

use tauri::{AppHandle, LogicalPosition, Manager, PhysicalSize, WebviewWindow};

use super::ScreenGeometry;
use crate::{
    AppState, HomeWindowBounds, NotchMetrics, COMPACT_WINDOW_HEIGHT, EXPANDED_WINDOW_HEIGHT,
    FALLBACK_NOTCH_HEIGHT, FALLBACK_NOTCH_WIDTH,
};

mod panel_store {
    use std::sync::atomic::{AtomicPtr, Ordering};

    static PANEL: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set(ptr: *mut std::ffi::c_void) {
        PANEL.store(ptr, Ordering::Release);
    }

    pub fn get_raw() -> *mut std::ffi::c_void {
        PANEL.load(Ordering::Acquire)
    }
}

fn ensure_island_panel_visible() {
    unsafe {
        let panel_ptr = panel_store::get_raw();
        if panel_ptr.is_null() {
            return;
        }
        let panel = panel_ptr as *mut objc2::runtime::AnyObject;
        use objc2_app_kit::NSMainMenuWindowLevel;
        let level = NSMainMenuWindowLevel + 3;
        let _: () = objc2::msg_send![panel, setLevel: level];
        let _: () = objc2::msg_send![panel, orderFrontRegardless];
    }
}

fn is_main_thread() -> bool {
    unsafe {
        let Some(thread_class) = objc2::runtime::AnyClass::get(c"NSThread") else {
            return false;
        };
        let is_main: objc2::runtime::Bool = objc2::msg_send![thread_class, isMainThread];
        is_main.as_bool()
    }
}

fn has_camera_housing(frame_width: f64, aux_left_width: f64, aux_right_width: f64) -> bool {
    aux_left_width > 0.0
        && aux_right_width > 0.0
        && aux_left_width + aux_right_width < frame_width - 1.0
}

/// Notch width in logical points, derived from the gap between the auxiliary
/// menu-bar areas (matches ping-island's detection). Falls back when the
/// auxiliary areas are unavailable.
fn notch_logical_width(
    frame_width: f64,
    aux_left_width: f64,
    aux_right_width: f64,
    fallback: f64,
) -> f64 {
    if aux_left_width > 0.0 && aux_right_width > 0.0 {
        let detected = (frame_width - aux_left_width - aux_right_width + 4.0).ceil();
        detected.max(fallback)
    } else {
        fallback
    }
}

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

pub fn detect_notch_metrics(
    window: &tauri::WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> NotchMetrics {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let safe_top = screen.safeAreaInsets().top;
        let frame = screen.frame();
        let aux_left_width = screen.auxiliaryTopLeftArea().size.width;
        let aux_right_width = screen.auxiliaryTopRightArea().size.width;
        let has_housing = has_camera_housing(frame.size.width, aux_left_width, aux_right_width);

        if safe_top <= 0.0 && !has_housing {
            return NotchMetrics::default();
        }

        NotchMetrics {
            has_notch: true,
            width: notch_logical_width(
                frame.size.width,
                aux_left_width,
                aux_right_width,
                FALLBACK_NOTCH_WIDTH,
            ),
            height: if safe_top > 0.0 {
                safe_top.ceil()
            } else {
                FALLBACK_NOTCH_HEIGHT
            },
            left_area_width: aux_left_width,
            right_area_width: aux_right_width,
        }
    })
    .unwrap_or_default()
}
pub fn set_island_cursor_events_ignored(window: &tauri::WebviewWindow, ignore: bool) {
    let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            // setIgnoresMouseEvents: MUST run on the main thread.
            // animate_island_window_mode calls us from a tokio worker,
            // so dispatch via run_on_main_thread.
            let ptr_val = panel_ptr as usize;
            let _ = window.run_on_main_thread(move || unsafe {
                use objc2::runtime::{AnyObject, Bool};
                let ptr = ptr_val as *mut AnyObject;
                let val = if ignore { Bool::YES } else { Bool::NO };
                let _: () = objc2::msg_send![ptr, setIgnoresMouseEvents: val];
                // Non-activating NSPanels do not deliver mouse-moved events to
                // the WKWebView until the panel becomes key (first click). Enable
                // mouse-moved delivery while expanded so CSS :hover works on hover.
                let moved = if ignore { Bool::NO } else { Bool::YES };
                let _: () = objc2::msg_send![ptr, setAcceptsMouseMovedEvents: moved];
            });
            return;
        }
    let _ = window.set_ignore_cursor_events(ignore);
}

pub fn set_island_window_frame_now(
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

        let height_progress = ((logical_size.height - COMPACT_WINDOW_HEIGHT)
            / (EXPANDED_WINDOW_HEIGHT - COMPACT_WINDOW_HEIGHT))
            .clamp(0.0, 1.0);
        let corner_radius = 15.0 + 7.0 * height_progress;

        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            let panel = &*(panel_ptr as *const NSWindow);
            panel.setFrame_display(frame, true);
            apply_content_view_corner_mask(panel, corner_radius);
        } else {
            apply_content_view_corner_mask(ns_window, corner_radius);
        }
    }

    Ok(())
}

pub fn set_island_window_frame(
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

    let frame_window = window.clone();
    window.run_on_main_thread(move || {
        let _ = set_island_window_frame_now(&frame_window, position, size, scale_factor, home);
    })?;

    Ok(())
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

pub fn apply_island_window_style(window: &tauri::WebviewWindow) {
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
        promote_to_floating_panel(ns_window);
        eprintln!("[Atoll] step: promote_to_floating_panel done");
        apply_macos_unconstrained_window_class(ns_window);
        eprintln!("[Atoll] step: unconstrained_window_class done");
        apply_accepts_first_mouse(ns_window);
        eprintln!("[Atoll] step: accepts_first_mouse done");
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
        eprintln!("[Atoll] step: window properties set");

        // Corner mask goes on the panel (where the WKWebView lives)
        // if it exists, otherwise on the Tauri window as fallback.
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            apply_content_view_corner_mask(&*(panel_ptr as *const NSWindow), 15.0);
        } else {
            apply_content_view_corner_mask(ns_window, 15.0);
        }
        eprintln!("[Atoll] step: apply_macos_island_window_style complete");
    }
}

/// Create a real NSPanel (properly initialised as a floating panel that
/// renders above the macOS menu bar), then move the WKWebView from the
/// Tauri window into this panel.  The Tauri window keeps an empty
/// contentView so tao's internal bookkeeping doesn't crash, and all
/// frame / mouse-event updates target the panel via `panel_store`.
fn promote_to_floating_panel(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
    use objc2::sel;
    use objc2_app_kit::{
        NSColor, NSMainMenuWindowLevel, NSScreen, NSWindow, NSWindowCollectionBehavior,
        NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| unsafe {
        let panel_cls = AnyClass::get(c"NSPanel").expect("NSPanel class");
        let frame = ns_window.frame();

        let raw: *mut AnyObject = objc2::msg_send![panel_cls, alloc];
        let style_bits: usize =
            NSWindowStyleMask::Borderless.0 as usize | (1usize << 7);
        let raw: *mut AnyObject = objc2::msg_send![
            raw,
            initWithContentRect: frame,
            styleMask: style_bits,
            backing: 2usize,
            defer: Bool::NO
        ];
        assert!(!raw.is_null(), "NSPanel init failed");

        let _: () = objc2::msg_send![raw, setFloatingPanel: Bool::YES];
        let _: () = objc2::msg_send![raw, setHidesOnDeactivate: Bool::NO];
        let _: () = objc2::msg_send![raw, setOpaque: Bool::NO];
        let clear = NSColor::clearColor();
        let _: () = objc2::msg_send![raw, setBackgroundColor: &*clear];
        let _: () = objc2::msg_send![raw, setHasShadow: Bool::NO];
        let _: () = objc2::msg_send![raw, setMovable: Bool::NO];
        let _: () = objc2::msg_send![raw, setLevel: NSMainMenuWindowLevel + 3];
        let _: () = objc2::msg_send![raw, setCollectionBehavior:
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle
        ];

        // Patch NSPanel's constrainFrameRect:toScreen: so the panel
        // is never clamped below the menu bar.
        extern "C-unwind" fn unconstrained_panel(
            _w: *mut NSWindow,
            _s: Sel,
            f: NSRect,
            _scr: *mut NSScreen,
        ) -> NSRect {
            f
        }
        let panel_class = (&*raw).class();
        let constrain_sel = sel!(constrainFrameRect:toScreen:);
        if let Some(m) = panel_class.instance_method(constrain_sel) {
            let imp: Imp = std::mem::transmute(
                unconstrained_panel
                    as extern "C-unwind" fn(*mut NSWindow, Sel, NSRect, *mut NSScreen) -> NSRect,
            );
            objc2::ffi::class_replaceMethod(
                panel_class as *const AnyClass as *mut AnyClass,
                constrain_sel,
                imp,
                objc2::ffi::method_getTypeEncoding(m),
            );
        }

        // A borderless non-activating NSPanel reports canBecomeKeyWindow == NO
        // by default, which silently swallows makeKeyAndOrderFront and leaves
        // the WKWebView unable to receive keyboard input. Force it to YES so
        // approval shortcuts work whenever we explicitly request focus.
        extern "C-unwind" fn always_yes(_w: *mut NSWindow, _s: Sel) -> Bool {
            Bool::YES
        }
        for key_sel in [sel!(canBecomeKeyWindow), sel!(canBecomeMainWindow)] {
            if let Some(m) = panel_class.instance_method(key_sel) {
                let imp: Imp = std::mem::transmute(
                    always_yes as extern "C-unwind" fn(*mut NSWindow, Sel) -> Bool,
                );
                objc2::ffi::class_replaceMethod(
                    panel_class as *const AnyClass as *mut AnyClass,
                    key_sel,
                    imp,
                    objc2::ffi::method_getTypeEncoding(m),
                );
            }
        }

        // ── Move the WKWebView from the Tauri window into the panel ──
        // We use addSubview: which automatically removes the view from
        // its old superview.  Crucially we do NOT replace the Tauri
        // window's contentView — tao keeps an internal reference to it
        // and replacing it causes a crash on mouse events.
        let content_view: *mut AnyObject = objc2::msg_send![ns_window, contentView];
        if !content_view.is_null() {
            let subviews: *mut AnyObject = objc2::msg_send![content_view, subviews];
            let count: usize = objc2::msg_send![subviews, count];
            if count > 0 {
                let wk: *mut AnyObject =
                    objc2::msg_send![subviews, objectAtIndex: 0usize];

                // addSubview: on the panel's contentView automatically
                // removes `wk` from the Tauri window's contentView.
                let pcv: *mut AnyObject = objc2::msg_send![raw, contentView];
                let _: () = objc2::msg_send![pcv, addSubview: wk];
                let bounds: NSRect = objc2::msg_send![pcv, bounds];
                let _: () = objc2::msg_send![wk, setFrame: bounds];
                // NSViewWidthSizable(2) | NSViewHeightSizable(16) = 18
                let _: () = objc2::msg_send![wk, setAutoresizingMask: 18usize];

                eprintln!("[Atoll] WKWebView moved to floating panel");
            }
        }

        // The Tauri window is now content-less; keep it permanently
        // ignoring mouse events so it never blocks the panel.
        let _: () = objc2::msg_send![ns_window, setIgnoresMouseEvents: Bool::YES];

        // Panel starts with ignoresMouseEvents=YES (compact mode).
        // The mode system will toggle this via set_island_cursor_events_ignored.
        let _: () = objc2::msg_send![raw, setIgnoresMouseEvents: Bool::YES];
        let _: () = objc2::msg_send![raw, orderFrontRegardless];

        panel_store::set(raw as *mut std::ffi::c_void);

        let is_floating: Bool = objc2::msg_send![raw, isFloatingPanel];
        eprintln!(
            "[Atoll] floating panel ready, floating={}, level={}",
            is_floating.as_bool(),
            { let lvl: isize = objc2::msg_send![raw, level]; lvl },
        );
    });
}

fn apply_accepts_first_mouse(ns_window: &objc2_app_kit::NSWindow) {
    use std::sync::OnceLock;

    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};

    extern "C-unwind" fn always_accepts(
        _view: *mut AnyObject,
        _sel: Sel,
        _event: *mut AnyObject,
    ) -> bool {
        true
    }

    unsafe fn patch_view_class(view: *mut AnyObject) {
        if view.is_null() {
            return;
        }
        let class = (&*view).class();
        let selector = objc2::sel!(acceptsFirstMouse:);
        let Some(method) = class.instance_method(selector) else {
            return;
        };
        let implementation: Imp = std::mem::transmute(
            always_accepts as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> bool,
        );
        objc2::ffi::class_replaceMethod(
            class as *const AnyClass as *mut AnyClass,
            selector,
            implementation,
            objc2::ffi::method_getTypeEncoding(method),
        );
    }

    static VIEW_PATCHED: OnceLock<()> = OnceLock::new();
    VIEW_PATCHED.get_or_init(|| unsafe {
        // Patch the Tauri window's contentView.
        let cv: *mut AnyObject = objc2::msg_send![ns_window, contentView];
        patch_view_class(cv);

        // Also patch the floating panel's views (contentView + WKWebView).
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            let pcv: *mut AnyObject =
                objc2::msg_send![panel_ptr as *mut AnyObject, contentView];
            patch_view_class(pcv);
            if !pcv.is_null() {
                let subviews: *mut AnyObject = objc2::msg_send![pcv, subviews];
                let count: usize = objc2::msg_send![subviews, count];
                for i in 0..count {
                    let sv: *mut AnyObject =
                        objc2::msg_send![subviews, objectAtIndex: i];
                    patch_view_class(sv);
                }
            }
        }
    });
}

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

unsafe fn apply_content_view_corner_mask(ns_window: &objc2_app_kit::NSWindow, radius: f64) {
    use objc2::runtime::AnyObject;

    let cv: *mut AnyObject = objc2::msg_send![ns_window, contentView];
    if cv.is_null() {
        return;
    }
    let _: () = objc2::msg_send![cv, setWantsLayer: true];
    let layer: *mut AnyObject = objc2::msg_send![cv, layer];
    if layer.is_null() {
        return;
    }
    let _: () = objc2::msg_send![layer, setCornerRadius: radius];
    let _: () = objc2::msg_send![layer, setMasksToBounds: true];
    // kCALayerMinXMinYCorner(1) | kCALayerMaxXMinYCorner(2) = bottom corners in CG coords
    let _: () = objc2::msg_send![layer, setMaskedCorners: 3_usize];
}

pub fn screen_geometry_for_monitor(
    window: &WebviewWindow,
    monitor_x: f64,
    monitor_width: f64,
) -> Option<ScreenGeometry> {
    with_nsscreen_for_monitor(window, monitor_x, monitor_width, |screen| {
        let frame = screen.frame();
        ScreenGeometry {
            origin_y: frame.origin.y,
            height: frame.size.height,
        }
    })
}

pub fn remember_frontmost_app(app: &AppHandle) {
    let own_pid = std::process::id() as i32;
    unsafe {
        let Some(ws_class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return;
        };
        let workspace: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return;
        }
        let front: *mut objc2::runtime::AnyObject =
            objc2::msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return;
        }
        let pid: i32 = objc2::msg_send![front, processIdentifier];
        if pid <= 0 || pid == own_pid {
            return;
        }
        if let Ok(mut guard) = app.state::<AppState>().previous_app_pid.lock() {
            *guard = Some(pid as i64);
        }
    }
}

unsafe fn activate_app_by_pid(pid: i32) -> bool {
    let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
        return false;
    };
    let running: *mut objc2::runtime::AnyObject =
        objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid];
    if running.is_null() {
        return false;
    }
    let options: usize = 1 << 1;
    let ok: objc2::runtime::Bool = objc2::msg_send![running, activateWithOptions: options];
    ok.as_bool()
}

pub fn try_restore_previous_app_focus(state: &AppState) -> bool {
    let previous = state
        .previous_app_pid
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());

    let Some(pid) = previous else {
        return false;
    };

    unsafe { activate_app_by_pid(pid as i32) }
}

pub fn deactivate_atoll_app() {
    deactivate_own_application();
}

pub fn activate_claude_app(app: &AppHandle) -> Result<(), String> {
    focus_claude_app_impl(app, false)
}

pub fn focus_claude_app(app: &AppHandle) -> Result<(), String> {
    focus_claude_app_impl(app, true)
}

fn focus_claude_app_impl(app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    let app = app.clone();
    if is_main_thread() {
        return focus_claude_app_on_main_thread(&app, launch_if_needed);
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = tx.send(focus_claude_app_on_main_thread(&app, launch_if_needed));
        })
        .map_err(|error| format!("Failed to dispatch Claude focus: {error}"))?;
    rx.recv()
        .map_err(|_| "Claude focus dispatch channel closed".to_string())?
}

fn focus_claude_app_on_main_thread(_app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    deactivate_own_application();

    let focused = if launch_if_needed {
        run_open_claude()
            || activate_claude_by_bundle_id()
            || activate_claude_via_applescript()
    } else {
        activate_claude_by_bundle_id() || activate_claude_via_applescript()
    };
    if !focused {
        return Err("Failed to focus Claude".to_string());
    }

    // Keep the compact island visible in the menu bar after handing off focus.
    ensure_island_panel_visible();
    Ok(())
}

fn run_open_claude() -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", "Claude"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn deactivate_own_application() {
    unsafe {
        let Some(ns_app_class) = objc2::runtime::AnyClass::get(c"NSApplication") else {
            return;
        };
        let ns_app: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ns_app_class, sharedApplication];
        if !ns_app.is_null() {
            let _: () = objc2::msg_send![ns_app, deactivate];
        }
    }
}

fn activate_claude_via_applescript() -> bool {
    Command::new("/usr/bin/osascript")
        .args(["-e", r#"tell application "Claude" to activate"#])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn activate_claude_by_bundle_id() -> bool {
    const ACTIVATE_ALL_WINDOWS: usize = 1;
    const ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
    let options = ACTIVATE_ALL_WINDOWS | ACTIVATE_IGNORING_OTHER_APPS;

    unsafe {
        let Some(running_app_class) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };

        for bundle_id in CLAUDE_DESKTOP_BUNDLE_IDS {
            let bundle = objc2_foundation::NSString::from_str(bundle_id);
            let apps: *mut objc2::runtime::AnyObject = objc2::msg_send![
                running_app_class,
                runningApplicationsWithBundleIdentifier: &*bundle
            ];
            if apps.is_null() {
                continue;
            }

            let count: usize = objc2::msg_send![apps, count];
            for index in 0..count {
                let app: *mut objc2::runtime::AnyObject =
                    objc2::msg_send![apps, objectAtIndex: index];
                if app.is_null() {
                    continue;
                }

                let ok: objc2::runtime::Bool =
                    objc2::msg_send![app, activateWithOptions: options];
                if ok.as_bool() {
                    return true;
                }
            }
        }
        false
    }
}

pub fn finish_show_for_approval(window: &WebviewWindow, app: &AppHandle, request_focus: bool) {
    let window_for_main_thread = window.clone();
    let app_for_focus = app.clone();
    let _ = window.run_on_main_thread(move || {
        let _ = window_for_main_thread.show();
        if request_focus {
            remember_frontmost_app(&app_for_focus);
            let _ = window_for_main_thread.set_focus();
        }
        let panel_ptr = panel_store::get_raw();
        if !panel_ptr.is_null() {
            unsafe {
                let panel_ptr = panel_ptr as *mut objc2::runtime::AnyObject;
                let _: () = objc2::msg_send![panel_ptr, orderFrontRegardless];
                if request_focus {
                    if let Some(ns_app_class) = objc2::runtime::AnyClass::get(c"NSApplication") {
                        let ns_app: *mut objc2::runtime::AnyObject =
                            objc2::msg_send![ns_app_class, sharedApplication];
                        if !ns_app.is_null() {
                            let _: () = objc2::msg_send![
                                ns_app,
                                activateIgnoringOtherApps: objc2::runtime::Bool::YES
                            ];
                        }
                    }
                    let _: () = objc2::msg_send![
                        panel_ptr,
                        makeKeyAndOrderFront: std::ptr::null_mut::<objc2::runtime::AnyObject>()
                    ];
                }
            }
        }
    });
}

const KNOWN_TERMINALS: &[(&str, &str)] = &[
    ("ghostty", "Ghostty"),
    ("Ghostty", "Ghostty"),
    ("iTerm2", "iTerm2"),
    ("iTerm2-Server", "iTerm2"),
    ("Terminal", "Terminal"),
    ("kitty", "kitty"),
    ("alacritty", "Alacritty"),
    ("Alacritty", "Alacritty"),
    ("wezterm-gui", "WezTerm"),
    ("WezTerm", "WezTerm"),
    ("Hyper", "Hyper"),
    ("tabby", "Tabby"),
    ("rio", "Rio"),
];

use super::SessionHost;

pub fn detect_claude_session_host(cwd: &str) -> SessionHost {
    if cwd.is_empty() || cwd == "." {
        return SessionHost::Unknown;
    }

    let pids = pids_with_cwd(cwd);

    // Desktop-hosted session processes (not running under a terminal).
    for pid in &pids {
        if find_terminal_ancestor(*pid).is_none() && is_in_claude_desktop_tree(*pid) {
            return SessionHost::ClaudeDesktop;
        }
    }

    // Terminal-hosted Claude Code (ignore unrelated shells in the same cwd).
    for pid in &pids {
        if find_terminal_ancestor(*pid).is_some() && is_claude_related_process(*pid) {
            return SessionHost::ClaudeCli;
        }
    }

    if frontmost_is_claude_desktop() {
        return SessionHost::ClaudeDesktop;
    }

    if frontmost_is_terminal() {
        return SessionHost::ClaudeCli;
    }

    SessionHost::Unknown
}

/// Snapshot frontmost app at hook time, before Atoll steals focus.
pub fn detect_claude_session_host_at_hook(cwd: &str) -> SessionHost {
    if frontmost_is_claude_desktop() {
        return SessionHost::ClaudeDesktop;
    }
    if frontmost_is_terminal() {
        return SessionHost::ClaudeCli;
    }
    detect_claude_session_host(cwd)
}

pub(crate) fn frontmost_is_claude_desktop() -> bool {
    unsafe {
        let Some(ws_class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return false;
        };
        let workspace: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let front: *mut objc2::runtime::AnyObject =
            objc2::msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return false;
        }
        let pid: i32 = objc2::msg_send![front, processIdentifier];
        is_claude_desktop_pid(pid as u32)
    }
}

pub(crate) fn frontmost_is_terminal() -> bool {
    unsafe {
        let Some(ws_class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return false;
        };
        let workspace: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let front: *mut objc2::runtime::AnyObject =
            objc2::msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return false;
        }
        let pid: i32 = objc2::msg_send![front, processIdentifier];
        is_terminal_pid(pid as u32)
    }
}

fn is_in_claude_desktop_tree(mut pid: u32) -> bool {
    for _ in 0..32 {
        if pid <= 1 {
            return false;
        }
        if is_claude_desktop_process(pid) {
            return true;
        }
        let output = match Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "ppid="])
            .output()
        {
            Ok(output) => output,
            Err(_) => return false,
        };
        let ppid_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
        pid = match ppid_str.parse::<u32>() {
            Ok(ppid) => ppid,
            Err(_) => return false,
        };
    }
    false
}

fn pids_with_cwd(cwd: &str) -> Vec<u32> {
    let output = match Command::new("lsof").args(["-d", "cwd", "+c", "0"]).output() {
        Ok(output) => output,
        Err(_) => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut pids = Vec::new();
    for line in text.lines().skip(1) {
        if line.contains(cwd) {
            if let Some(pid_str) = line.split_whitespace().nth(1) {
                if let Ok(pid) = pid_str.parse::<u32>() {
                    pids.push(pid);
                }
            }
        }
    }
    pids
}

const CLAUDE_DESKTOP_BUNDLE_IDS: &[&str] = &[
    "com.anthropic.claudefordesktop",
    "com.anthropic.claude",
];

fn is_claude_desktop_bundle(bundle: &str) -> bool {
    CLAUDE_DESKTOP_BUNDLE_IDS.contains(&bundle)
}

fn process_executable(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    let comm = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if comm.is_empty() {
        None
    } else {
        Some(comm)
    }
}

fn is_claude_related_process(pid: u32) -> bool {
    if is_claude_desktop_pid(pid) {
        return true;
    }
    process_executable(pid).is_some_and(|comm| {
        comm.contains("Claude.app")
            || comm.contains("Claude Helper")
            || comm.contains("Claude-3p/claude-code")
            || comm.contains("claude-code")
            || comm.ends_with("/claude")
    })
}

fn is_claude_desktop_process(pid: u32) -> bool {
    if find_terminal_ancestor(pid).is_some() {
        return false;
    }
    if is_claude_desktop_pid(pid) {
        return true;
    }
    process_executable(pid).is_some_and(|comm| {
        comm.contains("Claude.app")
            || comm.contains("Claude Helper")
            || comm.contains("Claude-3p/claude-code")
    })
}

fn is_claude_desktop_pid(pid: u32) -> bool {
    bundle_id_for_pid(pid as i32)
        .as_deref()
        .is_some_and(is_claude_desktop_bundle)
}

fn is_terminal_pid(pid: u32) -> bool {
    find_terminal_ancestor(pid).is_some()
}

fn bundle_id_for_pid(pid: i32) -> Option<String> {
    unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSRunningApplication")?;
        let running: *mut objc2::runtime::AnyObject =
            objc2::msg_send![cls, runningApplicationWithProcessIdentifier: pid];
        if running.is_null() {
            return None;
        }
        let bundle: *mut objc2_foundation::NSString =
            objc2::msg_send![running, bundleIdentifier];
        if bundle.is_null() {
            return None;
        }
        Some((*bundle).to_string())
    }
}

pub fn open_in_terminal(cwd: &str) -> Result<(), String> {
    if let Some(app) = detect_terminal_app_for_cwd(cwd) {
        Command::new("open")
            .arg("-a")
            .arg(&app)
            .spawn()
            .map_err(|e| format!("Failed to activate {app}: {e}"))?;
    } else {
        Command::new("open")
            .arg("-a")
            .arg("Terminal")
            .arg(cwd)
            .spawn()
            .map_err(|e| format!("Failed to open terminal: {e}"))?;
    }
    Ok(())
}

fn detect_terminal_app_for_cwd(cwd: &str) -> Option<String> {
    for pid in pids_with_cwd(cwd) {
        if let Some(app) = find_terminal_ancestor(pid) {
            return Some(app);
        }
    }
    None
}

fn find_terminal_ancestor(mut pid: u32) -> Option<String> {
    for _ in 0..20 {
        if pid <= 1 {
            return None;
        }
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "ppid=,comm="])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&output.stdout);
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let ppid_str = parts.next()?.trim();
        let comm = parts.next()?.trim();

        let basename = comm.rsplit('/').next().unwrap_or(comm);
        for &(pattern, app_name) in KNOWN_TERMINALS {
            if basename == pattern {
                return Some(app_name.to_string());
            }
        }

        pid = ppid_str.parse::<u32>().ok()?;
    }
    None
}
