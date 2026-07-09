export type UsageDisplayMode = "tokens" | "cost";

export const FOLDED_COUNTER_DISPLAY_KEY = "atoll.display.foldedCounter";
export const EXPANDED_COUNTER_DISPLAY_KEY = "atoll.display.expandedCounter";
export const SETTINGS_BADGE_DISPLAY_KEY = "atoll.display.settingsBadge";
export const HEATMAP_DISPLAY_KEY = "atoll.display.heatmap";

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
