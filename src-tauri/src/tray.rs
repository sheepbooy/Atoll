//! Menu-bar tray icon: Show / Quit menu and left-click activation.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

use super::{exit_atoll, platform, show_main_window};

pub(crate) fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let [(show_id, show_label), (quit_id, quit_label)] = tray_menu_entries();
    let show = MenuItem::with_id(app, show_id, show_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, quit_id, quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::new()
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
        });

    if let Some(icon) = platform::tray_icon(app) {
        builder = builder.icon(icon);
    }

    builder.build(app)?;

    Ok(())
}

pub(crate) fn tray_menu_entries() -> [(&'static str, &'static str); 2] {
    [("show", "Show Atoll"), ("quit", "Quit")]
}
