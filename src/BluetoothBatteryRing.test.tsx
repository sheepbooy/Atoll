import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { BluetoothBatteryRing, deviceBatteryPercent } from "./BluetoothBatteryRing";
import type { BluetoothDeviceBattery } from "./tauri";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => (key === "bluetooth.title" ? "Bluetooth battery" : key),
  }),
}));

function device(overrides: Partial<BluetoothDeviceBattery>): BluetoothDeviceBattery {
  return {
    id: "aa:bb:cc:dd:ee:01",
    name: "Magic Mouse",
    kind: "mouse",
    batteryPercent: 73,
    casePercent: null,
    leftPercent: null,
    rightPercent: null,
    ...overrides,
  };
}

describe("deviceBatteryPercent", () => {
  it("uses the lowest reported level across main/buds/case", () => {
    expect(
      deviceBatteryPercent(
        device({ batteryPercent: null, leftPercent: 90, rightPercent: 35, casePercent: 80 }),
      ),
    ).toBe(35);
    expect(deviceBatteryPercent(device({ batteryPercent: null }))).toBeNull();
  });
});

describe("BluetoothBatteryRing", () => {
  // jsdom has no canvas, so ringImageDataUrl returns null and the component
  // renders nothing; the drawing itself is exercised on real hardware.
  it("renders nothing when no device reports a battery", () => {
    const { container } = render(
      <BluetoothBatteryRing devices={[device({ batteryPercent: null })]} alertThreshold={20} />,
    );
    expect(container.querySelector(".compact-battery-ring")).toBeNull();
  });

  it("renders nothing in environments without canvas (graceful)", () => {
    const { container } = render(
      <BluetoothBatteryRing devices={[device({})]} alertThreshold={20} />,
    );
    expect(container.querySelector(".compact-battery-ring")).toBeNull();
  });
});
