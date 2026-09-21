//! Tests for the bluetooth_battery module: fixture parsing for both data
//! sources, the merge, and the low-battery alert bookkeeping.

use std::collections::HashSet;

use super::bluetooth_battery::*;

/// Real system_profiler output shape (macOS 26) with the battery keys
/// connected Apple devices carry, plus a battery-less BLE device and a
/// not-connected device that must both stay out of the report.
const SYSTEM_PROFILER_FIXTURE: &str = r#"{
  "SPBluetoothDataType" : [
    {
      "controller_properties" : {
        "controller_address" : "1C:F6:4C:51:7F:61",
        "controller_state" : "attrib_on"
      },
      "device_connected" : [
        {
          "AirPods Pro" : {
            "device_address" : "AA:BB:CC:DD:EE:01",
            "device_minorType" : "Headphones",
            "device_batteryLevelCase" : "80%",
            "device_batteryLevelLeft" : "90%",
            "device_batteryLevelRight" : "35%"
          }
        },
        {
          "Magic Mouse" : {
            "device_address" : "AA-BB-CC-DD-EE-02",
            "device_minorType" : "Mouse",
            "device_batteryLevelMain" : "73%"
          }
        },
        {
          "Mi DMMS2" : {
            "device_address" : "D1:02:5B:57:DB:18",
            "device_minorType" : "Mouse",
            "device_services" : "0x400020 < HID BLE >"
          }
        }
      ],
      "device_not_connected" : [
        {
          "羊仔的iPad" : {
            "device_address" : "7C:2A:CA:56:D8:00",
            "device_batteryLevelMain" : "50%"
          }
        }
      ]
    }
  ]
}"#;

/// ioreg -r -l -c AppleDeviceManagementHIDEventService shape: the Magic
/// Keyboard record carries a level, the second record has none and must be
/// dropped. Note the pipe-prefixed property lines and the hyphenated address.
const IOREG_FIXTURE: &str = r#"+-o AppleDeviceManagementHIDEventService  <class AppleDeviceManagementHIDEventService, id 0x100000abc, registered, matched, active, busy 0 (0 ms), retain 6>
    | {
    |   "BatteryPercent" = 91
    |   "BatteryStatus" = 0
    |   "DeviceAddress" = "aa-bb-cc-dd-ee-05"
    |   "Product" = "Magic Keyboard with Touch ID"
    |   "Transport" = "Bluetooth"
    | }
    
+-o AppleDeviceManagementHIDEventService  <class AppleDeviceManagementHIDEventService, id 0x100000abd, registered, matched, active, busy 0 (0 ms), retain 6>
    | {
    |   "Product" = "Magic Trackpad"
    |   "DeviceAddress" = "aa-bb-cc-dd-ee-06"
    | }
"#;

fn device(id: &str, name: &str, battery: Option<u8>) -> BluetoothDeviceBattery {
    BluetoothDeviceBattery::new(
        id.into(),
        name.into(),
        DEVICE_KIND_OTHER.into(),
        battery,
        None,
        None,
        None,
    )
}

#[test]
fn parses_system_profiler_fixture() {
    let devices = parse_system_profiler_json(SYSTEM_PROFILER_FIXTURE);
    assert_eq!(
        devices.len(),
        3,
        "all connected devices kept (battery-less feed the GATT probe); not-connected excluded"
    );
    let mi = devices.iter().find(|d| d.name == "Mi DMMS2").unwrap();
    assert_eq!(mi.battery_percent, None);
    assert_eq!(mi.kind, DEVICE_KIND_MOUSE);

    let airpods = devices.iter().find(|d| d.name == "AirPods Pro").unwrap();
    assert_eq!(airpods.id, "aa:bb:cc:dd:ee:01");
    assert_eq!(airpods.kind, DEVICE_KIND_HEADPHONES);
    assert_eq!(airpods.battery_percent, None);
    assert_eq!(airpods.case_percent, Some(80));
    assert_eq!(airpods.left_percent, Some(90));
    assert_eq!(airpods.right_percent, Some(35));

    let mouse = devices.iter().find(|d| d.name == "Magic Mouse").unwrap();
    assert_eq!(mouse.id, "aa:bb:cc:dd:ee:02");
    assert_eq!(mouse.kind, DEVICE_KIND_MOUSE);
    assert_eq!(mouse.battery_percent, Some(73));
}

