import {
  MAX_IDLE_DURATION_MIN,
  MAX_IDLE_INTERVAL_MIN,
  MAX_MAX_SUBAGENT_DISPLAY,
  MAX_RETENTION_MINUTES,
  MIN_IDLE_DURATION_MIN,
  MIN_IDLE_INTERVAL_MIN,
  MIN_MAX_SUBAGENT_DISPLAY,
  MIN_RETENTION_MINUTES,
} from "./SettingsPages";
import {
  ABSOLUTE_MAX_COMPACT_ICONS,
  MIN_MAX_COMPACT_ICONS,
} from "./compactLayout";
import {
  type FoldedIslandSize,
} from "./appTypes";

export const COMPACT_ICON_SETTING_KEY = "atoll.maxCompactIcons";
export const FOLDED_ISLAND_SIZE_SETTING_KEY = "atoll.foldedIslandSize";
export const RETENTION_SETTING_KEY = "atoll.sessionRetentionMinutes";
export const SUBAGENT_RETENTION_SETTING_KEY = "atoll.subagentRetentionMinutes";
export const MAX_SUBAGENT_DISPLAY_SETTING_KEY = "atoll.maxSubagentDisplay";
export const DEFAULT_MAX_COMPACT_ICONS = 3;
export const DEFAULT_MAX_SUBAGENT_DISPLAY = 3;
export const DEFAULT_RETENTION_MINUTES = 15;
export const DEFAULT_SUBAGENT_RETENTION_MINUTES = 10;
export const IDLE_INTERVAL_SETTING_KEY = "atoll.idleIntervalMin";
export const IDLE_DURATION_SETTING_KEY = "atoll.idleDurationMin";
export const DEFAULT_IDLE_INTERVAL_MIN = 10;
export const DEFAULT_IDLE_DURATION_MIN = 20;
export const SETTINGS_INITIALIZED_KEY = "atoll.settingsInitialized";

export function clampCompactIconLimit(
  value: number,
  max = ABSOLUTE_MAX_COMPACT_ICONS,
) {
  return Math.min(max, Math.max(MIN_MAX_COMPACT_ICONS, Math.round(value)));
}

export function readStoredSetting(
  key: string,
  defaultValue: number,
  clamp: (value: number) => number,
) {
  if (typeof window === "undefined") return defaultValue;
  try {
    const stored = window.localStorage.getItem(key);
    if (stored === null || stored.trim() === "") return defaultValue;
    const raw = Number(stored);
    if (!Number.isFinite(raw)) return defaultValue;
    return clamp(raw);
  } catch {
    return defaultValue;
  }
}

export function markSettingsInitialized() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(SETTINGS_INITIALIZED_KEY, "1");
  } catch {
    // ignore local storage errors
  }
}

export function migrateLegacySettings() {
  if (typeof window === "undefined") return;
  try {
    if (window.localStorage.getItem(SETTINGS_INITIALIZED_KEY) === "1") return;

    const stored = {
      icons: window.localStorage.getItem(COMPACT_ICON_SETTING_KEY),
      retention: window.localStorage.getItem(RETENTION_SETTING_KEY),
      interval: window.localStorage.getItem(IDLE_INTERVAL_SETTING_KEY),
      duration: window.localStorage.getItem(IDLE_DURATION_SETTING_KEY),
    };
    const hasAnyStored = Object.values(stored).some(
      (value) => value !== null && value.trim() !== "",
    );

    // First launch with the old bug wrote every slider to its minimum (1).
    if (
      hasAnyStored &&
      stored.icons === "1" &&
      stored.retention === "1" &&
      stored.interval === "1" &&
      stored.duration === "1"
    ) {
      window.localStorage.setItem(
        COMPACT_ICON_SETTING_KEY,
        String(DEFAULT_MAX_COMPACT_ICONS),
      );
      window.localStorage.setItem(
        RETENTION_SETTING_KEY,
        String(DEFAULT_RETENTION_MINUTES),
      );
      window.localStorage.setItem(
        IDLE_INTERVAL_SETTING_KEY,
        String(DEFAULT_IDLE_INTERVAL_MIN),
      );
      window.localStorage.setItem(
        IDLE_DURATION_SETTING_KEY,
        String(DEFAULT_IDLE_DURATION_MIN),
      );
    }

    if (hasAnyStored) {
      markSettingsInitialized();
    }
  } catch {
    // ignore local storage errors
  }
}

if (typeof window !== "undefined") {
  migrateLegacySettings();
}

export function readCompactIconLimit() {
  return readStoredSetting(
    COMPACT_ICON_SETTING_KEY,
    DEFAULT_MAX_COMPACT_ICONS,
    (value) => clampCompactIconLimit(value),
  );
}

export function readFoldedIslandSize(): FoldedIslandSize {
  if (typeof window === "undefined") return "small";
  try {
    const stored = window.localStorage.getItem(FOLDED_ISLAND_SIZE_SETTING_KEY);
    return stored === "regular" ? "regular" : "small";
  } catch {
    return "small";
  }
}

export function clampRetentionMinutes(value: number) {
  return Math.min(
    MAX_RETENTION_MINUTES,
    Math.max(MIN_RETENTION_MINUTES, Math.round(value)),
  );
}

export function readRetentionMinutes() {
  return readStoredSetting(
    RETENTION_SETTING_KEY,
    DEFAULT_RETENTION_MINUTES,
    clampRetentionMinutes,
  );
}

export function readSubagentRetentionMinutes() {
  return readStoredSetting(
    SUBAGENT_RETENTION_SETTING_KEY,
    DEFAULT_SUBAGENT_RETENTION_MINUTES,
    clampRetentionMinutes,
  );
}

export function clampMaxSubagentDisplay(value: number) {
  return Math.min(
    MAX_MAX_SUBAGENT_DISPLAY,
    Math.max(MIN_MAX_SUBAGENT_DISPLAY, Math.round(value)),
  );
}

export function readMaxSubagentDisplay() {
  return readStoredSetting(
    MAX_SUBAGENT_DISPLAY_SETTING_KEY,
    DEFAULT_MAX_SUBAGENT_DISPLAY,
    clampMaxSubagentDisplay,
  );
}

export function clampIdleInterval(v: number) {
  return Math.min(MAX_IDLE_INTERVAL_MIN, Math.max(MIN_IDLE_INTERVAL_MIN, Math.round(v)));
}
export function readIdleInterval() {
  return readStoredSetting(
    IDLE_INTERVAL_SETTING_KEY,
    DEFAULT_IDLE_INTERVAL_MIN,
    clampIdleInterval,
  );
}

export function clampIdleDuration(v: number) {
  return Math.min(MAX_IDLE_DURATION_MIN, Math.max(MIN_IDLE_DURATION_MIN, Math.round(v)));
}
export function readIdleDuration() {
  return readStoredSetting(
    IDLE_DURATION_SETTING_KEY,
    DEFAULT_IDLE_DURATION_MIN,
    clampIdleDuration,
  );
}
