import { useEffect, useState } from "react";
import {
  isAutostartEnabled,
  enableAutostart,
  disableAutostart,
  setSessionRetention,
  setSubagentRetention,
} from "../tauri";
import {
  COMPACT_ICON_SETTING_KEY,
  RETENTION_SETTING_KEY,
  SUBAGENT_RETENTION_SETTING_KEY,
  MAX_SUBAGENT_DISPLAY_SETTING_KEY,
  IDLE_INTERVAL_SETTING_KEY,
  IDLE_DURATION_SETTING_KEY,
  readCompactIconLimit,
  readRetentionMinutes,
  readSubagentRetentionMinutes,
  readMaxSubagentDisplay,
  readIdleInterval,
  readIdleDuration,
  markSettingsInitialized,
} from "../settingsStorage";
import {
  readCompactIndicator,
  readDisplayMode,
  writeCompactIndicator,
  writeDisplayMode,
  type CompactIndicatorMode,
  type UsageDisplayMode,
  EXPANDED_COUNTER_DISPLAY_KEY,
  FOLDED_COUNTER_DISPLAY_KEY,
  HEATMAP_DISPLAY_KEY,
  SETTINGS_BADGE_DISPLAY_KEY,
} from "../displayPrefs";

/**
 * Slider / display-preference state with its localStorage + backend
 * persistence. Initial values are read synchronously so the first frame
 * already shows the stored settings.
 */
export function useDisplayAndSettingsPrefs() {
  const [maxCompactIcons, setMaxCompactIcons] = useState<number>(() => readCompactIconLimit());
  const [retentionMinutes, setRetentionMinutes] = useState<number>(() => readRetentionMinutes());
  const [subagentRetentionMinutes, setSubagentRetentionMinutes] = useState<number>(() => readSubagentRetentionMinutes());
  const [maxSubagentDisplay, setMaxSubagentDisplay] = useState<number>(() => readMaxSubagentDisplay());
  const [idleIntervalSec, setIdleIntervalSec] = useState<number>(() => readIdleInterval());
  const [idleDurationSec, setIdleDurationSec] = useState<number>(() => readIdleDuration());
  const [foldedCounterDisplay, setFoldedCounterDisplay] = useState<UsageDisplayMode>(() =>
    readDisplayMode(FOLDED_COUNTER_DISPLAY_KEY),
  );
  const [compactIndicator, setCompactIndicatorState] = useState<CompactIndicatorMode>(() =>
    readCompactIndicator(),
  );
  const [expandedCounterDisplay, setExpandedCounterDisplay] = useState<UsageDisplayMode>(() =>
    readDisplayMode(EXPANDED_COUNTER_DISPLAY_KEY),
  );
  const [settingsBadgeDisplay, setSettingsBadgeDisplay] = useState<UsageDisplayMode>(() =>
    readDisplayMode(SETTINGS_BADGE_DISPLAY_KEY),
  );
  const [heatmapDisplay, setHeatmapDisplay] = useState<UsageDisplayMode>(() =>
    readDisplayMode(HEATMAP_DISPLAY_KEY),
  );
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [launchAtLoginBusy, setLaunchAtLoginBusy] = useState(false);

  useEffect(() => {
    writeDisplayMode(FOLDED_COUNTER_DISPLAY_KEY, foldedCounterDisplay);
  }, [foldedCounterDisplay]);

  useEffect(() => {
    writeCompactIndicator(compactIndicator);
  }, [compactIndicator]);

  useEffect(() => {
    writeDisplayMode(EXPANDED_COUNTER_DISPLAY_KEY, expandedCounterDisplay);
  }, [expandedCounterDisplay]);

  useEffect(() => {
    writeDisplayMode(SETTINGS_BADGE_DISPLAY_KEY, settingsBadgeDisplay);
  }, [settingsBadgeDisplay]);

  useEffect(() => {
    writeDisplayMode(HEATMAP_DISPLAY_KEY, heatmapDisplay);
  }, [heatmapDisplay]);

  useEffect(() => {
    try {
      window.localStorage.setItem(
        COMPACT_ICON_SETTING_KEY,
        String(maxCompactIcons),
      );
    } catch {
      // ignore local storage errors
    }
  }, [maxCompactIcons]);

  useEffect(() => {
    try {
      window.localStorage.setItem(
        RETENTION_SETTING_KEY,
        String(retentionMinutes),
      );
    } catch {
      // ignore local storage errors
    }
    setSessionRetention(retentionMinutes).catch(() => undefined);
  }, [retentionMinutes]);

  useEffect(() => {
    try {
      window.localStorage.setItem(SUBAGENT_RETENTION_SETTING_KEY, String(subagentRetentionMinutes));
    } catch {}
    setSubagentRetention(subagentRetentionMinutes).catch(() => undefined);
  }, [subagentRetentionMinutes]);

  useEffect(() => {
    try {
      window.localStorage.setItem(MAX_SUBAGENT_DISPLAY_SETTING_KEY, String(maxSubagentDisplay));
    } catch {
      // ignore local storage errors
    }
  }, [maxSubagentDisplay]);

  useEffect(() => {
    try { window.localStorage.setItem(IDLE_INTERVAL_SETTING_KEY, String(idleIntervalSec)); } catch {}
  }, [idleIntervalSec]);

  useEffect(() => {
    try { window.localStorage.setItem(IDLE_DURATION_SETTING_KEY, String(idleDurationSec)); } catch {}
  }, [idleDurationSec]);

  useEffect(() => {
    isAutostartEnabled()
      .then(setLaunchAtLogin)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    markSettingsInitialized();
  }, []);

  async function handleChangeLaunchAtLogin(enabled: boolean) {
    if (launchAtLoginBusy) {
      return;
    }

    const previous = launchAtLogin;
    setLaunchAtLogin(enabled);
    setLaunchAtLoginBusy(true);
    try {
      if (enabled) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
    } catch (error) {
      setLaunchAtLogin(previous);
      console.error("[Atoll] autostart toggle failed", error);
    } finally {
      setLaunchAtLoginBusy(false);
    }
  }

  return {
    maxCompactIcons,
    setMaxCompactIcons,
    retentionMinutes,
    setRetentionMinutes,
    subagentRetentionMinutes,
    setSubagentRetentionMinutes,
    maxSubagentDisplay,
    setMaxSubagentDisplay,
    idleIntervalSec,
    setIdleIntervalSec,
    idleDurationSec,
    setIdleDurationSec,
    foldedCounterDisplay,
    setFoldedCounterDisplay,
    compactIndicator,
    setCompactIndicatorState,
    expandedCounterDisplay,
    setExpandedCounterDisplay,
    settingsBadgeDisplay,
    setSettingsBadgeDisplay,
    heatmapDisplay,
    setHeatmapDisplay,
    launchAtLogin,
    launchAtLoginBusy,
    handleChangeLaunchAtLogin,
  };
}
