// Bluetooth device battery commands.
use tauri::{AppHandle, State};

use crate::*;

/// Initial pull for the frontend. Applies the shared GATT cache so a
/// freshly (re)loaded webview sees the same levels the monitor emits —
/// without this, devices the OS can't report would show empty until the
/// next monitor-driven change. Async so the blocking GATT probe never runs
/// on the main thread (probe_device dispatches to main and waits).
#[tauri::command]
pub(crate) async fn get_bluetooth_battery(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bluetooth_battery::BluetoothBatteryReport, String> {
    let mut report = bluetooth_battery::fetch_bluetooth_battery();
    #[cfg(target_os = "macos")]
    let mut gatt_probe = |name: &str| gatt_battery::probe_device(&app, name);
    #[cfg(not(target_os = "macos"))]
    let mut gatt_probe = |_: &str| gatt_battery::ProbeOutcome::NoData;
    {
        let mut cache = lock_state(&state.gatt_battery_cache);
        cache.fill_missing(
            &mut report.devices,
            std::time::Instant::now(),
            &mut gatt_probe,
        );
    }
    Ok(report)
}

#[tauri::command]
pub(crate) fn get_bluetooth_battery_card_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.bluetooth_battery_card_enabled)
}

#[tauri::command]
pub(crate) fn set_bluetooth_battery_card_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    *lock_state(&state.bluetooth_battery_card_enabled) = enabled;
    persist_bluetooth_battery_card_enabled(enabled);
    enabled
}

#[tauri::command]
pub(crate) fn get_bluetooth_battery_alert_enabled(state: State<'_, AppState>) -> bool {
    *lock_state(&state.bluetooth_battery_alert_enabled)
}

#[tauri::command]
pub(crate) fn set_bluetooth_battery_alert_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    *lock_state(&state.bluetooth_battery_alert_enabled) = enabled;
    persist_bluetooth_battery_alert_enabled(enabled);
    enabled
}

#[tauri::command]
pub(crate) fn get_bluetooth_battery_alert_threshold(state: State<'_, AppState>) -> u8 {
    *lock_state(&state.bluetooth_battery_alert_threshold)
}

#[tauri::command]
pub(crate) fn set_bluetooth_battery_alert_threshold(
    state: State<'_, AppState>,
    threshold: u8,
) -> u8 {
    let clamped = bluetooth_battery::clamp_alert_threshold(threshold);
    *lock_state(&state.bluetooth_battery_alert_threshold) = clamped;
    persist_bluetooth_battery_alert_threshold(clamped);
    clamped
}
