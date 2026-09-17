// Snapshot ingestion: initial load, event-stream subscription, capture-mode
// handlers, and the applySnapshot merge used by every consumer of fresh
// backend state. Extracted from App.tsx; behavior unchanged.
import type { Dispatch, SetStateAction } from "react";
import { useEffect, useRef } from "react";
import { toPng } from "html-to-image";
import {
  captureProvideScreenshot,
  getSnapshot,
  normalizeSnapshot,
  onCaptureCollapseRequested,
  onCaptureOpenHooksRequested,
  onCaptureScreenshotRequested,
  onSnapshotChanged,
  setIslandPresentation,
  type IslandSnapshot,
} from "../tauri";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { mergeHookHealthPreferReady } from "../hookHealth";
import { snapshotHasPlanPending } from "../planMode";
import type { PanelView } from "../appTypes";
import type { PresentationPhase } from "../islandPresentation";

interface UseSnapshotStreamOptions {
  snapshotRef: { current: IslandSnapshot };
  setSnapshot: Dispatch<SetStateAction<IslandSnapshot>>;
  phaseRef: { current: PresentationPhase };
  expandIsland: () => void;
  scheduleIdleCollapse: () => void;
  frozenCollapseWidthRef: { current: number | null };
  collapseIsland: (skipAnimation?: boolean) => void;
  openHooksPage: (page: "home") => void;
  suppressHoverExpandRef: { current: boolean };
  panelViewRef: { current: PanelView };
  collapsedModeRef: { current: "micro" | "compact" | "dormant" };
  collapsedWindowWidthRef: { current: number };
  compactLeftPaneWidthRef: { current: number };
  setHookHealthHydrated: (hydrated: boolean) => void;
}

export function useSnapshotStream({
  snapshotRef,
  setSnapshot,
  phaseRef,
  expandIsland,
  scheduleIdleCollapse,
  frozenCollapseWidthRef,
  collapseIsland,
  openHooksPage,
  suppressHoverExpandRef,
  panelViewRef,
  collapsedModeRef,
  collapsedWindowWidthRef,
  compactLeftPaneWidthRef,
  setHookHealthHydrated,
}: UseSnapshotStreamOptions) {
  const snapshotLoadSeqRef = useRef(0);

  function applySnapshot(
    nextSnapshot: IslandSnapshot,
    options?: { mergeHookHealth?: boolean },
  ) {
    const normalized = normalizeSnapshot(nextSnapshot);
    const hookHealth = options?.mergeHookHealth
      ? mergeHookHealthPreferReady(
          snapshotRef.current.hookHealth,
          normalized.hookHealth,
        )
      : normalized.hookHealth;
    const merged = { ...normalized, hookHealth };
    snapshotRef.current = merged;
    if (phaseRef.current === "opening" || phaseRef.current === "closing") {
      // Hook health must update immediately after install — waiting for the
      // presentation transition leaves the header logo stuck in the dead state.
      setSnapshot((previous) => ({
        ...previous,
        hookHealth: merged.hookHealth,
        online: merged.online,
        sessions: merged.sessions,
        dailyTokens: merged.dailyTokens,
        activeSessionTokens: merged.activeSessionTokens,
        pendingCount: merged.pendingCount,
        archivedCount: merged.archivedCount,
        recent: merged.recent,
        activeRequest: merged.activeRequest,
      }));
      return;
    }
    setSnapshot(merged);

    if (merged.pendingCount > 0) {
      expandIsland();
    } else {
      const collapseInFlight = frozenCollapseWidthRef.current !== null;
      if (!collapseInFlight) {
        scheduleIdleCollapse();
      }
    }
  }

  function invalidatePendingSnapshotLoads() {
    snapshotLoadSeqRef.current += 1;
  }

  useEffect(() => {
    const loadSnapshot = () => {
      const seq = snapshotLoadSeqRef.current;
      getSnapshot()
        .then((nextSnapshot) => {
          if (seq !== snapshotLoadSeqRef.current) return;
          applySnapshot(nextSnapshot, { mergeHookHealth: true });
          setHookHealthHydrated(true);
        })
        .catch(() => undefined);
    };

    const refreshHookHealth = () => {
      getSnapshot()
        .then((nextSnapshot) => {
          applySnapshot(nextSnapshot, { mergeHookHealth: true });
          setHookHealthHydrated(true);
        })
        .catch(() => undefined);
    };

    loadSnapshot();
    const retryTimer = window.setTimeout(refreshHookHealth, 750);
    const unsubscribe = manageAsyncUnlisten(
      onSnapshotChanged((nextSnapshot) => {
        applySnapshot(nextSnapshot, { mergeHookHealth: true });
        setHookHealthHydrated(true);
      }),
    );
    const unsubscribeCapture = manageAsyncUnlisten(
      onCaptureCollapseRequested(() => {
        collapseIsland(true);
      }),
    );
    const unsubscribeCaptureHooks = manageAsyncUnlisten(
      onCaptureOpenHooksRequested(() => {
        getSnapshot()
          .then(applySnapshot)
          .catch(() => undefined)
          .finally(() => {
            openHooksPage("home");
            suppressHoverExpandRef.current = false;
            expandIsland();
          });
      }),
    );
    const unsubscribeScreenshot = manageAsyncUnlisten(
      onCaptureScreenshotRequested(async () => {
        const stage = document.querySelector<HTMLElement>(".stage");
        if (!stage) return;

        const phase = phaseRef.current;
        if (phase === "compact" && collapsedModeRef.current !== "dormant") {
          await setIslandPresentation(
            "compact",
            collapsedWindowWidthRef.current,
            undefined,
            compactLeftPaneWidthRef.current,
            false,
            true,
          );
        } else if (phase === "expanded") {
          const idleExpanded =
            snapshotRef.current.pendingCount === 0 &&
            snapshotRef.current.sessions.length === 0;
          const planExpanded = snapshotHasPlanPending(snapshotRef.current);
          const settingsExpanded =
            panelViewRef.current.kind === "settings" ||
            panelViewRef.current.kind === "clipboard" ||
            panelViewRef.current.kind === "fileStation" ||
            panelViewRef.current.kind === "history";
          await setIslandPresentation(
            "expanded",
            collapsedWindowWidthRef.current,
            idleExpanded,
            compactLeftPaneWidthRef.current,
            false,
            true,
            planExpanded && !settingsExpanded,
            settingsExpanded,
          );
        }

        await new Promise<void>((resolve) => {
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
        });
        await new Promise<void>((resolve) => window.setTimeout(resolve, 120));

        try {
          const dataUrl = await toPng(stage, {
            pixelRatio: window.devicePixelRatio || 2,
            backgroundColor: "#0a0b0d",
            cacheBust: true,
          });
          const base64 = dataUrl.slice(dataUrl.indexOf(",") + 1);
          await captureProvideScreenshot(base64);
        } catch (error) {
          console.error("[Atoll] capture screenshot failed", error);
        }
      }),
    );

    return () => {
      snapshotLoadSeqRef.current += 1;
      window.clearTimeout(retryTimer);
      unsubscribe();
      unsubscribeCapture();
      unsubscribeCaptureHooks();
      unsubscribeScreenshot();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { applySnapshot, invalidatePendingSnapshotLoads };
}
