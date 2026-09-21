// Compact/dormant layout derivation: collapsed width and left-pane width with
// their animation-hold freeze semantics, the compact header layout, the
// collapsed mode resolution, and the mirror refs the presentation FSM reads.
// Extracted from App.tsx; behavior unchanged.
import { useEffect, useMemo, useRef } from "react";
import type { NowPlayingTrack, NotchMetrics } from "../tauri";
import type { LyricPayload } from "../tauri";
import {
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  computeMaxCompactIconLimit,
  type CompactHeaderLayout,
} from "../compactLayout";
import {
  microPresentationWidth,
  resolveCollapsedMode,
} from "../islandLayout";
import type { CompactIndicatorMode } from "../displayPrefs";
import type { PresentationPhase } from "../islandPresentation";
import { setCompactLayout } from "../tauri";
import type { SessionSummary } from "../tauri/types";

interface UseCompactLayoutOptions {
  notchMetrics: NotchMetrics;
  sessions: SessionSummary[];
  maxCompactIcons: number;
  activeSessionTokenTotal: number;
  pendingCount: number;
  nowPlayingTrack: NowPlayingTrack | null;
  compactIndicator: CompactIndicatorMode;
  lyricsEnabled: boolean;
  lyricsData: LyricPayload | null;
  /** Battery ring has data: the island keeps compact mode instead of dormant. */
  bluetoothRingActive: boolean;
  phase: PresentationPhase;
  phaseRef: { current: PresentationPhase };
  usesMicroIslandRef: { current: boolean };
  supportsMicroIsland: boolean;
  suppressPostCollapseSyncRef: { current: boolean };
  holdCompactAfterSubviewOpenRef: { current: boolean };
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
  pendingCount,
  nowPlayingTrack,
  compactIndicator,
  lyricsEnabled,
  lyricsData,
  bluetoothRingActive,
  phase,
  phaseRef,
  usesMicroIslandRef,
  supportsMicroIsland,
  suppressPostCollapseSyncRef,
  holdCompactAfterSubviewOpenRef,
  collapsedModeRef,
  collapsedWindowWidthRef,
  compactLeftPaneWidthRef,
  microPresentationWidthRef,
}: UseCompactLayoutOptions) {
  const maxCompactIconLimit = useMemo(
    () => computeMaxCompactIconLimit(notchMetrics),
    [notchMetrics],
  );
  const computedCollapsedWidth = useMemo(
    () =>
      computeCollapsedWindowWidth(
        notchMetrics,
        sessions.length,
        maxCompactIcons,
        activeSessionTokenTotal,
        pendingCount,
        nowPlayingTrack?.artworkBase64 != null,
        compactIndicator === "media" || compactIndicator === "both",
        lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0,
        bluetoothRingActive,
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      pendingCount,
      nowPlayingTrack?.artworkBase64,
      compactIndicator,
      lyricsEnabled,
      lyricsData,
      bluetoothRingActive,
    ],
  );
  const stableWidthRef = useRef(computedCollapsedWidth);
  const hasActiveSessions = sessions.length > 0;
  const collapsedWindowWidth = useMemo(() => {
    if (!hasActiveSessions) {
      if (
        phaseRef.current === "expanded" ||
        phaseRef.current === "opening" ||
        phaseRef.current === "closing" ||
        suppressPostCollapseSyncRef.current
      ) {
        return stableWidthRef.current;
      }
      stableWidthRef.current = computedCollapsedWidth;
      return computedCollapsedWidth;
    }
    if (computedCollapsedWidth > stableWidthRef.current) {
      stableWidthRef.current = computedCollapsedWidth;
    }
    return stableWidthRef.current;
  }, [computedCollapsedWidth, hasActiveSessions]);
  const rawCollapsedMode = resolveCollapsedMode(
    usesMicroIslandRef.current,
    supportsMicroIsland,
    sessions.length,
    pendingCount,
    phase,
    // When lyrics are active, stay in compact mode (not dormant) so the
    // header has room for the lyrics column. Dormant mode is too narrow.
    lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0 && !notchMetrics.hasNotch,
    // Same for the Bluetooth battery ring: it lives in the compact header.
    bluetoothRingActive,
  );
  const collapsedMode: "micro" | "compact" | "dormant" =
    (suppressPostCollapseSyncRef.current ||
      holdCompactAfterSubviewOpenRef.current) &&
    (rawCollapsedMode === "dormant" || rawCollapsedMode === "micro")
      ? "compact"
      : rawCollapsedMode;

  const stableHeaderLayoutRef = useRef(
    computeCompactHeaderLayout(
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      pendingCount,
    ),
  );
  const compactHeaderLayout = useMemo(() => {
    const computed = computeCompactHeaderLayout(
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      pendingCount,
    );
    // Hold the pre-transition layout during opening/closing so a session
    // resolving or pending count changing mid-animation cannot reflow the
    // header icons. Mirrors the stableLeftWidthRef freeze below.
    if (
      phaseRef.current === "opening" ||
      phaseRef.current === "closing"
    ) {
      return stableHeaderLayoutRef.current;
    }
    stableHeaderLayoutRef.current = computed;
    return computed;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    notchMetrics,
    sessions.length,
    maxCompactIcons,
    activeSessionTokenTotal,
    pendingCount,
  ]);

  const computedLeftPaneWidth = useMemo(
    () => computeCompactLeftPaneWidth(compactHeaderLayout),
    [compactHeaderLayout],
  );
  const stableLeftWidthRef = useRef(computedLeftPaneWidth);
  const compactLeftPaneWidth = useMemo(() => {
    if (!hasActiveSessions) {
      if (
        phaseRef.current === "expanded" ||
        phaseRef.current === "opening" ||
        phaseRef.current === "closing" ||
        suppressPostCollapseSyncRef.current
      ) {
        return stableLeftWidthRef.current;
      }
      stableLeftWidthRef.current = computedLeftPaneWidth;
      return computedLeftPaneWidth;
    }
    if (computedLeftPaneWidth > stableLeftWidthRef.current) {
      stableLeftWidthRef.current = computedLeftPaneWidth;
    }
    return stableLeftWidthRef.current;
  }, [computedLeftPaneWidth, hasActiveSessions]);

  collapsedModeRef.current = collapsedMode;
  collapsedWindowWidthRef.current = collapsedWindowWidth;
  compactLeftPaneWidthRef.current = compactLeftPaneWidth;
  microPresentationWidthRef.current = microPresentationWidth(
    sessions.length,
    activeSessionTokenTotal,
    compactHeaderLayout.tokenCompactLevel,
  );

  useEffect(() => {
    if (typeof document === "undefined") return;
    document.documentElement.style.setProperty(
      "--compact-left-pane-width",
      `${compactLeftPaneWidth}px`,
    );
  }, [compactLeftPaneWidth]);

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
  };
}

