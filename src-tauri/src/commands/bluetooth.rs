// Bluetooth device battery commands.
use tauri::State;

use crate::*;

#[tauri::command]
pub(crate) fn get_bluetooth_battery() -> bluetooth_battery::BluetoothBatteryReport {
    bluetooth_battery::fetch_bluetooth_battery()
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
