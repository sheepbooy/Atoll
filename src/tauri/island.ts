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

export type IslandWindowMode = "micro" | "compact" | "expanded" | "dormant";

export interface IslandLayoutMetrics {
  compactWidth?: number;
  compactLeftWidth?: number;
  expandedWingLeft?: number;
  expandedWingRight?: number;
}

export interface IslandPresentationOptions extends IslandLayoutMetrics {
  mode: IslandWindowMode;
  expandedIdle?: boolean;
  expandedPlan?: boolean;
  expandedSettings?: boolean;
  animate?: boolean;
  snap?: boolean;
  durationMs?: number;
  transitionId?: number;
}

export interface IslandPresentationSettled {
  mode: IslandWindowMode;
  transitionId: number;
}

let transitionSequence = 0;
export function nextIslandTransitionId(): number {
  return ++transitionSequence;
}

export async function setIslandPresentation(options: IslandPresentationOptions) {
  if (!isTauriRuntime()) return;
  return invoke<void>("set_island_presentation", {
    animate: true,
    snap: false,
    ...options,
    transitionId: options.transitionId ?? nextIslandTransitionId(),
  });
}

/** Metrics persistence never changes presentation, including reduced motion. */
export async function updateIslandLayoutMetrics(metrics: IslandLayoutMetrics) {
  if (!isTauriRuntime()) return;
  return invoke<void>("update_island_layout_metrics", { ...metrics });
}

/** Cosmetic window pulse for the file-station choreography: grow/shrink the
 *  island by `widthDelta` x `heightDelta` logical points (top edge pinned),
 *  spring out over `outMs` and ease back over `backMs`. Fire-and-forget; the
 *  native side aborts any running pulse when a presentation change starts. */
export async function pulseIslandShape(options: {
  widthDelta?: number;
  heightDelta?: number;
  outMs?: number;
  backMs?: number;
}) {
  if (!isTauriRuntime()) {
    return;
  }
  return invoke<void>("pulse_island_shape", {
    widthDelta: options.widthDelta ?? 0,
    heightDelta: options.heightDelta ?? 0,
    outMs: options.outMs,
    backMs: options.backMs,
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

  return updateIslandLayoutMetrics({ compactWidth, compactLeftWidth });
}

export interface NotchMetrics {
  hasNotch: boolean;
  width: number;
  height: number;
  leftAreaWidth?: number;
  rightAreaWidth?: number;
  /** Notch bottom corner radius (logical px) for the collapsed capsule. */
  cornerRadius?: number;
}

export async function getNotchMetrics(): Promise<NotchMetrics> {
  if (isTauriRuntime()) {
    return invoke<NotchMetrics>("get_notch_metrics");
  }

  if (getDemoMode() === "compact" || getDemoMode() === "gif") {
    return {
      hasNotch: true,
      width: 220,
      height: 38,
      leftAreaWidth: 120,
      rightAreaWidth: 120,
      cornerRadius: 10,
    };
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

/** Fires when macOS rearranges displays (plug/unplug, resolution change) and
 * the native notch metrics changed. Payload mirrors `get_notch_metrics`. */
export async function onNotchMetricsChanged(
  callback: (notch: NotchMetrics) => void | Promise<void>,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<NotchMetrics>("notch-metrics-changed", (event) =>
    callback(event.payload),
  );
}

/** Fires when the native window animation finishes or snaps to its target. */
export async function onIslandPresentationSettled(
  callback: (event: IslandPresentationSettled) => void | Promise<void>,
) {
  if (!isTauriRuntime()) {
    return () => undefined;
  }

  return listen<IslandPresentationSettled>("island-presentation-settled", (event) =>
    callback(event.payload),
  );
}