#[test]
fn parses_ioreg_fixture() {
    let devices = parse_ioreg_output(IOREG_FIXTURE);
    assert_eq!(devices.len(), 1, "record without BatteryPercent dropped");
    let keyboard = &devices[0];
    assert_eq!(keyboard.name, "Magic Keyboard with Touch ID");
    assert_eq!(
        keyboard.id, "aa:bb:cc:dd:ee:05",
        "hyphen address normalized"
    );
    assert_eq!(keyboard.battery_percent, Some(91));
}

#[test]
fn parses_numeric_battery_levels() {
    let json = r#"{
      "SPBluetoothDataType": [{
        "device_connected": [{
          "Old Magic Mouse": {
            "device_minorType": "Mouse",
            "device_batteryLevelMain": 64
          }
        }]
      }]
    }"#;
    let devices = parse_system_profiler_json(json);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].battery_percent, Some(64));
}

#[test]
fn parse_battery_level_text_accepts_common_shapes() {
    assert_eq!(parse_battery_level_text("85%"), Some(85));
    assert_eq!(parse_battery_level_text(" 9 %"), Some(9));
    assert_eq!(parse_battery_level_text("0"), Some(0));
    assert_eq!(parse_battery_level_text("100%"), Some(100));
    assert_eq!(parse_battery_level_text("101%"), None);
    assert_eq!(parse_battery_level_text("full"), None);
    assert_eq!(parse_battery_level_text(""), None);
}

#[test]
fn merge_fills_missing_battery_and_appends_new_devices() {
    let primary = parse_system_profiler_json(SYSTEM_PROFILER_FIXTURE);
    // ioreg knows "Mi DMMS2" (battery-less in system_profiler) and has a
    // second, unknown record. Address formats differ but normalize equal.
    let mut secondary = parse_ioreg_output(
        r#"+-o IOBluetoothDevice  <class IOBluetoothDevice, id 0x1, registered, matched, active, busy 0 (0 ms), retain 5>
    | {
    |   "BatteryPercent" = 64
    |   "DeviceAddress" = "d1-02-5b-57-db-18"
    |   "Name" = "Mi DMMS2"
    | }
"#,
    );
    secondary.push(device("xx:xx:xx:xx:xx:09", "Solo Pad", Some(12)));

    let merged = merge_devices(primary, secondary);
    let mi = merged.iter().find(|d| d.name == "Mi DMMS2").unwrap();
    assert_eq!(mi.battery_percent, Some(64));
    // "Mi DMMS2" IS in the primary list now (parser keeps battery-less
    // devices), so the system_profiler-inferred kind wins over the ioreg
    // record's name-only inference.
    assert_eq!(mi.kind, DEVICE_KIND_MOUSE);
    assert!(merged.iter().any(|d| d.name == "Solo Pad"));
    // Sorted by name for stable rendering.
    let names: Vec<&str> = merged.iter().map(|d| d.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_by_key(|n| n.to_lowercase());
    assert_eq!(names, sorted);
}

#[test]
fn merge_keeps_primary_battery_when_both_report() {
    let primary = vec![device("aa:bb:cc:dd:ee:02", "Magic Mouse", Some(73))];
    let secondary = vec![device("aa:bb:cc:dd:ee:02", "Magic Mouse", Some(99))];
    let merged = merge_devices(primary, secondary);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].battery_percent, Some(73));
}

