// Compact/dormant layout derivation: collapsed width and left-pane width with
// the compact header layout, collapsed mode, and refs the presentation FSM reads.
// Freeze the rendered header during a resize while keeping native targets live.
import { useEffect, useMemo, useRef } from "react";
import type { NowPlayingTrack, NotchMetrics } from "../tauri";
import type { LyricPayload } from "../tauri";
import {
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  compactOuterPadding,
  computeMaxCompactIconLimit,
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
        bluetoothRingCount,
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
      bluetoothRingCount,
    ],
  );
  const collapsedWindowWidth = computedCollapsedWidth;
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

  const liveHeaderLayout = useMemo(() => computeCompactHeaderLayout(
    notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount,
  ), [notchMetrics, sessions.length, maxCompactIcons, activeSessionTokenTotal, pendingCount]);
  const stableHeaderLayoutRef = useRef(liveHeaderLayout);
  const transitioning = phase === "opening" || phase === "closing";
  if (!transitioning) stableHeaderLayoutRef.current = liveHeaderLayout;
  const compactHeaderLayout = transitioning ? stableHeaderLayoutRef.current : liveHeaderLayout;

  const computedLeftPaneWidth = useMemo(
    () =>
      computeCompactLeftPaneWidth(
        liveHeaderLayout,
        compactOuterPadding(notchMetrics),
      ),
    [liveHeaderLayout, notchMetrics],
  );
  const compactLeftPaneWidth = computedLeftPaneWidth;

  collapsedModeRef.current = collapsedMode;
  collapsedWindowWidthRef.current = collapsedWindowWidth;
  compactLeftPaneWidthRef.current = compactLeftPaneWidth;
  microPresentationWidthRef.current = microPresentationWidth(
    sessions.length,
    activeSessionTokenTotal,
    liveHeaderLayout.tokenCompactLevel,
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
