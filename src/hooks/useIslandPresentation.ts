import {
  useEffect,
  useRef,
  useState,
  FocusEvent,
  MouseEvent,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  getNotchMetrics,
  setIslandPresentation,
  usesMicroIsland,
  usesMicroIslandSync,
  onIslandHoverChanged,
  onNotchMetricsChanged,
  onIslandOpenRequested,
  onIslandPresentationSettled,
  nextIslandTransitionId,
  type IslandSnapshot,
  type IslandPresentationOptions,
  type NotchMetrics,
} from "../tauri";
import {
  COLLAPSE_ANIMATION_MS,
  beginCollapse,
  beginExpand,
  finishExpand,
  DRAG_EXPAND_ANIMATION_MS,
  IDLE_COLLAPSE_DELAY_MS,
  MICRO_SHRINK_DELAY_MS,
  PRESENTATION_SETTLE_FALLBACK_MS,
  type PresentationPhase,
} from "../islandPresentation";
import {
  applyWindowMetrics,
  compactPresentationKey,
  expandedPresentationKey,
  microPresentationWidth,
  shouldRestInMicro,
  shouldUseMicroIsland,
} from "../islandLayout";
import {
  FOLDED_ISLAND_SIZE_SETTING_KEY,
  readFoldedIslandSize,
} from "../settingsStorage";
import { EMPTY_NOTCH_METRICS } from "../snapshotDefaults";
import { isPlanModeCommand, snapshotHasPlanPending } from "../planMode";
import { isTextEntryActive } from "../imeHelpers";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { motionDelay, useReducedMotion } from "../animationTiming";
import type { ArtworkBackdropOrigin, FoldedIslandSize, PanelView } from "../appTypes";

interface UseIslandPresentationOptions {
  snapshotRef: { current: IslandSnapshot };
  setSnapshot: (
    updater: IslandSnapshot | ((prev: IslandSnapshot) => IslandSnapshot),
  ) => void;
  collapsedModeRef: { current: "micro" | "compact" | "dormant" };
  collapsedWindowWidthRef: { current: number };
  compactLeftPaneWidthRef: { current: number };
  microPresentationWidthRef: { current: number };
  panelViewRef: { current: PanelView };
  navigationSeqRef: { current: number };
  setPanelView: (view: PanelView) => void;
  setNavDirection: (dir: "forward" | "back" | null) => void;
  tryBeginPanelExit: (onExited: () => void) => boolean;
  cancelPanelExit: () => void;
  clearPanelExitTimer: () => void;
  closeMenu: () => void;
  reminderHoldRef?: { current: boolean };
  compactUtilityRef?: { current: boolean };
}

/**
 * Island presentation FSM: phase (micro/compact/dormant/opening/closing/
 * expanded), native window animation handshakes, hover/summon handling,
 * folded-size state, notch metrics, and the artwork-backdrop presentation.
 */
