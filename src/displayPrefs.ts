export type UsageDisplayMode = "tokens" | "cost";
export type CompactIndicatorMode = "media" | "tokens" | "both" | "none";

export const FOLDED_COUNTER_DISPLAY_KEY = "atoll.display.foldedCounter";
export const EXPANDED_COUNTER_DISPLAY_KEY = "atoll.display.expandedCounter";
export const SETTINGS_BADGE_DISPLAY_KEY = "atoll.display.settingsBadge";
export const HEATMAP_DISPLAY_KEY = "atoll.display.heatmap";
export const COMPACT_INDICATOR_KEY = "atoll.display.compactIndicator";

export function readDisplayMode(key: string, fallback: UsageDisplayMode = "tokens"): UsageDisplayMode {
  if (typeof window === "undefined") return fallback;
  try {
    const stored = window.localStorage.getItem(key);
    return stored === "cost" ? "cost" : "tokens";
  } catch {
    return fallback;
  }
}

export function writeDisplayMode(key: string, mode: UsageDisplayMode) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(key, mode);
  } catch {
    // ignore local storage errors
  }
}

export function readCompactIndicator(
  fallback: CompactIndicatorMode = "both",
): CompactIndicatorMode {
  if (typeof window === "undefined") return fallback;
  try {
    const stored = window.localStorage.getItem(COMPACT_INDICATOR_KEY);
    if (
      stored === "media" ||
      stored === "tokens" ||
      stored === "both" ||
      stored === "none"
    ) {
      return stored;
    }
  } catch {
    // ignore
  }
  return fallback;
}

export function writeCompactIndicator(mode: CompactIndicatorMode) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(COMPACT_INDICATOR_KEY, mode);
  } catch {
    // ignore local storage errors
  }
}
