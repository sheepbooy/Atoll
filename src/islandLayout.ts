import {
  type NotchMetrics,
} from "./tauri";
import {
  type PresentationPhase,
} from "./islandPresentation";
import {
  COMPACT_HEADER_GAP,
  COMPACT_METRICS_GAP,
  COMPACT_NOTCH_INNER_GAP,
  computeMicroWindowWidth,
} from "./compactLayout";
import {
  type FoldedIslandSize,
} from "./appTypes";

// Keep in sync with COMPACT_WINDOW_HEIGHT in src-tauri/src/lib.rs.
export const COMPACT_WINDOW_HEIGHT = 36;
// Keep in sync with MICRO_WINDOW_HEIGHT in src-tauri/src/lib.rs.
export const MICRO_WINDOW_HEIGHT = 24;
// Keep in sync with NOTCH_COVER_PADDING in src-tauri/src/lib.rs.
export const NOTCH_COVER_PADDING = 16;

// Keep in sync with EXPANDED_IDLE_WINDOW_HEIGHT in src-tauri/src/lib.rs.
export const EXPANDED_IDLE_WINDOW_HEIGHT = 240;
// Keep in sync with EXPANDED_PLAN_WINDOW_WIDTH in src-tauri/src/lib.rs.
export const EXPANDED_PLAN_WINDOW_WIDTH = 680;
// Keep in sync with EXPANDED_PLAN_WINDOW_HEIGHT in src-tauri/src/lib.rs.
export const EXPANDED_PLAN_WINDOW_HEIGHT = 680;
// Keep in sync with EXPANDED_SETTINGS_WINDOW_WIDTH in src-tauri/src/lib.rs.
export const EXPANDED_SETTINGS_WINDOW_WIDTH = 680;
// Keep in sync with EXPANDED_SETTINGS_WINDOW_HEIGHT in src-tauri/src/lib.rs.
export const EXPANDED_SETTINGS_WINDOW_HEIGHT = 680;

export function applyWindowMetrics(notch: NotchMetrics) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--compact-height", `${COMPACT_WINDOW_HEIGHT}px`);
  root.style.setProperty("--micro-height", `${MICRO_WINDOW_HEIGHT}px`);
  root.style.setProperty(
    "--expanded-idle-height",
    `${EXPANDED_IDLE_WINDOW_HEIGHT}px`,
  );
  root.style.setProperty(
    "--expanded-plan-width",
    `${EXPANDED_PLAN_WINDOW_WIDTH}px`,
  );
  root.style.setProperty(
    "--expanded-plan-height",
    `${EXPANDED_PLAN_WINDOW_HEIGHT}px`,
  );
  root.style.setProperty(
    "--expanded-settings-width",
    `${EXPANDED_SETTINGS_WINDOW_WIDTH}px`,
  );
  root.style.setProperty(
    "--expanded-settings-height",
    `${EXPANDED_SETTINGS_WINDOW_HEIGHT}px`,
  );
  const coverHeight = notch.hasNotch
    ? Math.max(0, notch.height + NOTCH_COVER_PADDING)
    : 0;
  root.style.setProperty("--notch-height", `${coverHeight}px`);
  root.style.setProperty("--notch-width", `${Math.max(0, notch.width)}px`);
  if (notch.leftAreaWidth) {
    root.style.setProperty(
      "--notch-left-area-width",
      `${Math.max(0, notch.leftAreaWidth)}px`,
    );
  } else {
    root.style.removeProperty("--notch-left-area-width");
  }
  root.style.setProperty("--compact-notch-inner-gap", `${COMPACT_NOTCH_INNER_GAP}px`);
  root.style.setProperty(
    "--compact-header-gap",
    `${notch.hasNotch ? 0 : COMPACT_HEADER_GAP}px`,
  );
  root.style.setProperty("--compact-metrics-gap", `${COMPACT_METRICS_GAP}px`);
  root.classList.toggle("has-notch", notch.hasNotch);
}

export function compactPresentationKey(
  mode: "micro" | "compact" | "dormant",
  width: number,
  leftWidth: number,
): string {
  if (mode === "micro") return `micro:${width}`;
  return mode === "dormant" ? "dormant" : `compact:${width}:${leftWidth}`;
}

export function microPresentationWidth(
  sessionCount: number,
  tokenTotal: number,
  tokenCompactLevel: number,
): number {
  return computeMicroWindowWidth(sessionCount, tokenTotal, tokenCompactLevel);
}

export function shouldRestInMicro(usesMicro: boolean): boolean {
  return usesMicro;
}

export function shouldUseMicroIsland(
  supportsMicroIsland: boolean,
  foldedIslandSize: FoldedIslandSize,
): boolean {
  return supportsMicroIsland && foldedIslandSize === "small";
}

export function resolveCollapsedMode(
  usesMicro: boolean,
  supportsMicroIsland: boolean,
  sessionCount: number,
  pendingCount: number,
  phase: PresentationPhase,
  hasLyrics: boolean,
): "micro" | "compact" | "dormant" {
  if (phase === "micro") return "micro";
  if (shouldRestInMicro(usesMicro)) return "compact";
  if (supportsMicroIsland) return "compact";
  if (sessionCount === 0 && pendingCount === 0) {
    // Stay compact when lyrics are showing so the header has room.
    return hasLyrics ? "compact" : "dormant";
  }
  return "compact";
}

export function expandedPresentationKey(
  idle: boolean,
  plan: boolean,
  settings: boolean,
): string {
  if (plan) return "expanded:plan";
  if (settings) return "expanded:settings";
  return `expanded:${idle}`;
}
