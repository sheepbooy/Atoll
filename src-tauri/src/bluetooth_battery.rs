//! Battery levels for connected Bluetooth devices, without linking
//! CoreBluetooth (so the app never triggers the macOS Bluetooth permission
//! prompt). On macOS we shell out to `system_profiler -json
//! SPBluetoothDataType` — which covers AirPods (buds + case) and most
//! connected devices — and to `ioreg` for the Magic Mouse / Keyboard /
//! Trackpad HID devices system_profiler sometimes omits. The two views are
//! merged by address or name. Pure parsing logic is compiled on every
//! platform so the unit tests run anywhere; the system calls are
//! cfg(target_os = "macos") and every other platform reports an empty
//! report (the frontend card stays hidden).

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub(crate) const DEVICE_KIND_MOUSE: &str = "mouse";
pub(crate) const DEVICE_KIND_KEYBOARD: &str = "keyboard";
pub(crate) const DEVICE_KIND_TRACKPAD: &str = "trackpad";
pub(crate) const DEVICE_KIND_HEADPHONES: &str = "headphones";
pub(crate) const DEVICE_KIND_OTHER: &str = "other";

/// Default low-battery alert threshold, percent.
pub(crate) const DEFAULT_ALERT_THRESHOLD: u8 = 20;
pub(crate) const MIN_ALERT_THRESHOLD: u8 = 1;
pub(crate) const MAX_ALERT_THRESHOLD: u8 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BluetoothDeviceBattery {
    /// Stable identity across polls: MAC address when known, else the name.
    pub(crate) id: String,
    pub(crate) name: String,
    /// One of the DEVICE_KIND_* constants.
    pub(crate) kind: String,
    /// Main device battery; None when the source does not report one
    /// (e.g. AirPods only report per-bud and case levels).
    pub(crate) battery_percent: Option<u8>,
    pub(crate) case_percent: Option<u8>,
    pub(crate) left_percent: Option<u8>,
    pub(crate) right_percent: Option<u8>,
}

