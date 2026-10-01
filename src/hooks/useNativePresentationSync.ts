// Mirrors presentation changes to the native window (micro/compact/dormant/
// expanded) with change-key dedup. collapseIsland / expandIsland pre-mark the
// matching key so we do not replay the same native animation right after a
// user-driven transition finishes. Extracted verbatim from App.tsx.
import { useEffect, useRef } from "react";
import {
  compactPresentationKey,
  expandedPresentationKey,
} from "../islandLayout";
import type { PresentationPhase } from "../islandPresentation";

interface UseNativePresentationSyncOptions {
  phase: PresentationPhase;
  phaseRef: { current: PresentationPhase };
  suppressPostCollapseSyncRef: { current: boolean };
  microPresentationWidthRef: { current: number };
  lastNativePresentationKeyRef: { current: string | null };
  syncNativeIslandPresentation: (
    mode: "micro" | "compact" | "dormant" | "expanded",
    width?: number,
    idleExpanded?: boolean,
    leftPaneWidth?: number,
    planExpanded?: boolean,
    settingsExpanded?: boolean,
    forceSnap?: boolean,
  ) => Promise<void>;
  collapsedMode: "micro" | "compact" | "dormant";
  collapsedWindowWidth: number;
  compactLeftPaneWidth: number;
  isIdleExpanded: boolean;
  isPlanExpanded: boolean;
  isSettingsExpanded: boolean;
  nativeExpandedPlan: boolean;
  nativeExpandedSettings: boolean;
  notchMetricsHydrated: boolean;
  /** Notched display: collapsed-mode transitions snap the native window (the
   *  compact grid renders in the same commit as the new column widths, and an
   *  animated resize would render the notch spacer off the physical housing). */
  notchHasNotch: boolean;
}

export function useNativePresentationSync({
  phase,
  phaseRef,
  suppressPostCollapseSyncRef,
  microPresentationWidthRef,
  lastNativePresentationKeyRef,
  syncNativeIslandPresentation,
  collapsedMode,
  collapsedWindowWidth,
  compactLeftPaneWidth,
  isIdleExpanded,
  isPlanExpanded,
  isSettingsExpanded,
  nativeExpandedPlan,
  nativeExpandedSettings,
  notchMetricsHydrated,
  notchHasNotch,
}: UseNativePresentationSyncOptions) {
  // Collapsed-mode transitions (dormant ↔ compact ↔ micro) snap the native
  // window on notched displays: the compact header grid renders in the same
  // React commit as the new column widths, and an animated resize would draw
  // the notch spacer off the physical housing — worst on the first agent
  // logo, where the compact grid overflows the still-dormant window.
  // Same-mode width tweaks keep the smooth spring; their transient offset is
  // bounded by one icon slot. Non-notched displays have no alignment
  // constraint and keep the animated morph.
  const prevCollapsedModeRef = useRef(collapsedMode);
  useEffect(() => {
    const collapsedModeChanged = prevCollapsedModeRef.current !== collapsedMode;
    prevCollapsedModeRef.current = collapsedMode;
    if (
      phaseRef.current === "opening" ||
      phaseRef.current === "closing" ||
      phase === "opening" ||
      phase === "closing"
    ) {
      return;
    }

    if (suppressPostCollapseSyncRef.current) {
      suppressPostCollapseSyncRef.current = false;
      return;
    }

    if (phase === "micro") {
      const microWidth = microPresentationWidthRef.current;
      const key = compactPresentationKey("micro", microWidth, 0);
      if (lastNativePresentationKeyRef.current === key) return;
      lastNativePresentationKeyRef.current = key;
      syncNativeIslandPresentation(
        "micro",
        microWidth,
        undefined,
        undefined,
        undefined,
        undefined,
        notchHasNotch && collapsedModeChanged,
      ).catch(() => undefined);
      return;
    }

    if (phase === "compact") {
      const key = compactPresentationKey(
        collapsedMode,
        collapsedWindowWidth,
        compactLeftPaneWidth,
      );
      if (lastNativePresentationKeyRef.current === key) return;
      lastNativePresentationKeyRef.current = key;
      if (collapsedMode === "dormant") {
        syncNativeIslandPresentation(
          "dormant",
          undefined,
          undefined,
          undefined,
          undefined,
          undefined,
          notchHasNotch && collapsedModeChanged,
        ).catch(() => undefined);
      } else {
        syncNativeIslandPresentation(
          "compact",
          collapsedWindowWidth,
          undefined,
          compactLeftPaneWidth,
          undefined,
          undefined,
          notchHasNotch && collapsedModeChanged,
        ).catch(() => undefined);
      }
      return;
    }

    if (phase === "expanded") {
      const key = expandedPresentationKey(
        isIdleExpanded,
        nativeExpandedPlan,
        nativeExpandedSettings,
      );
      if (lastNativePresentationKeyRef.current === key) return;
      const previousKey = lastNativePresentationKeyRef.current;
      lastNativePresentationKeyRef.current = key;
      syncNativeIslandPresentation(
        "expanded",
        undefined,
        isIdleExpanded,
        undefined,
        nativeExpandedPlan,
        nativeExpandedSettings,
      ).catch(() => {
        lastNativePresentationKeyRef.current = previousKey;
      });
    }
  }, [
    phase,
    collapsedWindowWidth,
    compactLeftPaneWidth,
    collapsedMode,
    isIdleExpanded,
    isPlanExpanded,
    isSettingsExpanded,
    nativeExpandedPlan,
    nativeExpandedSettings,
    notchMetricsHydrated,
  ]);
}
