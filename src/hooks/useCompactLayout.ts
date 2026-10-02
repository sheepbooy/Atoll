// Compact/dormant layout derivation: collapsed width and left-pane width with
// the compact header layout, collapsed mode, and refs the presentation FSM reads.
// Freeze the rendered header during a resize while keeping native targets live.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { NowPlayingTrack, NotchMetrics } from "../tauri";
import type { LyricPayload } from "../tauri";
import {
  computeCompactPresentation,
  computeCompactHeaderLayout,
  estimateCompactCounterWidths,
  computeMaxCompactIconLimit,
} from "../compactLayout";
import {
  microPresentationWidth,
  resolveCollapsedMode,
} from "../islandLayout";
import type { CompactIndicatorMode, UsageDisplayMode } from "../displayPrefs";
import type { SalarySettings } from "../salarySettings";
import { salaryEarnedToday } from "../salaryFormat";
import type { PresentationPhase } from "../islandPresentation";
import { setCompactLayout } from "../tauri";
import type { SessionSummary } from "../tauri/types";

interface UseCompactLayoutOptions {
  notchMetrics: NotchMetrics;
  sessions: SessionSummary[];
  maxCompactIcons: number;
  activeSessionTokenTotal: number;
  activeSessionCostTotal: number;
  foldedCounterDisplay: UsageDisplayMode;
  salarySettings: SalarySettings;
  pendingCount: number;
  nowPlayingTrack: NowPlayingTrack | null;
  compactIndicator: CompactIndicatorMode;
  lyricsEnabled: boolean;
  lyricsData: LyricPayload | null;
  /** Devices reporting a battery level (drives ring width + compact mode). */
  bluetoothRingCount: number;
  phase: PresentationPhase;
  phaseRef: { current: PresentationPhase };
  usesMicroIslandRef: { current: boolean };
  supportsMicroIsland: boolean;
  collapsedModeRef: { current: "micro" | "compact" | "dormant" };
  collapsedWindowWidthRef: { current: number };
  compactLeftPaneWidthRef: { current: number };
  microPresentationWidthRef: { current: number };
}

