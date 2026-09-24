import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { BluetoothDeviceBattery, BluetoothDeviceKind } from "./tauri";
import { BATTERY_RING_MAX } from "./compactLayout";

interface BluetoothBatteryRingProps {
  devices: BluetoothDeviceBattery[];
  /** Percent at or below which the ring turns red (matches the alert). */
  alertThreshold: number;
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

// Emoji rather than an icon font/lucide: text always paints in the folded
// island webview where inline SVG does not composite reliably.
const KIND_GLYPH: Record<BluetoothDeviceKind, string> = {
  mouse: "🖱",
  keyboard: "⌨",
  trackpad: "🖥",
  headphones: "🎧",
  other: "🔵",
};

const RING_PX = 20;

/**
 * Canvas-draw the ring (track + battery arc + kind glyph) and return a PNG
 * data URL, rendered as a plain <img> — the exact mechanism the compact
 * media-artwork thumb uses, which demonstrably paints correctly in the
 * folded island panel (the panel's fixed/absolute coordinate space misplaces
 * CSS-positioned elements, but flows regular replaced elements fine).
 */
function ringImageDataUrl(
  percent: number,
  glyph: string,
  low: boolean,
  mid: boolean,
): string | null {
  if (typeof document === "undefined") {
    return null;
  }
  const scale = 2;
  const size = RING_PX * scale;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return null;
  }
  const cx = size / 2;
  const cy = size / 2;
  const radius = size / 2 - 3;
  // Track
  ctx.beginPath();
  ctx.arc(cx, cy, radius, 0, Math.PI * 2);
  ctx.strokeStyle = "rgba(255,255,255,0.18)";
  ctx.lineWidth = 3.2;
  ctx.stroke();
  // Battery arc, starting at 12 o'clock, clockwise.
  const arcColor = low
    ? "rgba(255,105,105,1)"
    : mid
      ? "rgba(255,190,80,1)"
      : "rgba(76,217,100,1)";
  ctx.beginPath();
  ctx.arc(cx, cy, radius, -Math.PI / 2, -Math.PI / 2 + (percent / 100) * Math.PI * 2);
  ctx.strokeStyle = arcColor;
  ctx.lineCap = "round";
  ctx.lineWidth = 3.2;
  ctx.stroke();
  // Device-kind glyph in the middle.
  ctx.font = `${10 * scale}px system-ui, "Apple Color Emoji", sans-serif`;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillStyle = low ? "rgba(255,150,150,0.95)" : "rgba(255,255,255,0.92)";
  ctx.fillText(glyph, cx, cy + 1 * scale);
  return canvas.toDataURL("image/png");
}

/** Devices that report a level, lowest battery first (the worst ring leads). */
export function devicesWithBattery(
  devices: BluetoothDeviceBattery[],
): Array<{ device: BluetoothDeviceBattery; percent: number }> {
  return devices
    .map((device) => ({ device, percent: deviceBatteryPercent(device) }))
    .filter((entry): entry is { device: BluetoothDeviceBattery; percent: number } =>
      entry.percent != null,
    )
    .sort((a, b) => a.percent - b.percent);
}

/**
 * Folded-island indicator: one battery ring per reporting device (green,
 * amber below 40%, red at the alert threshold) around the device-kind
 * glyph, drawn on a canvas and shown as an img next to the token counter —
 * the panel composites regular replaced elements reliably, unlike inline
 * SVG or conic gradients. Rings render lowest-battery-first; each carries
 * its own tooltip. Renders nothing when no device reports a battery.
 */
export function BluetoothBatteryRing({
  devices,
  alertThreshold,
}: BluetoothBatteryRingProps) {
  const { t } = useTranslation("common");
  const entries = devicesWithBattery(devices).slice(0, BATTERY_RING_MAX);
  const srcs = useMemo(
    () =>
      entries.map((entry) =>
        ringImageDataUrl(
          entry.percent,
          KIND_GLYPH[entry.device.kind],
          entry.percent <= alertThreshold,
          entry.percent > alertThreshold && entry.percent < 40,
        ),
      ),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      entries
        .map((entry) => `${entry.device.kind}:${entry.percent}`)
        .join("|"),
      alertThreshold,
    ],
  );
  if (entries.length === 0 || srcs.some((src) => !src)) {
    return null;
  }
  return (
    <span className="compact-battery-rings" data-no-drag>
      {entries.map((entry, index) => (
        <img
          key={entry.device.id}
          className="compact-battery-ring"
          src={srcs[index] ?? undefined}
          width={RING_PX}
          height={RING_PX}
          title={`${entry.device.name} ${entry.percent}%`}
          alt={`${t("bluetooth.title")}: ${entry.device.name} ${entry.percent}%`}
          draggable={false}
        />
      ))}
    </span>
  );
}
