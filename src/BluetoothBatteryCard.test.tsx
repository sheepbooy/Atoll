import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { BluetoothBatteryCard } from "./BluetoothBatteryCard";
import type { BluetoothDeviceBattery } from "./tauri";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) => (key === "bluetooth.case" ? "Case" : key),
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

describe("BluetoothBatteryCard", () => {
  it("renders device names and battery levels", () => {
    render(
      <BluetoothBatteryCard
        devices={[
          device({}),
          device({
            id: "bb:bb:cc:dd:ee:02",
            name: "Magic Keyboard",
            kind: "keyboard",
            batteryPercent: 91,
          }),
        ]}
        alertThreshold={20}
      />,
    );
    expect(screen.getByText("Magic Mouse")).toBeTruthy();
    expect(screen.getByText("Magic Keyboard")).toBeTruthy();
    expect(screen.getByText("73%")).toBeTruthy();
    expect(screen.getByText("91%")).toBeTruthy();
  });

  it("flags devices at or below the alert threshold as low", () => {
    render(
      <BluetoothBatteryCard
        devices={[device({ batteryPercent: 15 })]}
        alertThreshold={20}
      />,
    );
    const chip = screen.getByText("15%").closest(".bt-batt-chip");
    expect(chip?.className).toContain("is-low");
  });

  it("flags mid-level devices amber without alerting", () => {
    render(
      <BluetoothBatteryCard
        devices={[device({ batteryPercent: 35 })]}
        alertThreshold={20}
      />,
    );
    const chip = screen.getByText("35%").closest(".bt-batt-chip");
    expect(chip?.className).toContain("is-mid");
    expect(chip?.className).not.toContain("is-low");
  });

  it("shows per-bud and case detail for AirPods-style devices", () => {
    render(
      <BluetoothBatteryCard
        devices={[
          device({
            id: "cc:bb:cc:dd:ee:03",
            name: "AirPods Pro",
            kind: "headphones",
            batteryPercent: null,
            leftPercent: 90,
            rightPercent: 35,
            casePercent: 80,
          }),
        ]}
        alertThreshold={20}
      />,
    );
    expect(screen.getByText("L 90% · R 35% · Case 80%")).toBeTruthy();
    // The chip shows the lowest reported level, toned against the threshold:
    // 35% is above 20%, so it's mid, not low.
    expect(screen.getByText("35%")).toBeTruthy();
    const chip = screen.getByText("35%").closest(".bt-batt-chip");
    expect(chip?.className).toContain("is-mid");
    expect(chip?.className).not.toContain("is-low");
  });
});
