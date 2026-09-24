import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";

export type BluetoothDeviceKind =
  | "mouse"
  | "keyboard"
  | "trackpad"
  | "headphones"
  | "other";

export interface BluetoothDeviceBattery {
  id: string;
  name: string;
  kind: BluetoothDeviceKind;
  batteryPercent: number | null;
  casePercent: number | null;
  leftPercent: number | null;
  rightPercent: number | null;
}

export interface BluetoothBatteryReport {
  devices: BluetoothDeviceBattery[];
}

export async function getBluetoothBattery(): Promise<BluetoothBatteryReport> {
  if (!isTauriRuntime()) {
    return { devices: [] };
  }
  return invoke<BluetoothBatteryReport>("get_bluetooth_battery");
}

export async function onBluetoothBatteryChanged(
  callback: (report: BluetoothBatteryReport | null) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<BluetoothBatteryReport | null>(
    "bluetooth-battery-changed",
    (event) => callback(event.payload),
  );
}

export async function getBluetoothBatteryCardEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return true;
  }
  return invoke<boolean>("get_bluetooth_battery_card_enabled");
}

export async function setBluetoothBatteryCardEnabled(
  enabled: boolean,
): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_bluetooth_battery_card_enabled", { enabled });
}

export async function getBluetoothBatteryAlertEnabled(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return true;
  }
  return invoke<boolean>("get_bluetooth_battery_alert_enabled");
}

export async function setBluetoothBatteryAlertEnabled(
  enabled: boolean,
): Promise<boolean> {
  if (!isTauriRuntime()) {
    return enabled;
  }
  return invoke<boolean>("set_bluetooth_battery_alert_enabled", { enabled });
}

export async function getBluetoothBatteryAlertThreshold(): Promise<number> {
  if (!isTauriRuntime()) {
    return 20;
  }
  return invoke<number>("get_bluetooth_battery_alert_threshold");
}

export async function setBluetoothBatteryAlertThreshold(
  threshold: number,
): Promise<number> {
  if (!isTauriRuntime()) {
    return threshold;
  }
  return invoke<number>("set_bluetooth_battery_alert_threshold", { threshold });
}