#[test]
fn effective_battery_uses_lowest_reported_level() {
    let airpods = BluetoothDeviceBattery::new(
        "a".into(),
        "AirPods Pro".into(),
        DEVICE_KIND_HEADPHONES.into(),
        None,
        Some(80),
        Some(90),
        Some(35),
    );
    assert_eq!(effective_battery_percent(&airpods), Some(35));
    let silent = device("b", "No Data", None);
    assert_eq!(effective_battery_percent(&silent), None);
}

#[test]
fn low_battery_alerts_fire_once_until_recovery() {
    let devices = vec![
        device("a", "Magic Mouse", Some(15)),
        device("b", "Keyboard", Some(40)),
        BluetoothDeviceBattery::new(
            "c".into(),
            "AirPods".into(),
            DEVICE_KIND_HEADPHONES.into(),
            None,
            Some(10),
            None,
            None,
        ),
    ];
    let mut alerted = HashSet::new();

    let first = collect_low_battery_alerts(&devices, 20, &mut alerted);
    assert_eq!(
        first,
        vec![("Magic Mouse".to_string(), 15), ("AirPods".to_string(), 10)]
    );
    let second = collect_low_battery_alerts(&devices, 20, &mut alerted);
    assert!(second.is_empty(), "no repeat alerts while still low");

    let recovered = vec![device("a", "Magic Mouse", Some(55))];
    collect_low_battery_alerts(&recovered, 20, &mut alerted);
    let low_again = vec![device("a", "Magic Mouse", Some(10))];
    let re_alert = collect_low_battery_alerts(&low_again, 20, &mut alerted);
    assert_eq!(
        re_alert,
        vec![("Magic Mouse".to_string(), 10)],
        "recovery re-arms"
    );
}

#[test]
fn low_battery_alerts_drop_stale_devices() {
    let mut alerted: HashSet<String> = HashSet::from(["gone".to_string()]);
    let devices = vec![device("a", "Magic Mouse", Some(80))];
    collect_low_battery_alerts(&devices, 20, &mut alerted);
    assert!(!alerted.contains("gone"), "vanished devices cleared");
}

#[test]
fn normalize_address_and_ids() {
    assert_eq!(normalize_address("D1-02-5B-57-DB-18"), "d1:02:5b:57:db:18");
    assert_eq!(make_id(Some("AA:BB"), "X"), "aa:bb");
    assert_eq!(make_id(None, "Magic Mouse"), "name:magic mouse");
    assert_eq!(make_id(Some("  "), "X"), "name:x");
}

#[test]
fn infer_kind_matches_common_names() {
    assert_eq!(infer_kind(Some("Mouse"), ""), DEVICE_KIND_MOUSE);
    assert_eq!(infer_kind(None, "Magic Keyboard"), DEVICE_KIND_KEYBOARD);
    assert_eq!(infer_kind(None, "Magic Trackpad"), DEVICE_KIND_TRACKPAD);
    assert_eq!(infer_kind(Some("Headphones"), ""), DEVICE_KIND_HEADPHONES);
    assert_eq!(infer_kind(None, "AirPods Pro"), DEVICE_KIND_HEADPHONES);
    assert_eq!(infer_kind(None, "Solo Pad"), DEVICE_KIND_OTHER);
}

#[test]
fn alert_threshold_clamps_to_supported_range() {
    assert_eq!(clamp_alert_threshold(0), MIN_ALERT_THRESHOLD);
    assert_eq!(clamp_alert_threshold(20), 20);
    assert_eq!(clamp_alert_threshold(200), MAX_ALERT_THRESHOLD);
}

#[test]
fn report_serializes_with_camel_case() {
    let report = BluetoothBatteryReport {
        devices: vec![BluetoothDeviceBattery::new(
            "aa".into(),
            "Magic Mouse".into(),
            DEVICE_KIND_MOUSE.into(),
            Some(73),
            None,
            None,
            None,
        )],
    };
    let json = serde_json::to_string(&report).unwrap();
    assert!(json.contains("\"batteryPercent\":73"));
    assert!(json.contains("\"devices\":"));
    assert!(!json.contains("battery_percent"));
}
