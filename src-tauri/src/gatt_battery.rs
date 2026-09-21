//! GATT battery probe for connected BLE devices that macOS itself doesn't
//! report a level for (third-party HID peripherals like gaming mice and
//! keyboards). Uses CoreBluetooth to find system-connected peripherals that
//! expose the standard Battery Service (0x180F), connect to them, and read
//! the Battery Level characteristic (0x2A19).
//!
//! Unlike the system_profiler/ioreg sources this needs the app to use the
//! Bluetooth API, so the first probe triggers the one-time macOS Bluetooth
//! permission prompt (`NSBluetoothAlwaysUsageDescription` in Info.plist).
//! Denial (or the service being absent on the device) degrades gracefully:
//! the caller caches the empty result and retries after
//! [`GATT_PROBE_TTL`].
//!
//! Threading: the delegate and all CoreBluetooth calls live on the main
//! thread (callbacks dispatch to the main queue, which Tauri's run loop
//! pumps). The monitor thread kicks each probe off via
//! `AppHandle::run_on_main_thread` and blocks on a channel with a timeout.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::bluetooth_battery::BluetoothDeviceBattery;

/// How long a probe result (success or failure) is trusted before the device
/// is probed again.
pub(crate) const GATT_PROBE_TTL: Duration = Duration::from_secs(10 * 60);
/// One probe per poll at most this many battery-less devices, so a handful
/// of stubborn devices can't stretch a cycle out.
pub(crate) const MAX_PROBE_DEVICES_PER_CYCLE: usize = 4;

/// Result of a single GATT probe for one device.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    Percent(u8),
    NoData,
    PermissionDenied,
}

#[derive(Debug)]
struct CachedProbe {
    percent: Option<u8>,
    at: Instant,
}

/// Thread-local (monitor-thread) bookkeeping so each battery-less device is
/// GATT-probed at most once per TTL.
#[derive(Default)]
pub(crate) struct GattProbeCache {
    entries: HashMap<String, CachedProbe>,
    disabled: bool,
}

impl GattProbeCache {
    /// Probe battery-less devices through `probe` and fill the results into
    /// `devices`. `now` is injectable for tests. Probing stops early once
    /// `PermissionDenied` is seen (cached forever within this process).
    pub(crate) fn fill_missing<F: FnMut(&str) -> ProbeOutcome>(
        &mut self,
        devices: &mut [BluetoothDeviceBattery],
        now: Instant,
        probe: &mut F,
    ) {
        if self.disabled {
            return;
        }
        self.entries
            .retain(|_, cached| now.duration_since(cached.at) < GATT_PROBE_TTL);
        let mut budget = MAX_PROBE_DEVICES_PER_CYCLE;
        let mut denied = false;
        for device in devices.iter_mut() {
            if budget == 0 || denied {
                break;
            }
            let has_battery = device.battery_percent.is_some()
                || device.case_percent.is_some()
                || device.left_percent.is_some()
                || device.right_percent.is_some();
            if has_battery || self.entries.contains_key(&device.name.to_lowercase()) {
                continue;
            }
            let percent = match probe(&device.name) {
                ProbeOutcome::Percent(percent) => Some(percent),
                ProbeOutcome::NoData => None,
                ProbeOutcome::PermissionDenied => {
                    denied = true;
                    None
                }
            };
            self.entries
                .insert(device.name.to_lowercase(), CachedProbe { percent, at: now });
            if let Some(percent) = percent {
                device.battery_percent = Some(percent);
            }
            budget -= 1;
        }
        if denied {
            self.disabled = true;
        }
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::cell::RefCell;
    use std::sync::mpsc::{channel, Sender};
    use std::sync::OnceLock;
    use std::time::Duration;

    use objc2::rc::Retained;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, AnyThread, ClassType};
    use objc2_core_bluetooth::{
        CBCentralManager, CBCentralManagerDelegate, CBCharacteristic, CBManagerState, CBPeripheral,
        CBPeripheralDelegate, CBService, CBUUID,
    };
    use objc2_foundation::{NSArray, NSError, NSString};
    use tauri::AppHandle;

    use super::ProbeOutcome;