impl BluetoothDeviceBattery {
    pub(crate) fn new(
        id: String,
        name: String,
        kind: String,
        battery_percent: Option<u8>,
        case_percent: Option<u8>,
        left_percent: Option<u8>,
        right_percent: Option<u8>,
    ) -> Self {
        Self {
            id,
            name,
            kind,
            battery_percent,
            case_percent,
            left_percent,
            right_percent,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BluetoothBatteryReport {
    pub(crate) devices: Vec<BluetoothDeviceBattery>,
}

/// "85%", "85", "85 %"-style system_profiler battery strings → 0..=100.
pub(crate) fn parse_battery_level_text(value: &str) -> Option<u8> {
    let digits: String = value
        .trim()
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u8>().ok().filter(|percent| *percent <= 100)
}

/// system_profiler battery keys arrive as strings ("85%") on current macOS
/// but have been numeric on older releases; accept both.
pub(crate) fn parse_battery_level_value(value: &Value) -> Option<u8> {
    match value {
        Value::String(text) => parse_battery_level_text(text),
        Value::Number(number) => number
            .as_u64()
            .and_then(|percent| u8::try_from(percent).ok())
            .filter(|percent| *percent <= 100),
        _ => None,
    }
}

/// Normalize an address so the two sources compare equal:
/// "D1:02:5B:57:DB:18" and "d1-02-5b-57-db-18" → "d1:02:5b:57:db:18".
pub(crate) fn normalize_address(address: &str) -> String {
    address.trim().to_lowercase().replace('-', ":")
}

/// Identity used for change detection and alert bookkeeping.
pub(crate) fn make_id(address: Option<&str>, name: &str) -> String {
    match address.map(normalize_address).filter(|a| !a.is_empty()) {
        Some(address) => address,
        None => format!("name:{}", name.to_lowercase()),
    }
}

pub(crate) fn infer_kind(minor_type: Option<&str>, name: &str) -> String {
    let hay = format!("{} {}", minor_type.unwrap_or(""), name).to_lowercase();
    if hay.contains("mouse") {
        DEVICE_KIND_MOUSE.into()
    } else if hay.contains("keyboard") {
        DEVICE_KIND_KEYBOARD.into()
    } else if hay.contains("trackpad") {
        DEVICE_KIND_TRACKPAD.into()
    } else if hay.contains("headphone")
        || hay.contains("headset")
        || hay.contains("earphone")
        || hay.contains("earbud")
        || hay.contains("airpod")
    {
        DEVICE_KIND_HEADPHONES.into()
    } else {
        DEVICE_KIND_OTHER.into()
    }
}

#[derive(Deserialize)]
struct SystemProfilerRoot {
    #[serde(rename = "SPBluetoothDataType", default)]
    bluetooth: Vec<SystemProfilerEntry>,
}

#[derive(Deserialize)]
struct SystemProfilerEntry {
    #[serde(rename = "device_connected", default)]
    connected: Vec<std::collections::HashMap<String, SystemProfilerDevice>>,
}

#[derive(Deserialize, Default)]
struct SystemProfilerDevice {
    #[serde(rename = "device_address", default)]
    address: Option<String>,
    #[serde(rename = "device_minorType", default)]
    minor_type: Option<String>,
    #[serde(rename = "device_batteryLevelMain", default)]
    battery_main: Option<Value>,
    #[serde(rename = "device_batteryLevelCase", default)]
    battery_case: Option<Value>,
    #[serde(rename = "device_batteryLevelLeft", default)]
    battery_left: Option<Value>,
    #[serde(rename = "device_batteryLevelRight", default)]
    battery_right: Option<Value>,
}

/// Parse the stdout of `system_profiler -json SPBluetoothDataType`. Every
/// connected device is kept — battery-less entries included — because they
/// are exactly the devices the GATT probe targets (a device the OS reports a
/// level for never needs a direct read); the UI decides what to show.
pub(crate) fn parse_system_profiler_json(stdout: &str) -> Vec<BluetoothDeviceBattery> {
    let Ok(root) = serde_json::from_str::<SystemProfilerRoot>(stdout) else {
        return Vec::new();
    };
    let mut devices = Vec::new();
    for entry in root.bluetooth {
        for named in entry.connected {
            for (name, device) in named {
                let battery_percent = device
                    .battery_main
                    .as_ref()
                    .and_then(parse_battery_level_value);
                let case_percent = device
                    .battery_case
                    .as_ref()
                    .and_then(parse_battery_level_value);
                let left_percent = device
                    .battery_left
                    .as_ref()
                    .and_then(parse_battery_level_value);
                let right_percent = device
                    .battery_right
                    .as_ref()
                    .and_then(parse_battery_level_value);
                let kind = infer_kind(device.minor_type.as_deref(), &name);
                devices.push(BluetoothDeviceBattery::new(
                    make_id(device.address.as_deref(), &name),
                    name,
                    kind,
                    battery_percent,
                    case_percent,
                    left_percent,
                    right_percent,
                ));
            }
        }
    }
    devices
}

/// Pull `"Key" = value` lines out of one ioreg record (the text between two
/// `+-o` root markers). Returns (name, address, battery_percent).
fn ioreg_record_fields(record: &str) -> Option<(String, Option<String>, Option<u8>)> {
    let mut name: Option<String> = None;
    let mut address: Option<String> = None;
    let mut battery: Option<u8> = None;
    for line in record.lines() {
        let line = line.trim().trim_start_matches("| ").trim();
        let Some((key, value)) = line.split_once(" = ") else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        if name.is_none() && (key == "\"Product\"" || key == "\"Name\"") {
            let cleaned = value.trim_matches('"');
            if !cleaned.is_empty() {
                name = Some(cleaned.to_string());
            }
        } else if address.is_none() && key == "\"DeviceAddress\"" {
            let cleaned = value.trim_matches('"');
            if !cleaned.is_empty() {
                address = Some(cleaned.to_string());
            }
        } else if battery.is_none() && key == "\"BatteryPercent\"" {
            battery = parse_battery_level_text(value);
        }
    }
    let name = name?;
    Some((name, address, battery))
}

/// Parse `ioreg -r -l -c <class>` text output. Only records that carry a
/// BatteryPercent are returned — this source exists to add battery data, the
/// full device list comes from system_profiler.
pub(crate) fn parse_ioreg_output(stdout: &str) -> Vec<BluetoothDeviceBattery> {
    let mut devices = Vec::new();
    for record in stdout.split("+-o ").skip(1) {
        let Some((name, address, battery)) = ioreg_record_fields(record) else {
            continue;
        };
        let Some(battery) = battery else {
            continue;
        };
        let kind = infer_kind(None, &name);
        devices.push(BluetoothDeviceBattery::new(
            make_id(address.as_deref(), &name),
            name,
            kind,
            Some(battery),
            None,
            None,
            None,
        ));
    }
    devices
}

fn same_device(a: &BluetoothDeviceBattery, b: &BluetoothDeviceBattery) -> bool {
    a.id.eq_ignore_ascii_case(&b.id) || a.name.eq_ignore_ascii_case(&b.name)
}

/// Merge the system_profiler view with the ioreg view: fill missing battery
/// fields on matching devices, append devices only ioreg knows, and sort by
/// name for stable rendering.
pub(crate) fn merge_devices(
    mut primary: Vec<BluetoothDeviceBattery>,
    secondary: Vec<BluetoothDeviceBattery>,
) -> Vec<BluetoothDeviceBattery> {
    for extra in secondary {
        if let Some(existing) = primary.iter_mut().find(|d| same_device(d, &extra)) {
            if existing.battery_percent.is_none() {
                existing.battery_percent = extra.battery_percent;
            }
            if existing.case_percent.is_none() {
                existing.case_percent = extra.case_percent;
            }
            if existing.left_percent.is_none() {
                existing.left_percent = extra.left_percent;
            }
            if existing.right_percent.is_none() {
                existing.right_percent = extra.right_percent;
            }
        } else {
            primary.push(extra);
        }
    }
    primary.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    primary
}

/// Lowest reported level across main/buds/case — what the low-battery alert
/// watches.
pub(crate) fn effective_battery_percent(device: &BluetoothDeviceBattery) -> Option<u8> {
    [
        device.battery_percent,
        device.case_percent,
        device.left_percent,
        device.right_percent,
    ]
    .into_iter()
    .flatten()
    .min()
}

/// Advance the alert bookkeeping for one poll: devices at or below
/// `threshold` alert once (until they recover above the threshold), devices
/// that recovered or vanished clear their flag. Returns the (name, percent)
/// pairs to notify now, percent being the lowest reported level.
pub(crate) fn collect_low_battery_alerts(
    devices: &[BluetoothDeviceBattery],
    threshold: u8,
    alerted: &mut HashSet<String>,
) -> Vec<(String, u8)> {
    let mut names = Vec::new();
    let mut present: HashSet<String> = HashSet::new();
    for device in devices {
        present.insert(device.id.clone());
        match effective_battery_percent(device) {
            Some(percent) if percent <= threshold => {
                if alerted.insert(device.id.clone()) {
                    names.push((device.name.clone(), percent));
                }
            }
            _ => {
                alerted.remove(&device.id);
            }
        }
    }
    alerted.retain(|id| present.contains(id));
    names
}

pub(crate) fn clamp_alert_threshold(threshold: u8) -> u8 {
    threshold.clamp(MIN_ALERT_THRESHOLD, MAX_ALERT_THRESHOLD)
}

/// Fetch the merged battery report. Empty on platforms without a source.
pub(crate) fn fetch_bluetooth_battery() -> BluetoothBatteryReport {
    #[cfg(target_os = "macos")]
    {
        fetch_macos()
    }
    #[cfg(not(target_os = "macos"))]
    {
        BluetoothBatteryReport::default()
    }
}

#[cfg(target_os = "macos")]
fn run_command(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(target_os = "macos")]
fn fetch_macos() -> BluetoothBatteryReport {
    let profiler =
        run_command("system_profiler", &["-json", "SPBluetoothDataType"]).unwrap_or_default();
    let devices = parse_system_profiler_json(&profiler);

    // Magic Mouse / Keyboard / Trackpad expose battery via the HID event
    // service; classic Bluetooth devices (some headphones) via IOBluetoothDevice.
    let mut ioreg_devices = parse_ioreg_output(
        &run_command(
            "ioreg",
            &["-r", "-l", "-c", "AppleDeviceManagementHIDEventService"],
        )
        .unwrap_or_default(),
    );
    ioreg_devices.extend(parse_ioreg_output(
        &run_command("ioreg", &["-r", "-l", "-c", "IOBluetoothDevice"]).unwrap_or_default(),
    ));
    BluetoothBatteryReport {
        devices: merge_devices(devices, ioreg_devices),
    }
}
