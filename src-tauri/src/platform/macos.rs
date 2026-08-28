use std::process::Command;
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use super::ScreenGeometry;
use crate::{
    AppState, HomeWindowBounds, NotchMetrics, COMPACT_WINDOW_HEIGHT, EXPANDED_WINDOW_HEIGHT,
    FALLBACK_NOTCH_HEIGHT, FALLBACK_NOTCH_WIDTH,
};

mod panel_store {
    use std::sync::atomic::{AtomicPtr, Ordering};

    static PANEL: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
    static TAURI_WINDOW: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

    pub fn set(ptr: *mut std::ffi::c_void) {
        PANEL.store(ptr, Ordering::Release);
    }

    pub fn get_raw() -> *mut std::ffi::c_void {
        PANEL.load(Ordering::Acquire)
    }

    pub fn set_tauri(ptr: *mut std::ffi::c_void) {
        TAURI_WINDOW.store(ptr, Ordering::Release);
    }

    pub fn get_tauri() -> *mut std::ffi::c_void {
        TAURI_WINDOW.load(Ordering::Acquire)
    }
}

/// Keep Chinese IME candidate windows above the island NSPanel.
/// The island sits at NSMainMenuWindowLevel+3 so it can cover the menu bar;
/// IMK/TSM candidate UI is usually lower (or the same level) and gets covered
/// unless we tell the input method to draw at pop-up-menu level.
mod ime_overlay {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Imp, NSObject, Sel};
    use objc2::{define_class, msg_send, ClassType};
    use objc2_app_kit::{NSMainMenuWindowLevel, NSPopUpMenuWindowLevel};
    use objc2_foundation::NSString;
    use tauri::WebviewWindow;

    use super::panel_store;

    static IME_ACTIVE: AtomicBool = AtomicBool::new(false);

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "AtollImeWindowObserver"]
        struct AtollImeWindowObserver;

        impl AtollImeWindowObserver {
            #[unsafe(method(atollImeWindowDidChange:))]
            fn atoll_ime_window_did_change(&self, _notification: Option<&AnyObject>) {
                apply_tsm_window_level();
                if IME_ACTIVE.load(Ordering::SeqCst) {
                    raise_in_process_ime_windows();
                }
            }
        }
    );

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TSMGetActiveDocument() -> *mut c_void;
        fn TSMSetDocumentProperty(
            doc: *mut c_void,
            property_tag: u32,
            size: u32,
            data: *const c_void,
        ) -> i32;
    }

    /// FourCharCode `'wlev'` — TSM document property for the candidate window level.
    const TSM_DOCUMENT_WINDOW_LEVEL_TAG: u32 = u32::from_be_bytes(*b"wlev");

    pub fn island_window_level() -> isize {
        NSMainMenuWindowLevel + 3
    }

    pub fn ime_window_level() -> isize {
        NSPopUpMenuWindowLevel
    }

    pub fn should_order_island_front() -> bool {
        !IME_ACTIVE.load(Ordering::SeqCst)
    }

    /// True when `window` is the island panel or the leftover Tauri NSWindow.
    pub fn is_island_owned_window(window: usize, panel: usize, tauri_window: usize) -> bool {
        window == 0 || window == panel || (tauri_window != 0 && window == tauri_window)
    }

    pub fn set_ime_active(window: &WebviewWindow, active: bool) {
        IME_ACTIVE.store(active, Ordering::SeqCst);
        if !active {
            return;
        }
        let window = window.clone();
        let _ = window.run_on_main_thread(|| {
            apply_tsm_window_level();
            raise_in_process_ime_windows();
        });
    }

    pub fn install_observer() {
        static OBSERVER: OnceLock<Retained<AtollImeWindowObserver>> = OnceLock::new();
        OBSERVER.get_or_init(|| unsafe {
            let observer: Retained<AtollImeWindowObserver> =
                msg_send![AtollImeWindowObserver::class(), new];
            let Some(center_class) = AnyClass::get(c"NSNotificationCenter") else {
                return observer;
            };
            let center: *mut AnyObject = msg_send![center_class, defaultCenter];
            if center.is_null() {
                return observer;
            }
            let selector = objc2::sel!(atollImeWindowDidChange:);
            for name in [
                "NSWindowDidBecomeKeyNotification",
                "NSWindowDidChangeOcclusionStateNotification",
                "NSWindowDidOrderOnScreenNotification",
            ] {
                let ns_name = NSString::from_str(name);
                let _: () = msg_send![
                    center,
                    addObserver: &*observer,
                    selector: selector,
                    name: &*ns_name,
                    object: std::ptr::null_mut::<AnyObject>()
                ];
            }
            observer
        });
    }

    pub fn patch_view_tree(view: *mut AnyObject) {
        unsafe { patch_view_tree_ime_level(view) };
    }

    fn apply_tsm_window_level() {
        unsafe {
            let doc = TSMGetActiveDocument();
            if doc.is_null() {
                return;
            }
            let level = ime_window_level() as i32;
            let _ = TSMSetDocumentProperty(
                doc,
                TSM_DOCUMENT_WINDOW_LEVEL_TAG,
                std::mem::size_of::<i32>() as u32,
                (&level as *const i32).cast::<c_void>(),
            );
        }
    }

    fn raise_in_process_ime_windows() {
        unsafe {
            let Some(app_class) = AnyClass::get(c"NSApplication") else {
                return;
            };
            let app: *mut AnyObject = msg_send![app_class, sharedApplication];
            if app.is_null() {
                return;
            }
            let windows: *mut AnyObject = msg_send![app, windows];
            if windows.is_null() {
                return;
            }
            let count: usize = msg_send![windows, count];
            let panel = panel_store::get_raw();
            let tauri = panel_store::get_tauri();
            let ime_level = ime_window_level();
            for index in 0..count {
                let window: *mut AnyObject = msg_send![windows, objectAtIndex: index];
                if is_island_owned_window(window as usize, panel as usize, tauri as usize) {
                    continue;
                }
                let current: isize = msg_send![window, level];
                if current < ime_level {
                    let _: () = msg_send![window, setLevel: ime_level];
                }
            }
        }
    }

    fn class_is_generic_appkit(class: &AnyClass) -> bool {
        matches!(
            class.name().to_str().unwrap_or(""),
            "NSView" | "NSWindow" | "NSPanel" | "NSControl" | "NSResponder"
        )
    }

    unsafe fn patch_view_tree_ime_level(view: *mut AnyObject) {
        if view.is_null() {
            return;
        }
        let class = (*view).class();
        if !class_is_generic_appkit(class) {
            patch_imk_window_level(class);
        }
        let subviews: *mut AnyObject = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for index in 0..count {
            let subview: *mut AnyObject = msg_send![subviews, objectAtIndex: index];
            patch_view_tree_ime_level(subview);
        }
    }

    unsafe fn patch_imk_window_level(class: &AnyClass) {
        static PATCHED: Mutex<Vec<usize>> = Mutex::new(Vec::new());
        let key = class as *const AnyClass as usize;
        {
            let mut patched = PATCHED.lock().unwrap_or_else(|error| error.into_inner());
            if patched.contains(&key) {
                return;
            }
            patched.push(key);
        }

        extern "C-unwind" fn ime_window_level_imp(_view: *mut AnyObject, _sel: Sel) -> isize {
            NSPopUpMenuWindowLevel
        }

        let selector = objc2::sel!(windowLevel);
        let types = if let Some(method) = class.instance_method(selector) {
            objc2::ffi::method_getTypeEncoding(method)
        } else {
            c"q@:".as_ptr()
        };
        let implementation: Imp = std::mem::transmute(
            ime_window_level_imp as extern "C-unwind" fn(*mut AnyObject, Sel) -> isize,
        );
        let cls = class as *const AnyClass as *mut AnyClass;
        if !objc2::ffi::class_addMethod(cls, selector, implementation, types).as_bool() {
            objc2::ffi::class_replaceMethod(cls, selector, implementation, types);
        }
    }
}

