//! Tests for the GATT probe bookkeeping: filling battery-less devices from
//! injected probe outcomes, TTL caching, the per-cycle budget, and
//! permission-denial disabling. The CoreBluetooth flow itself is not
//! unit-testable.

use std::time::{Duration, Instant};

use crate::bluetooth_battery::{BluetoothDeviceBattery, DEVICE_KIND_OTHER};
use crate::gatt_battery::{
    GattProbeCache, ProbeOutcome, GATT_PROBE_TTL, MAX_PROBE_DEVICES_PER_CYCLE,
};

fn device(name: &str, battery_percent: Option<u8>) -> BluetoothDeviceBattery {
    BluetoothDeviceBattery::new(
        format!("id:{name}"),
        name.to_string(),
        DEVICE_KIND_OTHER.to_string(),
        battery_percent,
        None,
        None,
        None,
    )
}

/// Counter-based probe: returns the outcomes in order (the last one repeats)
/// and records each call.
fn counting_probe(
    outcomes: Vec<ProbeOutcome>,
) -> (impl FnMut(&str) -> ProbeOutcome, impl Fn() -> usize) {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let queue = std::rc::Rc::new(std::cell::RefCell::new(outcomes));
    let probe = {
        let calls = calls.clone();
        let queue = queue.clone();
        move |_name: &str| {
            calls.set(calls.get() + 1);
            let mut queue = queue.borrow_mut();
            if queue.len() <= 1 {
                queue.first().copied().unwrap_or(ProbeOutcome::NoData)
            } else {
                queue.remove(0)
            }
        }
    };
    (probe, move || calls.get())
}

#[test]
fn fills_percent_into_battery_less_devices_only() {
    let mut devices = vec![
        device("Mi DMMS2", None),
        device("ROG AZOTH", None),
        device("AirPods Pro", Some(64)),
    ];
    let mut cache = GattProbeCache::default();
    let now = Instant::now();
    cache.fill_missing(&mut devices, now, &mut |name| {
        if name == "Mi DMMS2" {
            ProbeOutcome::Percent(73)
        } else {
            ProbeOutcome::NoData
        }
    });
    assert_eq!(devices[0].battery_percent, Some(73));
    assert_eq!(devices[1].battery_percent, None);
    assert_eq!(
        devices[2].battery_percent,
        Some(64),
        "with battery untouched"
    );
}

#[test]
fn fresh_results_are_not_probed_again_until_ttl_expires() {
    // A NoData outcome keeps the device battery-less, so the TTL path is
    // what decides whether the device is probed again (a Percent result
    // would fill the device and the has_battery check would hide it).
    let mut devices = vec![device("Mi DMMS2", None)];
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::NoData]);

    let start = Instant::now();
    cache.fill_missing(&mut devices, start, &mut probe);
    cache.fill_missing(&mut devices, start + Duration::from_secs(60), &mut probe);
    assert_eq!(calls(), 1, "still fresh inside the TTL");

    cache.fill_missing(
        &mut devices,
        start + GATT_PROBE_TTL + Duration::from_secs(1),
        &mut probe,
    );
    assert_eq!(calls(), 2, "stale entry probed again");
}

#[test]
fn empty_results_are_cached_too() {
    let mut devices = vec![device("Mi DMMS2", None)];
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::NoData]);
    let start = Instant::now();
    cache.fill_missing(&mut devices, start, &mut probe);
    cache.fill_missing(&mut devices, start + Duration::from_secs(60), &mut probe);
    assert_eq!(calls(), 1, "a device without the service is not hammered");
}

/// Regression: a cached level MUST be applied to every caller's devices, not
/// only to the caller that ran the probe. The bug made the ring vanish for
/// every frontend reload and every monitor cycle after the first probe.
#[test]
fn cached_percent_is_applied_to_later_callers() {
    let mut devices = vec![device("Mi DMMS2", None)];
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::Percent(79)]);

    cache.fill_missing(&mut devices, Instant::now(), &mut probe);
    assert_eq!(devices[0].battery_percent, Some(79));

    // A later caller (fresh webview pull, monitor cycle) hits the cache and
    // must receive the level without probing again.
    let mut fresh_devices = vec![device("Mi DMMS2", None)];
    cache.fill_missing(&mut fresh_devices, Instant::now(), &mut probe);
    assert_eq!(
        fresh_devices[0].battery_percent,
        Some(79),
        "cache hit applies the level"
    );
    assert_eq!(calls(), 1, "no re-probe while fresh");
}

/// A NoData entry expires after the short TTL, not the success TTL.
#[test]
fn no_data_expires_quickly_but_success_lasts() {
    let mut devices = vec![device("Mi DMMS2", None)];
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::NoData]);
    let start = Instant::now();
    cache.fill_missing(&mut devices, start, &mut probe);
    cache.fill_missing(
        &mut devices,
        start + crate::gatt_battery::NO_DATA_TTL + Duration::from_secs(1),
        &mut probe,
    );
    assert_eq!(calls(), 2, "empty result retried after the short TTL");
}

#[test]
fn permission_denial_disables_probing_for_the_process() {
    let mut devices = vec![device("Mi DMMS2", None), device("ROG AZOTH", None)];
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::PermissionDenied]);
    let start = Instant::now();
    cache.fill_missing(&mut devices, start, &mut probe);
    assert_eq!(calls(), 1, "stops at the first denial");
    cache.fill_missing(&mut devices, start + GATT_PROBE_TTL * 10, &mut probe);
    assert_eq!(calls(), 1, "disabled cache never probes again");
}

#[test]
fn probes_at_most_the_per_cycle_budget() {
    let mut devices: Vec<BluetoothDeviceBattery> = (0..10)
        .map(|index| device(&format!("Device {index}"), None))
        .collect();
    let mut cache = GattProbeCache::default();
    let (mut probe, calls) = counting_probe(vec![ProbeOutcome::NoData]);
    cache.fill_missing(&mut devices, Instant::now(), &mut probe);
    assert_eq!(calls(), MAX_PROBE_DEVICES_PER_CYCLE);
}
