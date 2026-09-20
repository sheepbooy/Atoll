import { useCallback, useEffect, useState } from "react";
import {
  BluetoothBatteryReport,
  getBluetoothBattery,
  getBluetoothBatteryAlertEnabled,
  getBluetoothBatteryAlertThreshold,
  getBluetoothBatteryCardEnabled,
  onBluetoothBatteryChanged,
  setBluetoothBatteryAlertEnabled,
  setBluetoothBatteryAlertThreshold,
  setBluetoothBatteryCardEnabled,
} from "../tauri";
import { manageAsyncUnlisten } from "../asyncUnlisten";

/**
 * Connected Bluetooth battery levels plus the card/alert settings. Mirrors
 * `useNowPlaying`: an initial `get_bluetooth_battery` pull, then the backend
 * monitor pushes `bluetooth-battery-changed` only when the report changes.
 */
export function useBluetoothBattery() {
  const [report, setReport] = useState<BluetoothBatteryReport | null>(null);
  const [cardEnabled, setCardEnabledState] = useState(true);
  const [alertEnabled, setAlertEnabledState] = useState(true);
  const [alertThreshold, setAlertThresholdState] = useState(20);

  useEffect(() => {
    getBluetoothBatteryCardEnabled()
      .then(setCardEnabledState)
      .catch(() => undefined);
    getBluetoothBatteryAlertEnabled()
      .then(setAlertEnabledState)
      .catch(() => undefined);
    getBluetoothBatteryAlertThreshold()
      .then(setAlertThresholdState)
      .catch(() => undefined);
    getBluetoothBattery()
      .then(setReport)
      .catch(() => undefined);
    const unsubscribe = manageAsyncUnlisten(
      onBluetoothBatteryChanged((next) => {
        setReport(next);
      }),
    );
    return () => {
      unsubscribe();
    };
  }, []);

  const handleChangeCardEnabled = useCallback((enabled: boolean) => {
    setCardEnabledState(enabled);
    setBluetoothBatteryCardEnabled(enabled).catch(() => undefined);
  }, []);

  const handleChangeAlertEnabled = useCallback((enabled: boolean) => {
    setAlertEnabledState(enabled);
    setBluetoothBatteryAlertEnabled(enabled).catch(() => undefined);
  }, []);

  const handleChangeAlertThreshold = useCallback((threshold: number) => {
    setAlertThresholdState(threshold);
    setBluetoothBatteryAlertThreshold(threshold).catch(() => undefined);
  }, []);

  return {
    bluetoothDevices: report?.devices ?? [],
    cardEnabled,
    alertEnabled,
    alertThreshold,
    handleChangeCardEnabled,
    handleChangeAlertEnabled,
    handleChangeAlertThreshold,
  };
}
