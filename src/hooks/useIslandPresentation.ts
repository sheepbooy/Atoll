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
  onIslandOpenRequested,
  onIslandPresentationSettled,
  type IslandSnapshot,
  type NotchMetrics,
} from "../tauri";
import {
  beginCollapse,
  beginExpand,
  finishExpand,
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
import { snapshotHasPlanPending } from "../planMode";
import { isTextEntryActive } from "../imeHelpers";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import type { ArtworkBackdropOrigin, FoldedIslandSize, PanelView } from "../appTypes";

interface UseIslandPresentationOptions {
  snapshotRef: { current: IslandSnapshot };
  setSnapshot: (
    updater: IslandSnapshot | ((prev: IslandSnapshot) => IslandSnapshot),
  ) => void;
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
}

/**
 * Island presentation FSM: phase (micro/compact/dormant/opening/closing/
 * expanded), native window animation handshakes, hover/summon handling,
 * folded-size state, notch metrics, and the artwork-backdrop presentation.
 */
export function useIslandPresentation({
  snapshotRef,
  setSnapshot,
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
}: UseIslandPresentationOptions) {
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
  const initialNativePresentationSyncedRef = useRef(false);
  const hoveringRef = useRef(false);
  const cursorOverIslandRef = useRef(false);
  // A hotkey summon holds the island open: until it collapses again, later
  // snapshot refreshes / blur events must not schedule the idle collapse.
  const summonHoldRef = useRef(false);
  const shrinkInFlightRef = useRef(false);
  const focusedRef = useRef(false);
  const suppressHoverExpandRef = useRef(false);
  const transitionTimerRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  const frozenCollapseWidthRef = useRef<number | null>(null);
  const frozenCollapseLeftWidthRef = useRef<number | null>(null);
  const suppressPostCollapseSyncRef = useRef(false);
  const holdCompactAfterSubviewOpenRef = useRef(false);
  // Closures captured at transition start, run when the native window emits
  // `island-presentation-settled` (or the 2s fallback fires). Captured by value
  // so the listener sees the metrics that were current when the transition began.
  const pendingExpandRef = useRef<(() => Promise<void>) | null>(null);
  const pendingCollapseRef = useRef<(() => Promise<void>) | null>(null);
  const expandCollapseAnchorRef = useRef<{
    width: number;
    leftWidth: number;
  } | null>(null);
  const lastNativePresentationKeyRef = useRef<string | null>(null);

  const [notchMetrics, setNotchMetrics] = useState<NotchMetrics>(EMPTY_NOTCH_METRICS);

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
      syncNativeIslandPresentation(
        "expanded",
        undefined,
        false,
        undefined,
        false,
        true,
      ).catch(() => {
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
      if (cursorOverWindow) {
        clearIdleTimer();
        if (
          !suppressHoverExpandRef.current &&
          (phaseRef.current === "closing" ||
            (shrinkInFlightRef.current && phaseRef.current === "micro"))
        ) {
          expandIsland();
          return;
        }
      }
      hoveringRef.current = hovering;
      if (hovering) {
        if (!suppressHoverExpandRef.current) {
          expandIsland();
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
    onIslandPresentationSettled((mode) => {
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
    return () => {
      unsubscribeHover();
      unsubscribeOpen();
      unsubscribeSettled();
      clearTransitionWork();
      clearIdleTimer();
    };
  }, []);

  function setPresentationPhase(next: PresentationPhase) {
    phaseRef.current = next;
    setPhase(next);
    if (next === "expanded" || next === "compact") {
      setSnapshot(snapshotRef.current);
    }
  }

  function syncNativeIslandPresentation(
    mode: "micro" | "compact" | "expanded" | "dormant",
    compactWidth?: number,
    expandedIdle?: boolean,
    compactLeftWidth?: number,
    expandedPlan?: boolean,
    expandedSettings?: boolean,
  ) {
    const snap =
      !notchMetricsHydrated || !initialNativePresentationSyncedRef.current;
    return setIslandPresentation(
      mode,
      compactWidth,
      expandedIdle,
      compactLeftWidth,
      !snap,
      snap,
      expandedPlan,
      expandedSettings,
    ).finally(() => {
      if (notchMetricsHydrated) {
        initialNativePresentationSyncedRef.current = true;
      }
    });
  }

  function clearTransitionWork() {
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
    holdCompactAfterSubviewOpenRef.current = false;
    clearIdleTimer();

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
      await setIslandPresentation(
        "compact",
        compactWidth,
        undefined,
        compactLeftWidth,
      );
      if (
        !options?.skipExpand &&
        hoveringRef.current &&
        !suppressHoverExpandRef.current
      ) {
        expandIsland();
      }
    } catch {
      setPresentationPhase("micro");
    }
  }

  async function shrinkToMicro() {
    if (phaseRef.current !== "compact") return;
    if (holdCompactAfterSubviewOpenRef.current) return;
    if (
      !shouldRestInMicro(usesMicroIslandRef.current)
    ) {
      return;
    }

    clearIdleTimer();
    setPresentationPhase("micro");
    const microWidth = microPresentationWidthRef.current;
    lastNativePresentationKeyRef.current = compactPresentationKey(
      "micro",
      microWidth,
      0,
    );
    shrinkInFlightRef.current = true;
    try {
      await setIslandPresentation("micro", microWidth);
    } catch {
      setPresentationPhase("compact");
    } finally {
      shrinkInFlightRef.current = false;
    }
  }

  function scheduleShrinkToMicro() {
    clearIdleTimer();
    if (
      holdCompactAfterSubviewOpenRef.current ||
      hoveringRef.current ||
      cursorOverIslandRef.current ||
      snapshotRef.current.pendingCount > 0 ||
      isTextEntryActive()
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

  async function expandIsland() {
    clearIdleTimer();
    holdCompactAfterSubviewOpenRef.current = false;
    cancelPanelExit();

    const next = beginExpand(phaseRef.current);
    if (next === phaseRef.current) return;
    clearTransitionWork();
    if (artworkBackdropExitFadeRef.current) {
      artworkBackdropExitFadeRef.current = false;
      setArtworkBackdropExitFade(false);
    }
    if (artworkBackdropOriginRef.current === null) {
      captureArtworkBackdropOrigin();
    }
    expandCollapseAnchorRef.current = {
      width: collapsedWindowWidthRef.current,
      leftWidth: compactLeftPaneWidthRef.current,
    };

    const idleExpanded =
      snapshotRef.current.pendingCount === 0 &&
      snapshotRef.current.sessions.length === 0;
    const planExpanded = snapshotHasPlanPending(snapshotRef.current);
    const settingsExpanded =
      panelViewRef.current.kind === "settings" ||
      panelViewRef.current.kind === "clipboard" ||
      panelViewRef.current.kind === "history";
    lastNativePresentationKeyRef.current = expandedPresentationKey(
      idleExpanded,
      planExpanded && !settingsExpanded,
      settingsExpanded,
    );
    setPresentationPhase(next);
    const nativeTransition = setIslandPresentation(
      "expanded",
      collapsedWindowWidthRef.current,
      idleExpanded,
      compactLeftPaneWidthRef.current,
      true,
      false,
      planExpanded && !settingsExpanded,
      settingsExpanded,
    );
    pendingExpandRef.current = async () => {
      if (phaseRef.current !== "opening") return;
      try {
        await nativeTransition;
        if (phaseRef.current === "opening") {
          if (notchMetricsHydrated) {
            initialNativePresentationSyncedRef.current = true;
          }
          setPresentationPhase(finishExpand("opening"));
        }
      } catch {
        setPresentationPhase(usesMicroIslandRef.current ? "micro" : "compact");
      }
    };
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      // 2s fallback: only fires if the native `island-presentation-settled`
      // event never arrives (e.g. the `animate: false, snap: false`
      // fire-and-forget presentation path).
      await runPendingExpand();
    }, PRESENTATION_SETTLE_FALLBACK_MS);
  }

  function runPendingExpand() {
    const finalize = pendingExpandRef.current;
    pendingExpandRef.current = null;
    if (!finalize) return Promise.resolve();
    return finalize();
  }

  function collapsePresentationMode(): "micro" | "compact" | "dormant" {
    const sessionCount = snapshotRef.current.sessions.length;
    const pendingCount = snapshotRef.current.pendingCount;
    if (shouldRestInMicro(usesMicroIslandRef.current)) {
      return "micro";
    }
    if (supportsMicroIslandRef.current) return "compact";
    if (sessionCount === 0 && pendingCount === 0) return "dormant";
    return "compact";
  }

  function collapsedRestPhase(): PresentationPhase {
    return collapsePresentationMode() === "micro" ? "micro" : "compact";
  }

  function resolveCollapseMetrics(): { width: number; leftWidth: number } {
    const anchor = expandCollapseAnchorRef.current;
    return {
      width: Math.max(
        collapsedWindowWidthRef.current,
        anchor?.width ?? 0,
      ),
      leftWidth: Math.max(
        compactLeftPaneWidthRef.current,
        anchor?.leftWidth ?? 0,
      ),
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

  function collapseIslandNow(releaseFocus = false) {
    summonHoldRef.current = false;
    const next = beginCollapse(phaseRef.current);
    if (next === phaseRef.current) {
      if (releaseFocus) {
        focusedRef.current = false;
        if (document.activeElement instanceof HTMLElement) {
          document.activeElement.blur();
        }
      }
      return;
    }

    if (releaseFocus) {
      suppressHoverExpandRef.current = true;
      focusedRef.current = false;
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    }
    const leavingPanel = panelViewRef.current.kind;
    clearTransitionWork();
    const collapseMetrics = resolveCollapseMetrics();
    frozenCollapseWidthRef.current = collapseMetrics.width;
    frozenCollapseLeftWidthRef.current = collapseMetrics.leftWidth;
    setPresentationPhase(next);
    ++navigationSeqRef.current;
    setNavDirection(null);
    setPanelView({ kind: "home" });

    const compactWidth = collapseCompactWidth();
    const compactLeftWidth = collapseCompactLeftWidth();
    const naturalCollapseMode = collapsePresentationMode();
    const wasSessionSubview =
      leavingPanel === "session" || leavingPanel === "subagent" || leavingPanel === "subagentList";
    const collapseMode =
      wasSessionSubview && naturalCollapseMode === "dormant"
        ? "compact"
        : naturalCollapseMode;
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

    const nativeTransition =
      collapseMode === "micro"
        ? setIslandPresentation("micro", microPresentationWidthRef.current)
        : collapseMode === "dormant"
          ? setIslandPresentation("dormant")
          : setIslandPresentation(
              "compact",
              compactWidth,
              undefined,
              compactLeftWidth,
            );
    pendingCollapseRef.current = async () => {
      if (phaseRef.current !== "closing") return;

      try {
        await nativeTransition;
        if (phaseRef.current === "closing") {
          if (collapseMode === "micro") {
            await setIslandPresentation(
              "micro",
              microPresentationWidthRef.current,
              undefined,
              undefined,
              false,
              true,
            );
          } else if (collapseMode === "dormant") {
            await setIslandPresentation(
              "dormant",
              undefined,
              undefined,
              undefined,
              false,
              true,
            );
          } else {
            await setIslandPresentation(
              "compact",
              compactWidth,
              undefined,
              compactLeftWidth,
              false,
              true,
            );
          }
          lastNativePresentationKeyRef.current = compactPresentationKey(
            collapseMode,
            collapsePresentationWidth,
            compactLeftWidth,
          );
          expandCollapseAnchorRef.current = {
            width: compactWidth,
            leftWidth: compactLeftWidth,
          };
          if (wasSessionSubview && naturalCollapseMode === "dormant") {
            suppressPostCollapseSyncRef.current = true;
          }
          setPresentationPhase(collapseMode === "micro" ? "micro" : "compact");
        }
      } catch {
        releaseFrozenCollapseMetrics();
        setPresentationPhase("expanded");
      } finally {
        releaseFrozenCollapseMetrics();
        pendingCollapseRef.current = null;
        suppressHoverExpandRef.current = false;
      }
    };
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      // 2s fallback: only fires if the native `island-presentation-settled`
      // event never arrives (e.g. the `animate: false, snap: false`
      // fire-and-forget presentation path).
      await runPendingCollapse();
    }, PRESENTATION_SETTLE_FALLBACK_MS);
  }

  function runPendingCollapse() {
    const finalize = pendingCollapseRef.current;
    pendingCollapseRef.current = null;
    if (!finalize) return Promise.resolve();
    return finalize();
  }

  function scheduleIdleCollapse() {
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
    phase,
    phaseRef,
    supportsMicroIsland,
    foldedIslandSize,
    notchMetrics,
    notchMetricsHydrated,
    usesMicroIslandRef,
    suppressPostCollapseSyncRef,
    holdCompactAfterSubviewOpenRef,
    frozenCollapseWidthRef,
    suppressHoverExpandRef,
    lastNativePresentationKeyRef,
    artworkBackdropOrigin,
    artworkBackdropRevealed,
    artworkBackdropExitFade,
    artworkBackdropOriginRef,
    compactMediaThumbRef,
    expandIsland,
    collapseIsland,
    scheduleIdleCollapse,
    clearIdleTimer,
    promoteToCompact,
    shrinkToMicro,
    handleChangeFoldedIslandSize,
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
