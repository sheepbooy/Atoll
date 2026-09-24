import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useBluetoothBattery } from "./useBluetoothBattery";
import type { BluetoothBatteryReport } from "../tauri";

const stateMocks = vi.hoisted(() => ({
  getBluetoothBattery: vi.fn(),
  getBluetoothBatteryCardEnabled: vi.fn(),
  getBluetoothBatteryAlertEnabled: vi.fn(),
  getBluetoothBatteryAlertThreshold: vi.fn(),
  setBluetoothBatteryCardEnabled: vi.fn(),
  setBluetoothBatteryAlertEnabled: vi.fn(),
  setBluetoothBatteryAlertThreshold: vi.fn(),
  listeners: new Map<string, (payload: unknown) => void>(),
}));

vi.mock("../tauri", () => ({
  getBluetoothBattery: stateMocks.getBluetoothBattery,
  getBluetoothBatteryCardEnabled: stateMocks.getBluetoothBatteryCardEnabled,
  getBluetoothBatteryAlertEnabled: stateMocks.getBluetoothBatteryAlertEnabled,
  getBluetoothBatteryAlertThreshold: stateMocks.getBluetoothBatteryAlertThreshold,
  setBluetoothBatteryCardEnabled: stateMocks.setBluetoothBatteryCardEnabled,
  setBluetoothBatteryAlertEnabled: stateMocks.setBluetoothBatteryAlertEnabled,
  setBluetoothBatteryAlertThreshold: stateMocks.setBluetoothBatteryAlertThreshold,
  onBluetoothBatteryChanged: (callback: (payload: unknown) => void) => {
    stateMocks.listeners.set("bluetooth-battery-changed", callback);
    return Promise.resolve(() => stateMocks.listeners.delete("bluetooth-battery-changed"));
  },
}));

import { onBluetoothBatteryChanged } from "../tauri";

const emptyReport: BluetoothBatteryReport = { devices: [] };

async function emitReport(report: BluetoothBatteryReport | null) {
  const listener = stateMocks.listeners.get("bluetooth-battery-changed");
  expect(listener).toBeDefined();
  await act(async () => {
    listener?.(report);
  });
}

describe("useBluetoothBattery", () => {
  it("loads settings and the initial report, then follows change events", async () => {
    stateMocks.getBluetoothBattery.mockResolvedValue(emptyReport);
    stateMocks.getBluetoothBatteryCardEnabled.mockResolvedValue(true);
    stateMocks.getBluetoothBatteryAlertEnabled.mockResolvedValue(false);
    stateMocks.getBluetoothBatteryAlertThreshold.mockResolvedValue(30);
    vi.mocked(onBluetoothBatteryChanged);

    const { result } = renderHook(() => useBluetoothBattery());

    await waitFor(() => {
      expect(result.current.bluetoothDevices).toEqual([]);
      expect(result.current.cardEnabled).toBe(true);
      expect(result.current.alertEnabled).toBe(false);
      expect(result.current.alertThreshold).toBe(30);
    });

    await emitReport({
      devices: [
        {
          id: "aa:bb:cc:dd:ee:01",
          name: "Magic Mouse",
          kind: "mouse",
          batteryPercent: 73,
          casePercent: null,
          leftPercent: null,
          rightPercent: null,
        },
      ],
    });
    expect(result.current.bluetoothDevices).toHaveLength(1);
    expect(result.current.bluetoothDevices[0].name).toBe("Magic Mouse");

    // Backend clears the report when the card is disabled.
    await emitReport(null);
    expect(result.current.bluetoothDevices).toEqual([]);
  });

  it("persists setting changes through the tauri wrappers", async () => {
    stateMocks.getBluetoothBattery.mockResolvedValue(emptyReport);
    stateMocks.getBluetoothBatteryCardEnabled.mockResolvedValue(true);
    stateMocks.getBluetoothBatteryAlertEnabled.mockResolvedValue(true);
    stateMocks.getBluetoothBatteryAlertThreshold.mockResolvedValue(20);
    stateMocks.setBluetoothBatteryCardEnabled.mockResolvedValue(true);
    stateMocks.setBluetoothBatteryAlertEnabled.mockResolvedValue(true);
    stateMocks.setBluetoothBatteryAlertThreshold.mockResolvedValue(25);

    const { result } = renderHook(() => useBluetoothBattery());
    await waitFor(() => {
      expect(result.current.alertThreshold).toBe(20);
    });

    act(() => result.current.handleChangeCardEnabled(false));
    act(() => result.current.handleChangeAlertEnabled(false));
    act(() => result.current.handleChangeAlertThreshold(25));

    expect(result.current.cardEnabled).toBe(false);
    expect(result.current.alertEnabled).toBe(false);
    expect(result.current.alertThreshold).toBe(25);
    expect(stateMocks.setBluetoothBatteryCardEnabled).toHaveBeenCalledWith(false);
    expect(stateMocks.setBluetoothBatteryAlertEnabled).toHaveBeenCalledWith(false);
    expect(stateMocks.setBluetoothBatteryAlertThreshold).toHaveBeenCalledWith(25);
  });
});