fn ensure_island_panel_visible() {
    unsafe {
        let panel_ptr = panel_store::get_raw();
        if panel_ptr.is_null() {
            return;
        }
        let panel = panel_ptr as *mut objc2::runtime::AnyObject;
        let level = ime_overlay::island_window_level();
        let _: () = objc2::msg_send![panel, setLevel: level];
        if ime_overlay::should_order_island_front() {
            let _: () = objc2::msg_send![panel, orderFrontRegardless];
        }
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
    apply_island_cursor_events(window, ignore, None);
}

/// Like [`set_island_cursor_events_ignored`], but the toggle is skipped when
/// `presentation_generation` has moved past `generation` by the time the
/// main-thread block actually runs. Without this, a finished (but not yet
/// finalized) collapse animation can re-enable click pass-through after a
/// newer expand animation already accepted clicks — leaving an expanded
/// island that ignores every pointer event until the next cycle.
pub fn set_island_cursor_events_ignored_if_current(
    window: &tauri::WebviewWindow,
    ignore: bool,
    presentation_generation: std::sync::Arc<std::sync::atomic::AtomicU64>,
    generation: u64,
) {
    apply_island_cursor_events(
        window,
        ignore,
        Some((presentation_generation, generation)),
    );
}

fn apply_island_cursor_events(
    window: &tauri::WebviewWindow,
    ignore: bool,
    generation_guard: Option<(
        std::sync::Arc<std::sync::atomic::AtomicU64>,
        u64,
    )>,
) {
    let panel_ptr = panel_store::get_raw();
    if !panel_ptr.is_null() {
        // setIgnoresMouseEvents: MUST run on the main thread.
        // animate_island_window_mode calls us from a tokio worker,
        // so dispatch via run_on_main_thread.
        let ptr_val = panel_ptr as usize;
        let _ = window.run_on_main_thread(move || unsafe {
            if let Some((generation, expected)) = &generation_guard {
                if generation.load(std::sync::atomic::Ordering::SeqCst) != *expected {
                    return;
                }
            }
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
    if generation_guard
        .as_ref()
        .is_some_and(|(generation, expected)| {
            generation.load(std::sync::atomic::Ordering::SeqCst) != *expected
        })
    {
        return;
    }
    let _ = window.set_ignore_cursor_events(ignore);
}

pub fn set_island_window_frame_now(
    window: &tauri::WebviewWindow,
    position: LogicalPosition<f64>,
    size: LogicalSize<f64>,
    _scale_factor: f64,
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

    let logical_size = size;
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
    size: LogicalSize<f64>,
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

/// Animation frame interval derived from the display the island currently
/// sits on (ProMotion panels report 120). Falls back to 60 Hz when the screen
/// cannot be resolved, and clamps to a sane range so a misbehaving display
/// cannot starve the animation thread.
pub fn display_animation_frame_interval(window: &tauri::WebviewWindow) -> Duration {
    const FALLBACK_FPS: i64 = 60;
    let (fps_tx, fps_rx) = std::sync::mpsc::sync_channel::<i64>(1);
    let probe_window = window.clone();
    let dispatched = window.run_on_main_thread(move || {
        let fps = unsafe {
            let Ok(ns_window_ptr) = probe_window.ns_window() else {
                return;
            };
            if ns_window_ptr.is_null() {
                return;
            }
            use objc2_app_kit::NSWindow;
            let ns_window = &*(ns_window_ptr.cast::<NSWindow>());
            match ns_window.screen() {
                Some(screen) => screen.maximumFramesPerSecond() as i64,
                None => FALLBACK_FPS,
            }
        };
        let _ = fps_tx.send(fps);
    });
    if dispatched.is_err() {
        return Duration::from_secs_f64(1.0 / FALLBACK_FPS as f64);
    }
    let fps = fps_rx
        .recv_timeout(Duration::from_millis(250))
        .unwrap_or(FALLBACK_FPS)
        .clamp(30, 240);
    Duration::from_secs_f64(1.0 / fps as f64)
}

/// True when the user enabled "Reduce motion" in Accessibility settings.
/// `NSWorkspace.accessibilityDisplayShouldReduceMotion` is a category method
/// not exposed by the objc2-app-kit features we enable, so send it directly.
pub fn prefers_reduced_motion() -> bool {
    unsafe {
        let Some(ws_class) = objc2::runtime::AnyClass::get(c"NSWorkspace") else {
            return false;
        };
        let workspace: *mut objc2::runtime::AnyObject =
            objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return false;
        }
        let reduce: objc2::runtime::Bool =
            objc2::msg_send![workspace, accessibilityDisplayShouldReduceMotion];
        reduce.as_bool()
    }
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
        NSColor, NSWindow, NSWindowAnimationBehavior, NSWindowCollectionBehavior,
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
        ns_window.setLevel(ime_overlay::island_window_level());
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

pub fn set_ime_active(window: &WebviewWindow, active: bool) {
    ime_overlay::set_ime_active(window, active);
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
        NSColor, NSScreen, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
    };
    use objc2_foundation::NSRect;

    static DONE: OnceLock<()> = OnceLock::new();
    DONE.get_or_init(|| unsafe {
        let panel_cls = AnyClass::get(c"NSPanel").expect("NSPanel class");
        let frame = ns_window.frame();

        let raw: *mut AnyObject = objc2::msg_send![panel_cls, alloc];
        let style_bits: usize = NSWindowStyleMask::Borderless.0 as usize | (1usize << 7);
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
        let _: () = objc2::msg_send![raw, setLevel: ime_overlay::island_window_level()];
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
                let wk: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: 0usize];

                // addSubview: on the panel's contentView automatically
                // removes `wk` from the Tauri window's contentView.
                let pcv: *mut AnyObject = objc2::msg_send![raw, contentView];
                let _: () = objc2::msg_send![pcv, addSubview: wk];
                let bounds: NSRect = objc2::msg_send![pcv, bounds];
                let _: () = objc2::msg_send![wk, setFrame: bounds];
                // NSViewWidthSizable(2) | NSViewHeightSizable(16) = 18
                let _: () = objc2::msg_send![wk, setAutoresizingMask: 18usize];

                eprintln!("[Atoll] WKWebView moved to floating panel");
                ime_overlay::patch_view_tree(wk);
                ime_overlay::patch_view_tree(pcv);
            }
        }

        panel_store::set_tauri(ns_window as *const NSWindow as *mut std::ffi::c_void);

        // The Tauri window is now content-less; keep it permanently
        // ignoring mouse events so it never blocks the panel.
        let _: () = objc2::msg_send![ns_window, setIgnoresMouseEvents: Bool::YES];

        // Panel starts with ignoresMouseEvents=YES (compact mode).
        // The mode system will toggle this via set_island_cursor_events_ignored.
        let _: () = objc2::msg_send![raw, setIgnoresMouseEvents: Bool::YES];
        let _: () = objc2::msg_send![raw, orderFrontRegardless];

        panel_store::set(raw as *mut std::ffi::c_void);
        ime_overlay::install_observer();

        let is_floating: Bool = objc2::msg_send![raw, isFloatingPanel];
        eprintln!(
            "[Atoll] floating panel ready, floating={}, level={}",
            is_floating.as_bool(),
            {
                let lvl: isize = objc2::msg_send![raw, level];
                lvl
            },
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
            let pcv: *mut AnyObject = objc2::msg_send![panel_ptr as *mut AnyObject, contentView];
            patch_view_class(pcv);
            if !pcv.is_null() {
                let subviews: *mut AnyObject = objc2::msg_send![pcv, subviews];
                let count: usize = objc2::msg_send![subviews, count];
                for i in 0..count {
                    let sv: *mut AnyObject = objc2::msg_send![subviews, objectAtIndex: i];
                    patch_view_class(sv);
                    ime_overlay::patch_view_tree(sv);
                }
            }
            ime_overlay::patch_view_tree(pcv);
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
        let workspace: *mut objc2::runtime::AnyObject = objc2::msg_send![ws_class, sharedWorkspace];
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

pub fn activate_codex_app(app: &AppHandle) -> Result<(), String> {
    focus_codex_app_impl(app, false)
}

pub fn focus_codex_app(app: &AppHandle) -> Result<(), String> {
    focus_codex_app_impl(app, true)
}

fn focus_codex_app_impl(app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    let app = app.clone();
    if is_main_thread() {
        return focus_codex_app_on_main_thread(&app, launch_if_needed);
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = tx.send(focus_codex_app_on_main_thread(&app, launch_if_needed));
        })
        .map_err(|error| format!("Failed to dispatch Codex focus: {error}"))?;
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "Codex focus dispatch timed out".to_string())?
}

fn focus_codex_app_on_main_thread(_app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    deactivate_own_application();

    let focused = if launch_if_needed {
        run_open_codex() || activate_codex_by_bundle_id() || activate_codex_via_applescript()
    } else {
        activate_codex_by_bundle_id() || activate_codex_via_applescript()
    };
    if !focused {
        return Err("Failed to focus Codex".to_string());
    }

    ensure_island_panel_visible();
    Ok(())
}

fn run_open_codex() -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", "Codex"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn activate_codex_via_applescript() -> bool {
    Command::new("/usr/bin/osascript")
        .args(["-e", r#"tell application "Codex" to activate"#])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn activate_codex_by_bundle_id() -> bool {
    const ACTIVATE_ALL_WINDOWS: usize = 1;
    const ACTIVATE_IGNORING_OTHER_APPS: usize = 1 << 1;
    let options = ACTIVATE_ALL_WINDOWS | ACTIVATE_IGNORING_OTHER_APPS;

    unsafe {
        let Some(running_app_class) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };

        for bundle_id in CODEX_DESKTOP_BUNDLE_IDS {
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

                let ok: objc2::runtime::Bool = objc2::msg_send![app, activateWithOptions: options];
                if ok.as_bool() {
                    return true;
                }
            }
        }
        false
    }
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
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "Claude focus dispatch timed out".to_string())?
}

fn focus_claude_app_on_main_thread(_app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    deactivate_own_application();

    let focused = if launch_if_needed {
        run_open_claude() || activate_claude_by_bundle_id() || activate_claude_via_applescript()
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

pub fn focus_cursor_app(app: &AppHandle) -> Result<(), String> {
    let app = app.clone();
    if is_main_thread() {
        return focus_cursor_app_on_main_thread(&app, true);
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    window
        .run_on_main_thread(move || {
            let _ = tx.send(focus_cursor_app_on_main_thread(&app, true));
        })
        .map_err(|error| format!("Failed to dispatch Cursor focus: {error}"))?;
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .map_err(|_| "Cursor focus dispatch timed out".to_string())?
}

fn focus_cursor_app_on_main_thread(_app: &AppHandle, launch_if_needed: bool) -> Result<(), String> {
    deactivate_own_application();

    let focused = if launch_if_needed {
        run_open_cursor() || activate_cursor_by_bundle_id()
    } else {
        activate_cursor_by_bundle_id()
    };
    if !focused {
        return Err("Failed to focus Cursor".to_string());
    }

    ensure_island_panel_visible();
    Ok(())
}

fn run_open_cursor() -> bool {
    Command::new("/usr/bin/open")
        .args(["-a", "Cursor"])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn activate_cursor_by_bundle_id() -> bool {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };
        let options: u64 = 1 << 1;
        for bundle_id in CURSOR_BUNDLE_IDS {
            let bundle = objc2_foundation::NSString::from_str(bundle_id);
            let apps: *mut objc2::runtime::AnyObject = objc2::msg_send![
                cls,
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

                let ok: objc2::runtime::Bool = objc2::msg_send![app, activateWithOptions: options];
                if ok.as_bool() {
                    return true;
                }
            }
        }
        false
    }
}

pub fn detect_cursor_session_host_from_peer_pid(pid: u32) -> SessionHost {
    if is_in_cursor_tree(pid) {
        return SessionHost::CursorIde;
    }
    SessionHost::Unknown
}

pub(crate) fn is_cursor_app_running() -> bool {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };
        for bundle_id in CURSOR_BUNDLE_IDS {
            let bundle = objc2_foundation::NSString::from_str(bundle_id);
            let apps: *mut objc2::runtime::AnyObject = objc2::msg_send![
                cls,
                runningApplicationsWithBundleIdentifier: &*bundle
            ];
            if apps.is_null() {
                continue;
            }
            let count: usize = objc2::msg_send![apps, count];
            if count > 0 {
                return true;
            }
        }
        false
    }
}

fn is_in_cursor_tree(mut pid: u32) -> bool {
    for _ in 0..32 {
        if pid <= 1 {
            return false;
        }
        if is_cursor_process(pid) {
            return true;
        }
        let output = match super::command_output_with_timeout(
            Command::new("ps").args(["-p", &pid.to_string(), "-o", "ppid="]),
            std::time::Duration::from_secs(2),
        )
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

fn is_cursor_process(pid: u32) -> bool {
    bundle_id_for_pid(pid as i32)
        .is_some_and(|bundle| CURSOR_BUNDLE_IDS.contains(&bundle.as_str()))
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

                let ok: objc2::runtime::Bool = objc2::msg_send![app, activateWithOptions: options];
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
                if ime_overlay::should_order_island_front() {
                    let _: () = objc2::msg_send![panel_ptr, orderFrontRegardless];
                }
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

fn claude_cwd_signals(cwd: &str) -> (bool, bool) {
    let mut has_terminal_claude = false;
    let mut has_desktop_claude = false;
    for pid in pids_with_cwd(cwd) {
        if find_terminal_ancestor(pid).is_some() && is_claude_related_process(pid) {
            has_terminal_claude = true;
        } else if find_terminal_ancestor(pid).is_none() && is_in_claude_desktop_tree(pid) {
            has_desktop_claude = true;
        }
    }
    (has_terminal_claude, has_desktop_claude)
}

fn frontmost_app_pid() -> Option<u32> {
    unsafe {
        let ws_class = objc2::runtime::AnyClass::get(c"NSWorkspace")?;
        let workspace: *mut objc2::runtime::AnyObject = objc2::msg_send![ws_class, sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let front: *mut objc2::runtime::AnyObject =
            objc2::msg_send![workspace, frontmostApplication];
        if front.is_null() {
            return None;
        }
        let pid: i32 = objc2::msg_send![front, processIdentifier];
        if pid <= 0 {
            None
        } else {
            Some(pid as u32)
        }
    }
}

fn is_claude_desktop_app_pid(pid: u32) -> bool {
    if find_terminal_ancestor(pid).is_some() {
        return false;
    }
    is_claude_desktop_pid(pid) || is_in_claude_desktop_tree(pid)
}

pub fn resolve_claude_session_host(cwd: &str, hint_pid: Option<u32>) -> SessionHost {
    if cwd.is_empty() || cwd == "." {
        return SessionHost::Unknown;
    }

    if let Some(pid) = hint_pid {
        if is_claude_desktop_app_pid(pid) {
            return SessionHost::ClaudeDesktop;
        }
        if is_terminal_pid(pid) {
            return SessionHost::ClaudeCli;
        }
    }

    let (has_terminal_claude, has_desktop_claude) = claude_cwd_signals(cwd);
    match (has_terminal_claude, has_desktop_claude) {
        (true, false) => return SessionHost::ClaudeCli,
        (false, true) => return SessionHost::ClaudeDesktop,
        (true, true) => return SessionHost::Unknown,
        (false, false) => {}
    }

    if frontmost_is_claude_desktop() {
        return SessionHost::ClaudeDesktop;
    }

    if frontmost_is_terminal() {
        return SessionHost::ClaudeCli;
    }

    SessionHost::Unknown
}

pub fn detect_claude_session_host(cwd: &str) -> SessionHost {
    resolve_claude_session_host(cwd, None)
}

pub fn detect_codex_session_host_from_peer_pid(pid: u32) -> SessionHost {
    if find_terminal_ancestor(pid).is_some() {
        return SessionHost::CodexCli;
    }
    if is_in_codex_desktop_tree(pid) {
        return SessionHost::CodexDesktop;
    }
    SessionHost::Unknown
}

/// Walk the peer process's ancestry to determine if it originates from a
/// terminal (CLI) or Claude Desktop.  This is the most reliable method when
/// both Desktop and CLI share a working directory, because the frontmost app
/// hint becomes ambiguous (e.g. when Cursor is in the foreground).
pub fn detect_session_host_from_peer_pid(pid: u32) -> SessionHost {
    if find_terminal_ancestor(pid).is_some() {
        return SessionHost::ClaudeCli;
    }
    if is_in_claude_desktop_tree(pid) {
        return SessionHost::ClaudeDesktop;
    }
    SessionHost::Unknown
}

/// Snapshot frontmost app at hook time, before Atoll steals focus.
/// If Atoll is already frontmost (rapid-fire approvals), fall back to previous_app_pid.
pub fn detect_claude_session_host_at_hook(cwd: &str, previous_app_pid: Option<i64>) -> SessionHost {
    let own_pid = std::process::id();
    let frontmost = frontmost_app_pid();

    let hint = match frontmost {
        Some(pid) if pid != own_pid => Some(pid),
        _ => previous_app_pid.map(|p| p as u32),
    };

    resolve_claude_session_host(cwd, hint)
}

fn codex_cwd_signals(cwd: &str) -> (bool, bool) {
    let mut has_terminal_codex = false;
    let mut has_desktop_codex = false;
    for pid in pids_with_cwd(cwd) {
        if find_terminal_ancestor(pid).is_some() && is_codex_related_process(pid) {
            has_terminal_codex = true;
        } else if find_terminal_ancestor(pid).is_none() && is_in_codex_desktop_tree(pid) {
            has_desktop_codex = true;
        }
    }
    (has_terminal_codex, has_desktop_codex)
}

pub fn resolve_codex_session_host(cwd: &str, hint_pid: Option<u32>) -> SessionHost {
    if cwd.is_empty() || cwd == "." {
        return SessionHost::Unknown;
    }

    if let Some(pid) = hint_pid {
        if is_codex_desktop_app_pid(pid) {
            return SessionHost::CodexDesktop;
        }
        if is_terminal_pid(pid) {
            return SessionHost::CodexCli;
        }
    }

    let (has_terminal_codex, has_desktop_codex) = codex_cwd_signals(cwd);
    match (has_terminal_codex, has_desktop_codex) {
        (true, false) => return SessionHost::CodexCli,
        (false, true) => return SessionHost::CodexDesktop,
        (true, true) => return SessionHost::Unknown,
        (false, false) => {}
    }

    if frontmost_is_codex_desktop() {
        return SessionHost::CodexDesktop;
    }

    if frontmost_is_terminal() {
        return SessionHost::CodexCli;
    }

    SessionHost::Unknown
}

pub fn detect_codex_session_host(cwd: &str) -> SessionHost {
    resolve_codex_session_host(cwd, None)
}

pub fn detect_codex_session_host_at_hook(cwd: &str, previous_app_pid: Option<i64>) -> SessionHost {
    let own_pid = std::process::id();
    let frontmost = frontmost_app_pid();

    let hint = match frontmost {
        Some(pid) if pid != own_pid => Some(pid),
        _ => previous_app_pid.map(|p| p as u32),
    };

    resolve_codex_session_host(cwd, hint)
}

pub(crate) fn frontmost_is_codex_desktop() -> bool {
    frontmost_app_pid().is_some_and(is_codex_desktop_app_pid)
}

pub(crate) fn is_codex_desktop_app_running() -> bool {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };
        for bundle_id in CODEX_DESKTOP_BUNDLE_IDS {
            let bundle = objc2_foundation::NSString::from_str(bundle_id);
            let apps: *mut objc2::runtime::AnyObject = objc2::msg_send![
                cls,
                runningApplicationsWithBundleIdentifier: &*bundle
            ];
            if apps.is_null() {
                continue;
            }
            let count: usize = objc2::msg_send![apps, count];
            if count > 0 {
                return true;
            }
        }
        false
    }
}

fn is_in_codex_desktop_tree(mut pid: u32) -> bool {
    for _ in 0..32 {
        if pid <= 1 {
            return false;
        }
        if is_codex_desktop_process(pid) {
            return true;
        }
        let output = match super::command_output_with_timeout(
            Command::new("ps").args(["-p", &pid.to_string(), "-o", "ppid="]),
            std::time::Duration::from_secs(2),
        )
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

const CODEX_DESKTOP_BUNDLE_IDS: &[&str] = &["com.openai.codex"];

const CURSOR_BUNDLE_IDS: &[&str] = &["com.todesktop.230313mzl4w4u92"];

fn is_codex_desktop_bundle(bundle: &str) -> bool {
    CODEX_DESKTOP_BUNDLE_IDS.contains(&bundle)
}

fn command_line_matches_codex(command_line: &str) -> bool {
    let trimmed = command_line.trim();
    trimmed == "codex"
        || trimmed.ends_with("/codex")
        || trimmed.contains("/codex ")
        || trimmed.ends_with("/codex-cli")
        || command_line.contains("Codex.app")
        || command_line.contains("Codex Helper")
        || command_line.contains("com.openai.codex")
}

fn is_codex_related_process(pid: u32) -> bool {
    if is_codex_desktop_pid(pid) {
        return true;
    }
    if process_executable(pid).is_some_and(|comm| command_line_matches_codex(&comm)) {
        return true;
    }
    process_command_line(pid).is_some_and(|args| command_line_matches_codex(&args))
}

fn is_codex_desktop_process(pid: u32) -> bool {
    if find_terminal_ancestor(pid).is_some() {
        return false;
    }
    if is_codex_desktop_pid(pid) {
        return true;
    }
    if process_executable(pid).is_some_and(|comm| command_line_matches_codex(&comm)) {
        return true;
    }
    process_command_line(pid).is_some_and(|args| command_line_matches_codex(&args))
}

fn is_codex_desktop_app_pid(pid: u32) -> bool {
    if find_terminal_ancestor(pid).is_some() {
        return false;
    }
    is_codex_desktop_pid(pid) || is_in_codex_desktop_tree(pid)
}

fn is_codex_desktop_pid(pid: u32) -> bool {
    bundle_id_for_pid(pid as i32)
        .as_deref()
        .is_some_and(is_codex_desktop_bundle)
}

pub(crate) fn frontmost_is_claude_desktop() -> bool {
    frontmost_app_pid().is_some_and(is_claude_desktop_app_pid)
}

pub(crate) fn frontmost_is_terminal() -> bool {
    frontmost_app_pid().is_some_and(is_terminal_pid)
}

pub(crate) fn is_claude_desktop_app_running() -> bool {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSRunningApplication") else {
            return false;
        };
        for bundle_id in CLAUDE_DESKTOP_BUNDLE_IDS {
            let bundle = objc2_foundation::NSString::from_str(bundle_id);
            let apps: *mut objc2::runtime::AnyObject = objc2::msg_send![
                cls,
                runningApplicationsWithBundleIdentifier: &*bundle
            ];
            if apps.is_null() {
                continue;
            }
            let count: usize = objc2::msg_send![apps, count];
            if count > 0 {
                return true;
            }
        }
        false
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
        let output = match super::command_output_with_timeout(
            Command::new("ps").args(["-p", &pid.to_string(), "-o", "ppid="]),
            std::time::Duration::from_secs(2),
        )
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
    let output = match super::command_output_with_timeout(
        Command::new("lsof").args(["-d", "cwd", "+c", "0"]),
        std::time::Duration::from_secs(2),
    ) {
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

const CLAUDE_DESKTOP_BUNDLE_IDS: &[&str] =
    &["com.anthropic.claudefordesktop", "com.anthropic.claude"];

fn is_claude_desktop_bundle(bundle: &str) -> bool {
    CLAUDE_DESKTOP_BUNDLE_IDS.contains(&bundle)
}

fn process_executable(pid: u32) -> Option<String> {
    let output = super::command_output_with_timeout(
        Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]),
        std::time::Duration::from_secs(2),
    )
    .ok()?;
    let comm = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if comm.is_empty() {
        None
    } else {
        Some(comm)
    }
}

fn process_command_line(pid: u32) -> Option<String> {
    let output = super::command_output_with_timeout(
        Command::new("ps").args(["-p", &pid.to_string(), "-o", "args="]),
        std::time::Duration::from_secs(2),
    )
    .ok()?;
    let args = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

fn command_line_matches_claude(command_line: &str) -> bool {
    let trimmed = command_line.trim();
    trimmed == "claude"
        || trimmed.ends_with("/claude")
        || trimmed.contains("/claude ")
        || command_line.contains("Claude.app")
        || command_line.contains("Claude Helper")
        || command_line.contains("Claude-3p/claude-code")
        || command_line.contains("claude-code")
        || command_line.contains("@anthropic/claude")
}

fn is_claude_related_process(pid: u32) -> bool {
    if is_claude_desktop_pid(pid) {
        return true;
    }
    if process_executable(pid).is_some_and(|comm| command_line_matches_claude(&comm)) {
        return true;
    }
    process_command_line(pid).is_some_and(|args| command_line_matches_claude(&args))
}

fn is_claude_desktop_process(pid: u32) -> bool {
    if find_terminal_ancestor(pid).is_some() {
        return false;
    }
    if is_claude_desktop_pid(pid) {
        return true;
    }
    if process_executable(pid).is_some_and(|comm| command_line_matches_claude(&comm)) {
        return true;
    }
    process_command_line(pid).is_some_and(|args| command_line_matches_claude(&args))
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
        let bundle: *mut objc2_foundation::NSString = objc2::msg_send![running, bundleIdentifier];
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
        let output = super::command_output_with_timeout(
            Command::new("ps").args(["-p", &pid.to_string(), "-o", "ppid=,comm="]),
            std::time::Duration::from_secs(2),
        )
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

#[cfg(test)]
mod ime_overlay_tests {
    use super::ime_overlay::{
        ime_window_level, is_island_owned_window, island_window_level, should_order_island_front,
    };

    #[test]
    fn ime_candidate_level_is_above_the_island() {
        assert!(ime_window_level() > island_window_level());
    }

    #[test]
    fn skips_null_and_owned_windows() {
        assert!(is_island_owned_window(0, 10, 20));
        assert!(is_island_owned_window(10, 10, 20));
        assert!(is_island_owned_window(20, 10, 20));
        assert!(!is_island_owned_window(30, 10, 20));
        assert!(!is_island_owned_window(30, 10, 0));
    }

    #[test]
    fn orders_island_front_only_when_ime_is_idle() {
        assert!(should_order_island_front());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod live_probes {
    use super::*;

    #[test]
    #[ignore = "manual macOS integration probe; run with: cargo test live_probe -- --ignored --nocapture"]
    fn live_probe_session_host_detection() {
        let cwd = std::env::var("ATOLL_PROBE_CWD")
            .unwrap_or_else(|_| "/Users/yingguangshanshuo/code/Atoll".to_string());
        eprintln!("cwd: {cwd}");
        eprintln!(
            "frontmost_is_claude_desktop: {}",
            frontmost_is_claude_desktop()
        );
        eprintln!("frontmost_is_terminal: {}", frontmost_is_terminal());
        eprintln!(
            "detect_claude_session_host: {:?}",
            detect_claude_session_host(&cwd)
        );
        eprintln!(
            "detect_claude_session_host_at_hook: {:?}",
            detect_claude_session_host_at_hook(&cwd, None)
        );

        let cli_pid: u32 = std::env::var("ATOLL_PROBE_CLI_PID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3077);
        let desktop_pid: u32 = std::env::var("ATOLL_PROBE_DESKTOP_PID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(88646);

        eprintln!(
            "detect_session_host_from_peer_pid(CLI {}): {:?}",
            cli_pid,
            detect_session_host_from_peer_pid(cli_pid)
        );
        eprintln!(
            "detect_session_host_from_peer_pid(Desktop {}): {:?}",
            desktop_pid,
            detect_session_host_from_peer_pid(desktop_pid)
        );

        let codex_desktop_pid: u32 = std::env::var("ATOLL_PROBE_CODEX_DESKTOP_PID")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if codex_desktop_pid > 0 {
            eprintln!(
                "detect_codex_session_host_from_peer_pid(Desktop {}): {:?}",
                codex_desktop_pid,
                detect_codex_session_host_from_peer_pid(codex_desktop_pid)
            );
        }
        eprintln!(
            "detect_codex_session_host: {:?}",
            detect_codex_session_host(&cwd)
        );
        eprintln!(
            "detect_codex_session_host_at_hook: {:?}",
            detect_codex_session_host_at_hook(&cwd, None)
        );
    }
}
