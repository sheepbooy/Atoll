import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  BluetoothBatteryRing,
  deviceBatteryPercent,
} from "./BluetoothBatteryRing";
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
  it("renders nothing when no device reports a battery", () => {
    const { container } = render(
      <BluetoothBatteryRing
        devices={[device({ batteryPercent: null })]}
        alertThreshold={20}
      />,
    );
    expect(container.querySelector(".compact-battery-ring")).toBeNull();
  });

  it("shows one ring toned by the lowest battery across devices", () => {
    render(
      <BluetoothBatteryRing
        devices={[
          device({ name: "Magic Mouse", batteryPercent: 73 }),
          device({
            id: "bb",
            name: "AirPods Pro",
            kind: "headphones",
            batteryPercent: null,
            leftPercent: 90,
            rightPercent: 15,
          }),
        ]}
        alertThreshold={20}
      />,
    );
    const ring = document.querySelector(".compact-battery-ring");
    expect(ring).not.toBeNull();
    expect(ring?.className).toContain("is-low");
    expect(screen.getByTitle("Magic Mouse 73% · AirPods Pro 15%")).toBeTruthy();
    const fill = document.querySelector(".bt-ring-fill") as SVGCircleElement;
    const dash = parseFloat(fill.getAttribute("stroke-dasharray") ?? "");
    expect(dash).toBeCloseTo((15 / 100) * 2 * Math.PI * 8);
  });

  it("stays green while levels are healthy", () => {
    render(
      <BluetoothBatteryRing devices={[device({})]} alertThreshold={20} />,
    );
    const ring = document.querySelector(".compact-battery-ring");
    expect(ring?.className).not.toContain("is-low");
    expect(ring?.className).not.toContain("is-mid");
  });

  it("turns amber below 40% without alerting", () => {
    render(
      <BluetoothBatteryRing
        devices={[device({ batteryPercent: 35 })]}
        alertThreshold={20}
      />,
    );
    const ring = document.querySelector(".compact-battery-ring");
    expect(ring?.className).toContain("is-mid");
    expect(ring?.className).not.toContain("is-low");
  });
});
