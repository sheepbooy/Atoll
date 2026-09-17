// Mirrors presentation changes to the native window (micro/compact/dormant/
// expanded) with change-key dedup. collapseIsland / expandIsland pre-mark the
// matching key so we do not replay the same native animation right after a
// user-driven transition finishes. Extracted verbatim from App.tsx.
import { useEffect } from "react";
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
}: UseNativePresentationSyncOptions) {
  useEffect(() => {
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
      syncNativeIslandPresentation("micro", microWidth).catch(
        () => undefined,
      );
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
        syncNativeIslandPresentation("dormant").catch(() => undefined);
      } else {
        syncNativeIslandPresentation(
          "compact",
          collapsedWindowWidth,
          undefined,
          compactLeftPaneWidth,
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
