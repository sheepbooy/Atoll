import { invoke, } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import { isWindowsTauriRuntime } from "./runtime";
import { getDemoMode, getDemoSnapshot } from "../demoSnapshot";
import { normalizeSnapshot } from "./snapshot";
import type {
  IslandHoverChanged,
  IslandSnapshot,
} from "./types";
import { isTauriRuntime } from "./runtime";
/** Matches `uses_micro_island` in src-tauri (Windows-only micro island). */
export function usesMicroIslandSync(): boolean {
  return isWindowsTauriRuntime();
}

export async function setImeActive(active: boolean) {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_ime_active", { active });
}

export async function setIslandPresentation(
  mode: "micro" | "compact" | "expanded" | "dormant",
  compactWidth?: number,
  expandedIdle?: boolean,
  compactLeftWidth?: number,
  animate = true,
  snap = false,
  expandedPlan?: boolean,
  expandedSettings?: boolean,
) {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_island_presentation", {
    mode,
    compactWidth,
    compactLeftWidth,
    expandedIdle,
    expandedPlan,
    expandedSettings,
    animate,
    snap,
  });
}

export async function usesMicroIsland(): Promise<boolean> {
  if (!isTauriRuntime()) {
    return false;
  }

  return invoke<boolean>("uses_micro_island");
}

/** Persist compact layout metrics without triggering a native window animation. */
export async function setCompactLayout(
  compactWidth: number,
  compactLeftWidth: number,
) {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_island_presentation", {
    mode: "compact",
    compactWidth,
    compactLeftWidth,
    animate: false,
  });
}

export interface NotchMetrics {
  hasNotch: boolean;
  width: number;
  height: number;
  leftAreaWidth?: number;
  rightAreaWidth?: number;
}

export async function getNotchMetrics(): Promise<NotchMetrics> {
  if (isTauriRuntime()) {
    return invoke<NotchMetrics>("get_notch_metrics");
  }

  if (getDemoMode() === "compact" || getDemoMode() === "gif") {
    return { hasNotch: true, width: 180, height: 32, leftAreaWidth: 120, rightAreaWidth: 120 };
  }

  return { hasNotch: false, width: 0, height: 0 };
}

export async function onSnapshotChanged(callback: (snapshot: IslandSnapshot) => void) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<IslandSnapshot>("snapshot-changed", (event) =>
    callback(normalizeSnapshot(event.payload)),
  );
}

export async function onIslandHoverChanged(callback: (state: IslandHoverChanged) => void) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<IslandHoverChanged>("island-hover-changed", (event) => callback(event.payload));
}

/** Why the island was opened. "summon" comes from the global hotkey and
 * toggles (press again to collapse, no idle auto-collapse); every other
 * opener keeps the expand-then-idle-collapse behavior. */
export type IslandOpenSource = "summon" | "focus";

export async function onIslandOpenRequested(
  callback: (source: IslandOpenSource) => void,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<string | null>("island-open-requested", (event) =>
    callback(event.payload === "summon" ? "summon" : "focus"),
  );
}

/** Fires when the native window animation finishes or snaps to its target. */
export async function onIslandPresentationSettled(
  callback: (mode: string) => void | Promise<void>,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<string>("island-presentation-settled", (event) =>
    callback(event.payload),
  );
}