    const SERVICE_UUID: &str = "180F";
    const CHARACTERISTIC_UUID: &str = "2A19";
    /// Generous bound for connect + discover + read on a busy HID device.
    const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

    enum GattEvent {
        Percent(u8),
        NoData,
        PermissionDenied,
    }

    /// One probe in flight. CoreBluetooth objects are main-thread-only and
    /// every accessor here (delegate callbacks, probe kickoff, timeout reset)
    /// runs on the main thread via `run_on_main_thread` or the main queue.
    #[derive(Default)]
    struct GattSession {
        events: Option<Sender<GattEvent>>,
        target: Option<String>,
        central: Option<Retained<CBCentralManager>>,
        connecting: Option<Retained<CBPeripheral>>,
    }

    thread_local! {
        static SESSION: RefCell<Option<GattSession>> = const { RefCell::new(None) };
    }

    static DELEGATE: OnceLock<Retained<GattDelegate>> = OnceLock::new();

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "AtollGattDelegate"]
        struct GattDelegate;

        unsafe impl NSObjectProtocol for GattDelegate {}

        unsafe impl CBCentralManagerDelegate for GattDelegate {
            #[unsafe(method(centralManagerDidUpdateState:))]
            unsafe fn centralManagerDidUpdateState(&self, central: &CBCentralManager) {
                eprintln!("[Atoll] gatt: central state now {:?}", central.state().0);
                SESSION.with_borrow_mut(|session| {
                    let Some(state) = session.as_mut() else {
                        return;
                    };
                    match central.state() {
                        CBManagerState::PoweredOn => {
                            let target = state.target.clone();
                            if let Some(target) = target {
                                unsafe { start_probe_locked(state, &target) };
                            }
                        }
                        CBManagerState::Unauthorized | CBManagerState::Unsupported => {
                            finish_locked(state, GattEvent::PermissionDenied);
                        }
                        CBManagerState::PoweredOff => finish_locked(state, GattEvent::NoData),
                        _ => {}
                    }
                });
            }

            #[unsafe(method(centralManager:didConnectPeripheral:))]
            unsafe fn centralManager_didConnectPeripheral(
                &self,
                _central: &CBCentralManager,
                peripheral: &CBPeripheral,
            ) {
                eprintln!("[Atoll] gatt: didConnect");
                let delegate: &ProtocolObject<dyn CBPeripheralDelegate> =
                    ProtocolObject::from_ref(self);
                SESSION.with_borrow_mut(|session| {
                    let Some(state) = session.as_mut() else {
                        return;
                    };
                    if state.connecting.is_none() {
                        return;
                    }
                    unsafe {
                        peripheral.setDelegate(Some(delegate));
                        peripheral.discoverServices(Some(&NSArray::from_retained_slice(&[
                            battery_uuid(SERVICE_UUID),
                        ])));
                    }
                });
            }

            #[unsafe(method(centralManager:didFailToConnectPeripheral:error:))]
            unsafe fn centralManager_didFailToConnectPeripheral_error(
                &self,
                _central: &CBCentralManager,
                _peripheral: &CBPeripheral,
                _error: Option<&NSError>,
            ) {
                eprintln!("[Atoll] gatt: didFailToConnect");
                SESSION.with_borrow_mut(|session| {
                    let Some(state) = session.as_mut() else {
                        return;
                    };
                    if state.connecting.take().is_some() {
                        finish_locked(state, GattEvent::NoData);
                    }
                });
            }

            #[unsafe(method(centralManager:didDisconnectPeripheral:error:))]
            unsafe fn centralManager_didDisconnectPeripheral_error(
                &self,
                _central: &CBCentralManager,
                _peripheral: &CBPeripheral,
                _error: Option<&NSError>,
            ) {
                eprintln!("[Atoll] gatt: didDisconnect");
                SESSION.with_borrow_mut(|session| {
                    let Some(state) = session.as_mut() else {
                        return;
                    };
                    if state.connecting.take().is_some() {
                        finish_locked(state, GattEvent::NoData);
                    }
                });
            }
        }

        unsafe impl CBPeripheralDelegate for GattDelegate {
            #[unsafe(method(peripheral:didDiscoverServices:))]
            unsafe fn peripheral_didDiscoverServices(
                &self,
                peripheral: &CBPeripheral,
                _error: Option<&NSError>,
            ) {
                let Some(service) = (unsafe { find_service(peripheral) }) else {
                    eprintln!("[Atoll] gatt: battery service not found after discovery");
                    finish_current(GattEvent::NoData);
                    return;
                };
                unsafe { peripheral.discoverCharacteristics_forService(None, &service) };
            }

            #[unsafe(method(peripheral:didDiscoverCharacteristicsForService:error:))]
            unsafe fn peripheral_didDiscoverCharacteristicsForService_error(
                &self,
                peripheral: &CBPeripheral,
                service: &CBService,
                _error: Option<&NSError>,
            ) {
                let Some(characteristic) = (unsafe { find_battery_characteristic(service) }) else {
                    eprintln!("[Atoll] gatt: battery level characteristic not found");
                    finish_current(GattEvent::NoData);
                    return;
                };
                unsafe { peripheral.readValueForCharacteristic(&characteristic) };
            }

            #[unsafe(method(peripheral:didUpdateValueForCharacteristic:error:))]
            unsafe fn peripheral_didUpdateValueForCharacteristic_error(
                &self,
                _peripheral: &CBPeripheral,
                characteristic: &CBCharacteristic,
                error: Option<&NSError>,
            ) {
                let percent = error
                    .is_none()
                    .then(|| characteristic.value())
                    .flatten()
                    .map(|data| data.to_vec())
                    .and_then(|bytes| bytes.first().copied())
                    .map(|first| first.min(100));
                eprintln!("[Atoll] gatt: battery value read -> {percent:?}");
                match percent {
                    Some(percent) => finish_current(GattEvent::Percent(percent)),
                    None => finish_current(GattEvent::NoData),
                }
            }
        }
    );

    fn battery_uuid(value: &str) -> Retained<CBUUID> {
        unsafe { CBUUID::UUIDWithString(&NSString::from_str(value)) }
    }

    unsafe fn uuid_matches(uuid: &CBUUID, expected: &str) -> bool {
        unsafe { uuid.UUIDString() }
            .to_string()
            .eq_ignore_ascii_case(expected)
    }

    unsafe fn find_service(peripheral: &CBPeripheral) -> Option<Retained<CBService>> {
        let services = unsafe { peripheral.services() }?;
        services
            .objects_in_range(0..services.count())
            .into_iter()
            .find(|service| unsafe { uuid_matches(&service.UUID(), SERVICE_UUID) })
    }

    unsafe fn find_battery_characteristic(
        service: &CBService,
    ) -> Option<Retained<CBCharacteristic>> {
        let characteristics = unsafe { service.characteristics() }?;
        characteristics
            .objects_in_range(0..characteristics.count())
            .into_iter()
            .find(|characteristic| unsafe {
                uuid_matches(&characteristic.UUID(), CHARACTERISTIC_UUID)
            })
    }

    /// Look for `target` among the system-connected peripherals that expose
    /// the battery service and kick off a connection. Called with the session
    /// borrowed.
    unsafe fn start_probe_locked(session: &mut GattSession, target: &str) {
        let Some(central) = session.central.as_ref() else {
            finish_locked(session, GattEvent::NoData);
            return;
        };
        let connected =
            central.retrieveConnectedPeripheralsWithServices(&NSArray::from_retained_slice(&[
                battery_uuid(SERVICE_UUID),
            ]));
        let matched = connected
            .objects_in_range(0..connected.count())
            .into_iter()
            .find(|peripheral| {
                peripheral
                    .name()
                    .map(|name| name.to_string().eq_ignore_ascii_case(target))
                    .unwrap_or(false)
            });
        let Some(peripheral) = matched else {
            // Connected to the system but exposes no battery service (or the
            // name isn't advertised): nothing we can read.
            finish_locked(session, GattEvent::NoData);
            return;
        };
        unsafe { central.connectPeripheral_options(&peripheral, None) };
        session.connecting = Some(peripheral);
    }

    /// Complete the in-flight probe and detach the event channel so stray
    /// callbacks become no-ops. Called with the session borrowed.
    fn finish_locked(session: &mut GattSession, event: GattEvent) {
        if let Some(peripheral) = session.connecting.take() {
            if let Some(central) = session.central.as_ref() {
                unsafe { central.cancelPeripheralConnection(&peripheral) };
            }
        }
        if let Some(events) = session.events.take() {
            let _ = events.send(event);
        }
        session.target = None;
    }

    /// Helper for peripheral-delegate callbacks: report the device that is
    /// currently being probed.
    fn finish_current(event: GattEvent) {
        SESSION.with_borrow_mut(|session| {
            if let Some(state) = session.as_mut() {
                finish_locked(state, event);
            }
        });
    }

    fn reset_session() {
        SESSION.with_borrow_mut(|session| {
            let Some(state) = session.as_mut() else {
                return;
            };
            if let Some(peripheral) = state.connecting.take() {
                if let Some(central) = state.central.as_ref() {
                    unsafe { central.cancelPeripheralConnection(&peripheral) };
                }
            }
            state.events = None;
            state.target = None;
        });
    }

    /// Kick a probe off on the main thread. Creates the central manager on
    /// first use (this is what triggers the macOS Bluetooth permission
    /// prompt).
    unsafe fn begin_probe(target: String, events: Sender<GattEvent>) {
        SESSION.with_borrow_mut(|session| {
            // Abandon any probe that timed out earlier; cancelling the
            // leftover connection keeps CoreBluetooth state clean.
            if let Some(state) = session.as_mut() {
                state.events = None;
                state.target = None;
            }

            let central = match session.as_ref().and_then(|state| state.central.clone()) {
                Some(central) => central,
                None => {
                    eprintln!(
                        "[Atoll] requesting Bluetooth permission to read GATT battery levels"
                    );
                    let delegate: Retained<GattDelegate> = msg_send![GattDelegate::class(), new];
                    let central = CBCentralManager::initWithDelegate_queue(
                        CBCentralManager::alloc(),
                        Some(ProtocolObject::from_ref(&*delegate)),
                        None,
                    );
                    let _ = DELEGATE.set(delegate);
                    *session = Some(GattSession {
                        central: Some(central.clone()),
                        ..Default::default()
                    });
                    central
                }
            };

            let state = session.as_mut().expect("session just ensured");
            state.events = Some(events);
            state.target = Some(target);
            match central.state() {
                CBManagerState::PoweredOn => {
                    let target = state.target.clone().expect("target just set");
                    unsafe { start_probe_locked(state, &target) };
                }
                CBManagerState::Unauthorized | CBManagerState::Unsupported => {
                    finish_locked(state, GattEvent::PermissionDenied);
                }
                CBManagerState::PoweredOff => finish_locked(state, GattEvent::NoData),
                // Unknown/Resetting: centralManagerDidUpdateState drives the rest.
                _ => {}
            }
        });
    }

    /// Probe one device's battery level. Blocks the caller for at most
    /// `PROBE_TIMEOUT` while the main thread runs the CoreBluetooth flow.
    pub(crate) fn probe_device(app: &AppHandle, name: &str) -> ProbeOutcome {
        let (tx, rx) = channel();
        let target = name.to_string();
        let dispatch = app.run_on_main_thread(move || unsafe { begin_probe(target, tx) });
        if dispatch.is_err() {
            return ProbeOutcome::NoData;
        }
        match rx.recv_timeout(PROBE_TIMEOUT) {
            Ok(GattEvent::Percent(percent)) => {
                eprintln!("[Atoll] gatt: probe {name:?} -> {percent}%");
                ProbeOutcome::Percent(percent)
            }
            Ok(GattEvent::NoData) => {
                eprintln!("[Atoll] gatt: probe {name:?} -> no data");
                ProbeOutcome::NoData
            }
            Ok(GattEvent::PermissionDenied) => {
                eprintln!("[Atoll] gatt: probe {name:?} -> permission denied/unavailable");
                ProbeOutcome::PermissionDenied
            }
            Err(_) => {
                eprintln!("[Atoll] gatt: probe {name:?} timed out");
                // Timeout: release the main-thread session so the next probe
                // starts clean.
                let _ = app.run_on_main_thread(reset_session);
                ProbeOutcome::NoData
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::probe_device;