export function useIslandPresentation({
  snapshotRef,
  setSnapshot,
  collapsedModeRef,
  collapsedWindowWidthRef,
  compactLeftPaneWidthRef,
  microPresentationWidthRef,
  panelViewRef,
  navigationSeqRef,
  setPanelView,
  setNavDirection,
  tryBeginPanelExit,
  cancelPanelExit,
  clearPanelExitTimer,
  closeMenu,
  reminderHoldRef,
  compactUtilityRef,
}: UseIslandPresentationOptions) {
  const reducedMotion = useReducedMotion();
  const initialSupportsMicroIsland = usesMicroIslandSync();
  const initialFoldedIslandSize = readFoldedIslandSize();
  const initialUsesMicro = shouldUseMicroIsland(
    initialSupportsMicroIsland,
    initialFoldedIslandSize,
  );
  const [supportsMicroIsland, setSupportsMicroIsland] = useState(
    initialSupportsMicroIsland,
  );
  const supportsMicroIslandRef = useRef(initialSupportsMicroIsland);
  const [foldedIslandSize, setFoldedIslandSize] =
    useState<FoldedIslandSize>(initialFoldedIslandSize);
  const foldedIslandSizeRef = useRef(initialFoldedIslandSize);
  const [phase, setPhase] = useState<PresentationPhase>(
    initialUsesMicro ? "micro" : "compact",
  );
  const phaseRef = useRef<PresentationPhase>(initialUsesMicro ? "micro" : "compact");
  const usesMicroIslandRef = useRef(initialUsesMicro);
  foldedIslandSizeRef.current = foldedIslandSize;
  supportsMicroIslandRef.current = supportsMicroIsland;
  usesMicroIslandRef.current = shouldUseMicroIsland(
    supportsMicroIsland,
    foldedIslandSize,
  );
  const [notchMetricsHydrated, setNotchMetricsHydrated] = useState(false);
  const [presentationReady, setPresentationReady] = useState(true);
  const expandedResizeSeqRef = useRef(0);
  const initialNativePresentationSyncedRef = useRef(false);
  const hoveringRef = useRef(false);
  const cursorOverIslandRef = useRef(false);
  // A hotkey summon holds the island open: until it collapses again, later
  // snapshot refreshes / blur events must not schedule the idle collapse.
  const summonHoldRef = useRef(false);
  const shrinkInFlightRef = useRef(false);
  const focusedRef = useRef(false);
  const suppressHoverExpandRef = useRef(false);
  // Set when the user manually collapses while plan requests are pending: the
  // ids of those plan requests. Later snapshot refreshes must not auto-expand
  // the island for them again until a different request shows up.
  const dismissedPlanRequestIdsRef = useRef<Set<string> | null>(null);
  // True while a native file drag hovers the island (see useFileStation):
  // expansions triggered meanwhile use the fast drag duration.
  const fileDragActiveRef = useRef(false);
  const transitionTimerRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  const frozenCollapseWidthRef = useRef<number | null>(null);
  const frozenCollapseLeftWidthRef = useRef<number | null>(null);
  // Closures captured at transition start, run when the native window emits
  // `island-presentation-settled` (or the 2s fallback fires). Captured by value
  // so the listener sees the metrics that were current when the transition began.
  const pendingExpandRef = useRef<(() => Promise<void>) | null>(null);
  const pendingCollapseRef = useRef<(() => Promise<void>) | null>(null);
  const lastNativePresentationKeyRef = useRef<string | null>(null);
  const activeTransitionIdRef = useRef(0);
  const closingTargetKeyRef = useRef<string | null>(null);
  const closingDeadlineRef = useRef(0);

  const [notchMetrics, setNotchMetrics] = useState<NotchMetrics>(EMPTY_NOTCH_METRICS);

  useEffect(() => {
    if (!reducedMotion) return;
    if (transitionTimerRef.current !== null) {
      window.clearTimeout(transitionTimerRef.current);
      transitionTimerRef.current = null;
    }
    // The invoke resolves after the native snap; retain its generation checks.
    if (phaseRef.current === "opening") void runPendingExpand();
    if (phaseRef.current === "closing") void runPendingCollapse();
  }, [reducedMotion, phase]);

  // Rect of the compact media thumb (window coords) captured right before the
  // expand animation starts; drives the artwork backdrop grow-from-thumb origin.
  const [artworkBackdropOrigin, setArtworkBackdropOrigin] =
    useState<ArtworkBackdropOrigin | null>(null);
  const artworkBackdropOriginRef = useRef<ArtworkBackdropOrigin | null>(null);
  const [artworkBackdropRevealed, setArtworkBackdropRevealed] = useState(false);
  const [artworkBackdropExitFade, setArtworkBackdropExitFade] = useState(false);
  const artworkBackdropExitFadeRef = useRef(false);
  const compactMediaThumbRef = useRef<HTMLImageElement | null>(null);

  // Artwork backdrop: grow from the compact thumb on expand, shrink back on
  // collapse, and drop the stale origin once the island rests collapsed again.
  useEffect(() => {
    if (phase === "opening" || phase === "expanded") {
      // Double rAF so the start frame is painted before the reveal
      // transition (grow from thumb, or plain fade-in without an origin)
      // kicks in.
      let inner = 0;
      const outer = requestAnimationFrame(() => {
        inner = requestAnimationFrame(() => setArtworkBackdropRevealed(true));
      });
      return () => {
        cancelAnimationFrame(outer);
        if (inner) cancelAnimationFrame(inner);
      };
    }
    if (phase === "closing") {
      setArtworkBackdropRevealed(false);
      return;
    }
    artworkBackdropOriginRef.current = null;
    setArtworkBackdropOrigin(null);
  }, [phase]);

  function handleChangeFoldedIslandSize(small: boolean) {
    const nextSize: FoldedIslandSize = small ? "small" : "regular";
    foldedIslandSizeRef.current = nextSize;
    usesMicroIslandRef.current = shouldUseMicroIsland(
      supportsMicroIslandRef.current,
      nextSize,
    );
    setFoldedIslandSize(nextSize);

    if (
      phaseRef.current === "opening" ||
      phaseRef.current === "closing" ||
      phaseRef.current === "expanded"
    ) {
      return;
    }

    if (small && phaseRef.current === "compact") {
      shrinkToMicro().catch(() => undefined);
    } else if (!small && phaseRef.current === "micro") {
      promoteToCompact({ skipExpand: true }).catch(() => undefined);
    }
  }

  useEffect(() => {
    if (!supportsMicroIsland) return;
    try {
      window.localStorage.setItem(
        FOLDED_ISLAND_SIZE_SETTING_KEY,
        foldedIslandSize,
      );
    } catch {
      // ignore local storage errors
    }
  }, [foldedIslandSize, supportsMicroIsland]);

  // FSM-aware pre-step shared by the settings-page openers in usePanelNavigation.
  function ensureExpandedSettingsPresentation() {
    if (phaseRef.current === "expanded" && panelViewRef.current.kind !== "settings") {
      const previousKey = lastNativePresentationKeyRef.current;
      lastNativePresentationKeyRef.current = expandedPresentationKey(false, false, true);
      syncNativeIslandPresentation({
        mode: "expanded",
        expandedIdle: false,
        expandedPlan: false,
        expandedSettings: true,
      }).catch(() => {
        lastNativePresentationKeyRef.current = previousKey;
      });
    }
  }


  useEffect(() => {
  usesMicroIsland()
    .then((enabled) => {
      setSupportsMicroIsland(enabled);
      supportsMicroIslandRef.current = enabled;
      usesMicroIslandRef.current = shouldUseMicroIsland(
        enabled,
        foldedIslandSizeRef.current,
      );
    })
    .catch(() => undefined);
  getNotchMetrics()
    .then((notch) => {
      setNotchMetrics(notch);
      applyWindowMetrics(notch);
    })
    .catch(() => {
      setNotchMetrics(EMPTY_NOTCH_METRICS);
      applyWindowMetrics(EMPTY_NOTCH_METRICS);
    })
    .finally(() => {
      setNotchMetricsHydrated(true);
    });

  const unsubscribeHover = manageAsyncUnlisten(
    onIslandHoverChanged(({ hovering, cursorOverWindow }) => {
      cursorOverIslandRef.current = cursorOverWindow;
      const dragExpandOptions = fileDragActiveRef.current
        ? { fast: true }
        : undefined;
      if (cursorOverWindow) {
        clearIdleTimer();
        if (
          !suppressHoverExpandRef.current &&
          (phaseRef.current === "closing" ||
            (shrinkInFlightRef.current && phaseRef.current === "micro"))
        ) {
          expandIsland(dragExpandOptions);
          return;
        }
      }
      hoveringRef.current = hovering;
      if (hovering) {
        if (!suppressHoverExpandRef.current) {
          expandIsland(dragExpandOptions);
        }
      } else if (!cursorOverWindow) {
        if (phaseRef.current !== "closing") {
          suppressHoverExpandRef.current = false;
        }
        if (
          phaseRef.current === "compact" &&
          shouldRestInMicro(usesMicroIslandRef.current)
        ) {
          scheduleShrinkToMicro();
        } else {
          scheduleIdleCollapse();
        }
      }
    }),
  );
  const unsubscribeOpen = manageAsyncUnlisten(
    onIslandOpenRequested((source) => {
      suppressHoverExpandRef.current = false;
      if (source === "summon") {
        // Toggle semantics: a hotkey summon holds the island open (no idle
        // auto-collapse); pressing it again puts it away.
        if (
          phaseRef.current === "expanded" ||
          phaseRef.current === "opening"
        ) {
          summonHoldRef.current = false;
          collapseIsland(true);
        } else {
          summonHoldRef.current = true;
          expandIsland();
        }
        return;
      }
      expandIsland();
      scheduleIdleCollapse();
    }),
  );
  const unsubscribeSettled = manageAsyncUnlisten(
    onIslandPresentationSettled(({ mode, transitionId }) => {
      if (transitionId !== activeTransitionIdRef.current) return Promise.resolve();
      if (phaseRef.current === "opening" && mode === "expanded") {
        if (transitionTimerRef.current !== null) {
          window.clearTimeout(transitionTimerRef.current);
          transitionTimerRef.current = null;
        }
        return runPendingExpand();
      }
      if (phaseRef.current === "closing") {
        if (transitionTimerRef.current !== null) {
          window.clearTimeout(transitionTimerRef.current);
          transitionTimerRef.current = null;
        }
        return runPendingCollapse();
      }
      return Promise.resolve();
    }),
  );
  const unsubscribeNotch = manageAsyncUnlisten(
    onNotchMetricsChanged((notch) => {
      setNotchMetrics(notch);
      applyWindowMetrics(notch);
      // Re-anchor the collapsed island onto the refreshed geometry with a
      // snap — the animated path would re-derive its target from the stale
      // home center. Expanded states re-derive on their next transition.
      if (
        phaseRef.current !== "expanded" &&
        phaseRef.current !== "opening" &&
        phaseRef.current !== "closing"
      ) {
        const mode: "micro" | "compact" | "dormant" = phaseRef.current;
        setIslandPresentation({
          mode,
          compactWidth: mode === "micro" ? microPresentationWidthRef.current : collapsedWindowWidthRef.current,
          compactLeftWidth: mode === "compact" ? compactLeftPaneWidthRef.current : 0,
          animate: false,
          snap: true,
        }).catch(() => undefined);
      }
    }),
  );
  return () => {
    unsubscribeHover();
    unsubscribeOpen();
    unsubscribeSettled();
    unsubscribeNotch();
    clearTransitionWork();
    clearIdleTimer();
  };
}, []);

  function setPresentationPhase(next: PresentationPhase) {
    expandedResizeSeqRef.current++;
    setPresentationReady(next === "expanded");
    phaseRef.current = next;
    setPhase(next);
    if (next === "expanded" || next === "compact") {
      setSnapshot(snapshotRef.current);
    }
  }

  /** Re-fetch notch metrics after the island moves to a different display. */
  async function refreshNotchMetrics() {
    try {
      const notch = await getNotchMetrics();
      setNotchMetrics(notch);
      applyWindowMetrics(notch);
    } catch {
      // keep the previous metrics when the backend is unreachable
    }
  }

  function syncNativeIslandPresentation(options: IslandPresentationOptions) {
    // Snap collapsed-mode transitions on notched displays to land the
    // native frame in the same commit as the new column widths — an animated
    // resize would draw the notch spacer off the physical housing.
    const snap =
      options.snap === true ||
      !notchMetricsHydrated ||
      !initialNativePresentationSyncedRef.current;
    const resizeSeq = ++expandedResizeSeqRef.current;
    if (options.mode === "expanded") setPresentationReady(false);
    return setIslandPresentation({
      ...options,
      animate: !snap,
      snap,
    }).finally(() => {
      if (expandedResizeSeqRef.current === resizeSeq && phaseRef.current === "expanded") {
        setPresentationReady(true);
      }
      if (notchMetricsHydrated) {
        initialNativePresentationSyncedRef.current = true;
      }
    });
  }

  function clearTransitionWork() {
    activeTransitionIdRef.current = nextIslandTransitionId();
    shrinkInFlightRef.current = false;
    if (transitionTimerRef.current !== null) {
      window.clearTimeout(transitionTimerRef.current);
      transitionTimerRef.current = null;
    }
    clearPanelExitTimer();
    // Drop any closures waiting on a settled event so a superseded transition
    // cannot fire its finalize step on the next expand/collapse.
    pendingExpandRef.current = null;
    pendingCollapseRef.current = null;
  }

  function clearIdleTimer() {
    if (idleTimerRef.current === null) return;
    window.clearTimeout(idleTimerRef.current);
    idleTimerRef.current = null;
  }

  async function promoteToCompact(options?: { skipExpand?: boolean }) {
    if (phaseRef.current !== "micro") return;
    clearIdleTimer();
    clearTransitionWork();
    const transitionId = activeTransitionIdRef.current;

    const idleCompact =
      snapshotRef.current.sessions.length === 0 &&
      snapshotRef.current.pendingCount === 0;
    const compactWidth = collapsedWindowWidthRef.current;
    const compactLeftWidth = idleCompact ? 0 : compactLeftPaneWidthRef.current;

    setPresentationPhase("compact");
    lastNativePresentationKeyRef.current = compactPresentationKey(
      "compact",
      compactWidth,
      compactLeftWidth,
    );

    try {
      await setIslandPresentation({
        mode: "compact",
        compactWidth,
        compactLeftWidth,
        transitionId,
      });
      if (activeTransitionIdRef.current !== transitionId) return;
      if (
        !options?.skipExpand &&
        hoveringRef.current &&
        !suppressHoverExpandRef.current
      ) {
        expandIsland();
      }
    } catch {
      if (activeTransitionIdRef.current === transitionId) setPresentationPhase("micro");
    }
  }

  async function shrinkToMicro() {
    if (phaseRef.current !== "compact") return;
    if (
      !shouldRestInMicro(usesMicroIslandRef.current)
    ) {
      return;
    }

    clearIdleTimer();
    clearTransitionWork();
    const transitionId = activeTransitionIdRef.current;
    setPresentationPhase("micro");
    const microWidth = microPresentationWidthRef.current;
    lastNativePresentationKeyRef.current = compactPresentationKey(
      "micro",
      microWidth,
      0,
    );
    shrinkInFlightRef.current = true;
    try {
      await setIslandPresentation({
        mode: "micro",
        compactWidth: microWidth,
        transitionId,
      });
    } catch {
      if (activeTransitionIdRef.current === transitionId) setPresentationPhase("compact");
    } finally {
      if (activeTransitionIdRef.current === transitionId) shrinkInFlightRef.current = false;
    }
  }

  function scheduleShrinkToMicro() {
    clearIdleTimer();
    if (
      hoveringRef.current ||
      cursorOverIslandRef.current ||
      snapshotRef.current.pendingCount > 0 ||
      isTextEntryActive() || compactUtilityRef?.current
    ) {
      return;
    }

    idleTimerRef.current = window.setTimeout(() => {
      idleTimerRef.current = null;
      if (
        hoveringRef.current ||
        cursorOverIslandRef.current ||
        phaseRef.current !== "compact" ||
        !shouldRestInMicro(usesMicroIslandRef.current)
      ) {
        return;
      }
      shrinkToMicro().catch(() => undefined);
    }, MICRO_SHRINK_DELAY_MS);
  }

  // Snapshot the compact media thumb rect before the expand flips the phase —
  // the thumb unmounts as soon as the island starts opening.
  function captureArtworkBackdropOrigin() {
    const rect = compactMediaThumbRef.current?.getBoundingClientRect();
    if (!rect || rect.width <= 0 || rect.height <= 0) return;
    const winW = window.innerWidth || 1;
    const winH = window.innerHeight || 1;
    const origin: ArtworkBackdropOrigin = {
      x: rect.left,
      y: rect.top,
      w: rect.width,
      h: rect.height,
      winW,
      winH,
    };
    artworkBackdropOriginRef.current = origin;
    setArtworkBackdropOrigin(origin);
  }

  async function expandIsland(options?: { fast?: boolean }) {
    clearIdleTimer();
    cancelPanelExit();

    const next = beginExpand(phaseRef.current);
    if (next === phaseRef.current) {
      // A file drag re-requesting an in-flight expansion shortens the native
      // animation instead of waiting out the default 420ms one.
      if (options?.fast && phaseRef.current === "opening") {
        startExpandTransition(DRAG_EXPAND_ANIMATION_MS);
      }
      return;
    }
    startExpandTransition(options?.fast ? DRAG_EXPAND_ANIMATION_MS : undefined);
  }

  /** File-drag state from useFileStation: while a native file drag hovers the
   *  island, expansions take the fast path, and the drag-enter itself forces
   *  an expansion (the hover poll can lag behind the drag events). */
  function setFileDragActive(active: boolean) {
    if (fileDragActiveRef.current === active) return;
    fileDragActiveRef.current = active;
    if (
      active &&
      !suppressHoverExpandRef.current &&
      (phaseRef.current === "compact" ||
        phaseRef.current === "micro" ||
        phaseRef.current === "closing")
    ) {
      expandIsland({ fast: true });
    }
  }

  function startExpandTransition(durationMs?: number) {
    clearTransitionWork();
    releaseFrozenCollapseMetrics();
    if (artworkBackdropExitFadeRef.current) {
      artworkBackdropExitFadeRef.current = false;
      setArtworkBackdropExitFade(false);
    }
    if (artworkBackdropOriginRef.current === null) {
      captureArtworkBackdropOrigin();
    }

    const idleExpanded =
      snapshotRef.current.pendingCount === 0 &&
      snapshotRef.current.sessions.length === 0;
    const planExpanded = snapshotHasPlanPending(snapshotRef.current);
    const settingsExpanded =
      panelViewRef.current.kind === "settings" ||
      panelViewRef.current.kind === "clipboard" ||
      panelViewRef.current.kind === "fileStation" ||
      panelViewRef.current.kind === "history" || panelViewRef.current.kind === "reminders";
    lastNativePresentationKeyRef.current = expandedPresentationKey(
      idleExpanded,
      planExpanded && !settingsExpanded,
      settingsExpanded,
    );
    setPresentationPhase("opening");
    const transitionId = activeTransitionIdRef.current;
    const nativeTransition = setIslandPresentation({
      mode: "expanded",
      compactWidth: collapsedWindowWidthRef.current,
      expandedIdle: idleExpanded,
      compactLeftWidth: compactLeftPaneWidthRef.current,
      animate: true,
      snap: false,
      expandedPlan: planExpanded && !settingsExpanded,
      expandedSettings: settingsExpanded,
      durationMs,
      transitionId,
    });
    pendingExpandRef.current = async () => {
      if (phaseRef.current !== "opening") return;
      try {
        await nativeTransition;
        if (activeTransitionIdRef.current !== transitionId) return;
        if (phaseRef.current === "opening") {
          if (notchMetricsHydrated) {
            initialNativePresentationSyncedRef.current = true;
          }
          setPresentationPhase(finishExpand("opening"));
        }
      } catch {
        if (activeTransitionIdRef.current !== transitionId) return;
        setPresentationPhase(usesMicroIslandRef.current ? "micro" : "compact");
      }
    };
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      // 2s fallback: only fires if the native `island-presentation-settled`
      // event never arrives (e.g. the `animate: false, snap: false`
      // fire-and-forget presentation path).
      await runPendingExpand();
    }, motionDelay(PRESENTATION_SETTLE_FALLBACK_MS));
    // Attach a rejection handler immediately, including when superseded before
    // a completion event arrives and its pending closure has been discarded.
    void nativeTransition.catch(() => {
      if (activeTransitionIdRef.current !== transitionId) return;
      if (transitionTimerRef.current !== null) window.clearTimeout(transitionTimerRef.current);
      transitionTimerRef.current = null;
      void runPendingExpand();
    });
  }

  function runPendingExpand() {
    const finalize = pendingExpandRef.current;
    pendingExpandRef.current = null;
    if (!finalize) return Promise.resolve();
    return finalize();
  }

  function collapsePresentationMode(): "micro" | "compact" | "dormant" {
    if (compactUtilityRef?.current) return "compact";
    if (shouldRestInMicro(usesMicroIslandRef.current)) return "micro";
    if (supportsMicroIslandRef.current) return "compact";
    return collapsedModeRef.current;

  }

  function collapsedRestPhase(): PresentationPhase {
    return collapsePresentationMode() === "micro" ? "micro" : "compact";
  }

  function resolveCollapseMetrics(): { width: number; leftWidth: number } {
    return {
      width: collapsedWindowWidthRef.current,
      leftWidth: compactLeftPaneWidthRef.current,
    };
  }

  function collapseCompactWidth(): number {
    return frozenCollapseWidthRef.current ?? collapsedWindowWidthRef.current;
  }

  function collapseCompactLeftWidth(): number {
    return frozenCollapseLeftWidthRef.current ?? compactLeftPaneWidthRef.current;
  }

  function releaseFrozenCollapseMetrics() {
    frozenCollapseWidthRef.current = null;
    frozenCollapseLeftWidthRef.current = null;
  }

  function collapseIsland(releaseFocus = false) {
    clearIdleTimer();
    closeMenu();

    // Fade panel content out before the native window shrink starts.
    if (
      phaseRef.current === "expanded" &&
      tryBeginPanelExit(() => collapseIslandNow(releaseFocus))
    ) {
      return;
    }

    collapseIslandNow(releaseFocus);
  }

  function latchDismissedPlanRequests() {
    const planIds = snapshotRef.current.recent
      .filter(
        (request) =>
          request.status === "pending" && isPlanModeCommand(request.command),
      )
      .map((request) => request.id);
    dismissedPlanRequestIdsRef.current =
      planIds.length > 0 ? new Set(planIds) : null;
  }

  function collapseIslandNow(releaseFocus = false, retarget = false) {
    summonHoldRef.current = false;
    const next = beginCollapse(phaseRef.current);
    if (next === phaseRef.current && !retarget) {
      if (releaseFocus) {
        focusedRef.current = false;
        if (document.activeElement instanceof HTMLElement) {
          document.activeElement.blur();
        }
      }
      return;
    }

    // Remember which plan requests were pending when the user put the island
    // away, so snapshot refreshes stop re-opening the plan display.
    latchDismissedPlanRequests();

    if (releaseFocus) {
      suppressHoverExpandRef.current = true;
      focusedRef.current = false;
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    }
    clearTransitionWork();
    if (!retarget) closingDeadlineRef.current = performance.now() + COLLAPSE_ANIMATION_MS;
    const collapseMetrics = resolveCollapseMetrics();
    frozenCollapseWidthRef.current = collapseMetrics.width;
    frozenCollapseLeftWidthRef.current = collapseMetrics.leftWidth;
    setPresentationPhase(next);
    ++navigationSeqRef.current;
    setNavDirection(null);
    setPanelView({ kind: "home" });

    const compactWidth = collapseCompactWidth();
    const compactLeftWidth = collapseCompactLeftWidth();
    const collapseMode = collapsePresentationMode();
    // The dormant island vanishes entirely, so its backdrop fades out instead
    // of shrinking back towards a thumb that will not reappear.
    const backdropExitFade = collapseMode === "dormant";
    if (artworkBackdropExitFadeRef.current !== backdropExitFade) {
      artworkBackdropExitFadeRef.current = backdropExitFade;
      setArtworkBackdropExitFade(backdropExitFade);
    }
    const collapsePresentationWidth =
      collapseMode === "micro" ? microPresentationWidthRef.current : compactWidth;

    lastNativePresentationKeyRef.current = compactPresentationKey(
      collapseMode,
      collapsePresentationWidth,
      compactLeftWidth,
    );

    closingTargetKeyRef.current = compactPresentationKey(collapseMode, collapsePresentationWidth, compactLeftWidth);
    const durationMs = Math.max(1, closingDeadlineRef.current - performance.now());
    const transitionId = activeTransitionIdRef.current;
    const nativeTransition = setIslandPresentation({
      mode: collapseMode,
      durationMs: Math.round(durationMs),
      compactWidth: collapseMode === "micro" ? microPresentationWidthRef.current : compactWidth,
      compactLeftWidth: collapseMode === "compact" ? compactLeftWidth : undefined,
      transitionId,
    });
    pendingCollapseRef.current = async () => {
      if (phaseRef.current !== "closing" || activeTransitionIdRef.current !== transitionId) return;
      try {
        await nativeTransition;
        if (phaseRef.current !== "closing" || activeTransitionIdRef.current !== transitionId) return;
        lastNativePresentationKeyRef.current = compactPresentationKey(collapseMode, collapsePresentationWidth, compactLeftWidth);
        releaseFrozenCollapseMetrics();
        setPresentationPhase(collapseMode === "micro" ? "micro" : "compact");
      } catch {
        if (activeTransitionIdRef.current !== transitionId) return;
        releaseFrozenCollapseMetrics();
        setPresentationPhase("expanded");
      } finally {
        if (activeTransitionIdRef.current === transitionId) {
          pendingCollapseRef.current = null;
          suppressHoverExpandRef.current = false;
        }
      }
    };
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      // 2s fallback: only fires if the native `island-presentation-settled`
      // event never arrives (e.g. the `animate: false, snap: false`
      // fire-and-forget presentation path).
      await runPendingCollapse();
    }, motionDelay(PRESENTATION_SETTLE_FALLBACK_MS));
    void nativeTransition.catch(() => {
      if (activeTransitionIdRef.current !== transitionId) return;
      if (transitionTimerRef.current !== null) window.clearTimeout(transitionTimerRef.current);
      transitionTimerRef.current = null;
      void runPendingCollapse();
    });
  }

  // Snapshot changes retarget the existing close, rather than resizing after settlement.
  useEffect(() => {
    if (phaseRef.current !== "closing") return;
    const mode = collapsePresentationMode();
    const width = mode === "micro" ? microPresentationWidthRef.current : collapsedWindowWidthRef.current;
    const key = compactPresentationKey(mode, width, compactLeftPaneWidthRef.current);
    if (key !== closingTargetKeyRef.current) collapseIslandNow(false, true);
  });

  function runPendingCollapse() {
    const finalize = pendingCollapseRef.current;
    pendingCollapseRef.current = null;
    if (!finalize) return Promise.resolve();
    return finalize();
  }

  function scheduleIdleCollapse() {
    if (reminderHoldRef?.current) return;
    if (summonHoldRef.current) {
      // A hotkey summon is holding the island open.
      return;
    }
    clearIdleTimer();
    // Only an active text field (e.g. the reply input) should hold the island
    // open once the pointer leaves. A lingering button focus — e.g. after
    // tapping "View session" — must NOT block the idle collapse.
    if (
      hoveringRef.current ||
      snapshotRef.current.pendingCount > 0 ||
      isTextEntryActive()
    ) {
      return;
    }

    idleTimerRef.current = window.setTimeout(() => {
      idleTimerRef.current = null;
      if (
        !hoveringRef.current &&
        snapshotRef.current.pendingCount === 0 &&
        !isTextEntryActive()
      ) {
        collapseIsland();
      }
    }, IDLE_COLLAPSE_DELAY_MS);
  }

  function handlePointerEnter() {
    hoveringRef.current = true;
    cursorOverIslandRef.current = true;
    clearIdleTimer();
    if (!suppressHoverExpandRef.current) {
      expandIsland();
    }
  }

  function handlePointerLeave() {
    hoveringRef.current = false;
    cursorOverIslandRef.current = false;
    if (phaseRef.current !== "closing") {
      suppressHoverExpandRef.current = false;
    }
    if (
      phaseRef.current === "compact" &&
      shouldRestInMicro(usesMicroIslandRef.current)
    ) {
      scheduleShrinkToMicro();
    } else {
      scheduleIdleCollapse();
    }
  }

  function handleIslandClick(event: MouseEvent<HTMLElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
    if (
      (event.target as HTMLElement).closest(
        "input, textarea, [contenteditable='true']",
      )
    ) {
      return;
    }
    suppressHoverExpandRef.current = false;
    focusedRef.current = true;
    event.currentTarget.focus({ preventScroll: true });
    expandIsland();
  }

  function handleIslandFocus() {
    focusedRef.current = true;
    expandIsland();
  }

  function handleIslandBlur(event: FocusEvent<HTMLElement>) {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      focusedRef.current = false;
      scheduleIdleCollapse();
    }
  }

  function handleControlMouseDown(event: MouseEvent<HTMLElement>) {
    event.preventDefault();
    event.stopPropagation();
  }

  async function startWindowDrag(event: MouseEvent<HTMLElement>) {
    if (!("__TAURI_INTERNALS__" in window) || event.button !== 0) return;

    const target = event.target as HTMLElement;
    if (target.closest("[data-no-drag]")) return;

    await getCurrentWindow().startDragging().catch(() => undefined);
    focusedRef.current = false;
    if (document.activeElement instanceof HTMLElement) {
      document.activeElement.blur();
    }
    scheduleIdleCollapse();
  }

  return {
    presentationReady,
    phase,
    phaseRef,
    supportsMicroIsland,
    foldedIslandSize,
    notchMetrics,
    notchMetricsHydrated,
    usesMicroIslandRef,
    frozenCollapseWidthRef,
    suppressHoverExpandRef,
    dismissedPlanRequestIdsRef,
    lastNativePresentationKeyRef,
    artworkBackdropOrigin,
    artworkBackdropRevealed,
    artworkBackdropExitFade,
    artworkBackdropOriginRef,
    compactMediaThumbRef,
    expandIsland,
    setFileDragActive,
    collapseIsland,
    scheduleIdleCollapse,
    clearIdleTimer,
    promoteToCompact,
    shrinkToMicro,
    handleChangeFoldedIslandSize,
    refreshNotchMetrics,
    ensureExpandedSettingsPresentation,
    syncNativeIslandPresentation,
    handlePointerEnter,
    handlePointerLeave,
    handleIslandClick,
    handleIslandFocus,
    handleIslandBlur,
    handleControlMouseDown,
    startWindowDrag,
  };
}
