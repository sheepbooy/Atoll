import { useTranslation } from "react-i18next";
import {
  Battery,
  BatteryLow,
  BatteryWarning,
  Bluetooth,
  Headphones,
  Keyboard,
  Mouse,
  Pointer,
} from "lucide-react";
import type { BluetoothDeviceBattery, BluetoothDeviceKind } from "./tauri";

interface BluetoothBatteryCardProps {
  devices: BluetoothDeviceBattery[];
  /** Percent at or below which a device is flagged red (matches the alert). */
  alertThreshold: number;
}

function KindIcon({ kind }: { kind: BluetoothDeviceKind }) {
  switch (kind) {
    case "mouse":
      return <Mouse size={14} />;
    case "keyboard":
      return <Keyboard size={14} />;
    case "trackpad":
      return <Pointer size={14} />;
    case "headphones":
      return <Headphones size={14} />;
    default:
      return <Bluetooth size={14} />;
  }
}

function BatteryChip({
  percent,
  alertThreshold,
}: {
  percent: number;
  alertThreshold: number;
}) {
  const low = percent <= alertThreshold;
  const mid = !low && percent < 40;
  const Icon = low ? BatteryWarning : mid ? BatteryLow : Battery;
  return (
    <span
      className={`bt-batt-chip${low ? " is-low" : mid ? " is-mid" : ""}`}
      title={`${percent}%`}
    >
      <Icon size={13} />
      {percent}%
    </span>
  );
}

/** Per-bud / case detail line for AirPods-style devices. */
function BudsDetail({
  device,
}: {
  device: Pick<
    BluetoothDeviceBattery,
    "leftPercent" | "rightPercent" | "casePercent"
  >;
}) {
  const { t } = useTranslation("common");
  const parts: string[] = [];
  if (device.leftPercent != null) {
    parts.push(`L ${device.leftPercent}%`);
  }
  if (device.rightPercent != null) {
    parts.push(`R ${device.rightPercent}%`);
  }
  if (device.casePercent != null) {
    parts.push(`${t("bluetooth.case")} ${device.casePercent}%`);
  }
  if (parts.length === 0) {
    return null;
  }
  return <span className="bt-device-detail">{parts.join(" · ")}</span>;
}

/**
 * Footer card listing connected Bluetooth devices with their battery levels.
 * Rendered only when the backend reports at least one battery-carrying
 * device; levels turn amber below 40% and red at the alert threshold.
 */
export function BluetoothBatteryCard({
  devices,
  alertThreshold,
}: BluetoothBatteryCardProps) {
  const { t } = useTranslation("common");

  return (
    <div
      className="bluetooth-battery-card"
      data-no-drag
      aria-label={t("bluetooth.title")}
    >
      {devices.map((device) => {
        const detailPercent =
          device.batteryPercent ??
          [device.leftPercent, device.rightPercent, device.casePercent]
            .filter((value): value is number => value != null)
            .reduce((a, b) => Math.min(a, b), 100);
        return (
          <div className="bt-device-row" key={device.id}>
            <span className="bt-device-icon">
              <KindIcon kind={device.kind} />
            </span>
            <span className="bt-device-name" title={device.name}>
              {device.name}
            </span>
            {device.batteryPercent == null ? (
              <BudsDetail device={device} />
            ) : null}
            {Number.isFinite(detailPercent) ? (
              <BatteryChip percent={detailPercent} alertThreshold={alertThreshold} />
            ) : null}
          </div>
        );
      })}
    </div>
  );
}