export function useCompactLayout({
  notchMetrics,
  sessions,
  maxCompactIcons,
  activeSessionTokenTotal,
  activeSessionCostTotal,
  foldedCounterDisplay,
  salarySettings,
  pendingCount,
  nowPlayingTrack,
  compactIndicator,
  lyricsEnabled,
  lyricsData,
  bluetoothRingCount,
  phase,
  phaseRef,
  usesMicroIslandRef,
  supportsMicroIsland,
  collapsedModeRef,
  collapsedWindowWidthRef,
  compactLeftPaneWidthRef,
  microPresentationWidthRef,
}: UseCompactLayoutOptions) {
  const maxCompactIconLimit = useMemo(
    () => computeMaxCompactIconLimit(notchMetrics),
    [notchMetrics],
  );
  const counterVisible = sessions.length > 0 &&
    (compactIndicator === "tokens" || compactIndicator === "both");
  const estimatedWidths = useMemo(() => estimateCompactCounterWidths(
    foldedCounterDisplay,
    foldedCounterDisplay === "salary" ? salaryEarnedToday(salarySettings, new Date())
      : foldedCounterDisplay === "cost" ? activeSessionCostTotal : activeSessionTokenTotal,
    salarySettings.currency,
  ), [foldedCounterDisplay, salarySettings, activeSessionCostTotal, activeSessionTokenTotal, phase]);
  // Measurements belong to a display format, not to every salary tick. Token
  // and cost estimates invalidate them when the target's digit width changes.
  const measurementKey = foldedCounterDisplay === "salary"
    ? `salary:${salarySettings.currency}:${salarySettings.monthlyWage}:${salarySettings.workDaysPerMonth}:${salarySettings.workHoursPerDay}`
    : `${foldedCounterDisplay}:${estimatedWidths.join(",")}`;
  const [measurement, setMeasurement] = useState<{ key: string; widths: Partial<Record<number, number>> } | null>(null);
  useEffect(() => {
    setMeasurement((previous) => phase === "compact" && previous?.key === measurementKey ? previous : null);
  }, [measurementKey, phase]);
  const onCompactCounterWidthChange = useCallback((width: number, level: number) => {
    if (phaseRef.current !== "compact" || !counterVisible || width <= 0) return;
    setMeasurement((previous) => {
      if (previous?.key === measurementKey && previous.widths[level] === width) return previous;
      const widths = previous?.key === measurementKey ? previous.widths : {};
      return { key: measurementKey, widths: { ...widths, [level]: width } };
    });
  }, [measurementKey, counterVisible, phaseRef]);
  const contentBudget = useMemo(() => ({
    counterWidths: counterVisible ? estimatedWidths.map((estimate, level) => {
      const measuredWidth = measurement?.key === measurementKey ? measurement.widths[level] : undefined;
      if (phase !== "compact" || measuredWidth === undefined) return estimate;
      // Animated token digits can still be shorter than their target. Salary
      // updates directly, so its measurement can shrink again at midnight.
      return foldedCounterDisplay === "salary" ? measuredWidth : Math.max(estimate, measuredWidth);
    }) : [],
    hasMediaArtwork: nowPlayingTrack?.artworkBase64 != null,
    showMediaIndicator: compactIndicator === "media" || compactIndicator === "both",
    showLyrics: lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0,
    batteryRingCount: bluetoothRingCount,
  }), [counterVisible, estimatedWidths, phase, measurement, measurementKey, foldedCounterDisplay,
    nowPlayingTrack?.artworkBase64, compactIndicator, lyricsEnabled, lyricsData, bluetoothRingCount]);
  const presentation = useMemo(() => computeCompactPresentation(
    notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount, contentBudget,
  ), [notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount, contentBudget]);
  const collapsedWindowWidth = presentation.windowWidth;
  const rawCollapsedMode = resolveCollapsedMode(
    usesMicroIslandRef.current,
    supportsMicroIsland,
    sessions.length,
    pendingCount,
    phase,
    // When lyrics are active, stay in compact mode (not dormant) so the
    // header has room for the lyrics column. Dormant mode is too narrow.
    lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0 && !notchMetrics.hasNotch,
    // Same for the Bluetooth battery rings: they live in the compact header.
    bluetoothRingCount > 0,
  );
  const collapsedMode = rawCollapsedMode;

  // Micro mode keeps its existing token-only geometry and formatting tiers.
  const microHeaderLayout = useMemo(() => computeCompactHeaderLayout(
    notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount,
  ), [notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount]);
  const liveHeaderLayout = phase === "micro" ? microHeaderLayout : presentation.layout;
  const stableHeaderLayoutRef = useRef(liveHeaderLayout);
  const transitioning = phase === "opening" || phase === "closing";
  if (!transitioning) stableHeaderLayoutRef.current = liveHeaderLayout;
  const compactHeaderLayout = transitioning ? stableHeaderLayoutRef.current : liveHeaderLayout;
  const compactLeftPaneWidth = presentation.leftPaneWidth;

  collapsedModeRef.current = collapsedMode;
  collapsedWindowWidthRef.current = collapsedWindowWidth;
  compactLeftPaneWidthRef.current = compactLeftPaneWidth;
  microPresentationWidthRef.current = microPresentationWidth(
    sessions.length,
    activeSessionTokenTotal,
    microHeaderLayout.tokenCompactLevel,
  );

  useEffect(() => {
    if (typeof document === "undefined") return;
    document.documentElement.style.setProperty("--compact-metrics-gap", `${presentation.metricsGap}px`);
    document.documentElement.style.setProperty(
      "--compact-left-pane-width",
      `${compactLeftPaneWidth}px`,
    );
  }, [compactLeftPaneWidth, presentation.metricsGap]);

  useEffect(() => {
    if (collapsedMode === "dormant" || phase === "micro") return;
    if (phase === "expanded" || phase === "opening" || phase === "closing") {
      return;
    }
    setCompactLayout(collapsedWindowWidth, compactLeftPaneWidth).catch(
      () => undefined,
    );
  }, [collapsedMode, collapsedWindowWidth, compactLeftPaneWidth, phase]);

  return {
    maxCompactIconLimit,
    collapsedWindowWidth,
    collapsedMode,
    compactHeaderLayout,
    compactLeftPaneWidth,
    onCompactCounterWidthChange,
  };
}
