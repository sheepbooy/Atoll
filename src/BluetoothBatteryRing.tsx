import { useTranslation } from "react-i18next";
import { Bluetooth, Headphones, Keyboard, Mouse, Pointer } from "lucide-react";
import type { BluetoothDeviceBattery, BluetoothDeviceKind } from "./tauri";

interface BluetoothBatteryRingProps {
  devices: BluetoothDeviceBattery[];
  /** Percent at or below which the ring turns red (matches the alert). */
  alertThreshold: number;
}

const RING_ICON_PROPS = {
  className: "bt-ring-icon",
  size: 9,
  strokeWidth: 2.75,
} as const;

function KindIcon({ kind }: { kind: BluetoothDeviceKind }) {
  switch (kind) {
    case "mouse":
      return <Mouse {...RING_ICON_PROPS} />;
    case "keyboard":
      return <Keyboard {...RING_ICON_PROPS} />;
    case "trackpad":
      return <Pointer {...RING_ICON_PROPS} />;
    case "headphones":
      return <Headphones {...RING_ICON_PROPS} />;
    default:
      return <Bluetooth {...RING_ICON_PROPS} />;
  }
}

/** Whether any device reports a battery level (drives ring visibility). */
export function hasBatteryData(devices: BluetoothDeviceBattery[]): boolean {
  return devices.some((device) => deviceBatteryPercent(device) != null);
}

/** Lowest reported level across main/buds/case; null when none reported. */
export function deviceBatteryPercent(device: BluetoothDeviceBattery): number | null {
  const values = [
    device.batteryPercent,
    device.leftPercent,
    device.rightPercent,
    device.casePercent,
  ].filter((value): value is number => value != null);
  if (values.length === 0) {
    return null;
  }
  return Math.min(...values);
}

/**
 * Folded-island indicator: a battery ring (green, amber below 40%, red at
 * the alert threshold) around a device-kind icon. With several
 * devices it shows the lowest battery — the one that needs attention — and
 * the tooltip lists every device. Renders nothing when no device reports a
 * battery.
 */
export function BluetoothBatteryRing({
  devices,
  alertThreshold,
}: BluetoothBatteryRingProps) {
  const { t } = useTranslation("common");
  const withBattery = devices
    .map((device) => ({ device, percent: deviceBatteryPercent(device) }))
    .filter((entry): entry is { device: BluetoothDeviceBattery; percent: number } =>
      entry.percent != null,
    );
  if (withBattery.length === 0) {
    return null;
  }
  const worst = withBattery.reduce((a, b) => (b.percent < a.percent ? b : a));
  const low = worst.percent <= alertThreshold;
  const mid = !low && worst.percent < 40;
  const radius = 8;
  const circumference = 2 * Math.PI * radius;
  const dash = (worst.percent / 100) * circumference;
  const title = withBattery
    .map(({ device, percent }) => `${device.name} ${percent}%`)
    .join(" · ");

  return (
    <span
      className={`compact-battery-ring${low ? " is-low" : mid ? " is-mid" : ""}`}
      title={title}
      role="img"
      aria-label={`${t("bluetooth.title")}: ${title}`}
      data-no-drag
    >
      <svg viewBox="0 0 20 20" width={18} height={18} aria-hidden="true">
        <circle className="bt-ring-track" cx="10" cy="10" r={radius} />
        <circle
          className="bt-ring-fill"
          cx="10"
          cy="10"
          r={radius}
          strokeDasharray={`${dash} ${circumference}`}
          transform="rotate(-90 10 10)"
        />
      </svg>
      <KindIcon kind={worst.device.kind} />
    </span>
  );
}
