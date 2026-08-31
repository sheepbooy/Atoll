import { useEffect, useRef, useState } from "react";
import { getSessionRequests, type PermissionRequest, type SessionSummary } from "../tauri";
import { PANEL_EXIT_MS } from "../islandPresentation";
import type { PanelView, SettingsPage } from "../appTypes";

interface UsePanelNavigationOptions {
  sessions: SessionSummary[];
  setSessionRequests: (requests: PermissionRequest[]) => void;
  expandIsland: () => void;
  ensureExpandedSettingsPresentation: () => void;
  refreshClipboardHistory: () => void;
  closeMenu: () => void;
}

/**
 * Panel navigation: which subview is open, its enter/exit animation state,
 * and the back-target stack for the hooks/tokens/usage pages.
 */
export function usePanelNavigation({
  sessions,
  setSessionRequests,
  expandIsland,
  ensureExpandedSettingsPresentation,
  refreshClipboardHistory,
  closeMenu,
}: UsePanelNavigationOptions) {
  const [panelView, setPanelView] = useState<PanelView>({ kind: "home" });
  const panelViewRef = useRef<PanelView>({ kind: "home" });
  panelViewRef.current = panelView;
  const navigationSeqRef = useRef(0);
  const [hooksBackTarget, setHooksBackTarget] = useState<"home" | "settings-main">("home");
  const [tokensBackTarget, setTokensBackTarget] = useState<"home" | "settings-main">("home");
  const [usageBackTarget, setUsageBackTarget] = useState<"home" | "settings-main">("home");
  const [navDirection, setNavDirection] = useState<"forward" | "back" | null>(null);
  const [panelAnimKey, setPanelAnimKey] = useState(0);
  const [panelExiting, setPanelExiting] = useState(false);
  const panelExitTimerRef = useRef<number | null>(null);
  const panelExitingRef = useRef(false);
  panelExitingRef.current = panelExiting;

  useEffect(() => {
    if (panelView.kind === "session") {
      if (!sessions.some((session) => session.sessionId === panelView.sessionId)) {
        ++navigationSeqRef.current;
        setPanelView({ kind: "home" });
        setSessionRequests([]);
      }
      return;
    }
    if (panelView.kind === "subagent") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      const subagent = session?.activeSubagents?.find(
        (sub) => sub.agentId === panelView.agentId,
      );
      if (!subagent) {
        ++navigationSeqRef.current;
        if (session) {
          setPanelView({ kind: "session", sessionId: panelView.sessionId });
        } else {
          setPanelView({ kind: "home" });
        }
      }
      return;
    }
    if (panelView.kind === "subagentList") {
      if (!sessions.some((s) => s.sessionId === panelView.sessionId)) {
        ++navigationSeqRef.current;
        setPanelView({ kind: "home" });
      }
    }
  }, [panelView, sessions]);

  async function navigateToSession(sessionId: string) {
    const seq = ++navigationSeqRef.current;
    setSessionRequests([]);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    expandIsland();
    setPanelView({ kind: "session", sessionId });
    try {
      const requests = await getSessionRequests(sessionId);
      if (navigationSeqRef.current === seq) {
        setSessionRequests(requests);
      }
    } catch {
      // Tauri invoke failed; leave requests empty rather than hanging.
    }
  }

  function navigateToSubagent(sessionId: string, agentId: string) {
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "subagent", sessionId, agentId });
  }

  function navigateToSubagentList(sessionId: string) {
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "subagentList", sessionId });
  }

  function navigateBack() {
    ++navigationSeqRef.current;
    setNavDirection("back");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "home" });
  }

  function openTokensPage(backTarget: "home" | "settings-main") {
    closeMenu();
    setTokensBackTarget(backTarget);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page: "tokens" });
  }

  function handleOpenTokensFromCounter() {
    if (panelView.kind === "settings" && panelView.page === "tokens") return;
    // Kick off the settings-size resize before mounting the tokens page so the
    // native animation starts with a light DOM. TokenHeatmapView further defers
    // its dense grid/charts until COLLAPSE_ANIMATION_MS elapses.
    if (panelView.kind !== "settings") {
      ensureExpandedSettingsPresentation();
    }
    openTokensPage(panelView.kind === "settings" ? "settings-main" : "home");
  }

  function handleOpenTokensFromSettings() {
    openTokensPage("settings-main");
  }

  function navigateBackFromTokens() {
    if (tokensBackTarget === "settings-main") {
      setNavDirection("back");
      setPanelAnimKey((key) => key + 1);
      setPanelView({ kind: "settings", page: "main" });
    } else {
      navigateBack();
    }
  }

  function openUsagePage(backTarget: "home" | "settings-main") {
    closeMenu();
    setUsageBackTarget(backTarget);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page: "usage" });
  }

  function handleOpenUsageFromSettings() {
    openUsagePage("settings-main");
  }

  function navigateBackFromUsage() {
    if (usageBackTarget === "settings-main") {
      setNavDirection("back");
      setPanelAnimKey((key) => key + 1);
      setPanelView({ kind: "settings", page: "main" });
    } else {
      navigateBack();
    }
  }

  function openClipboardPage() {
    closeMenu();
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "clipboard" });
    refreshClipboardHistory();
  }

  function handleOpenClipboard() {
    openClipboardPage();
  }

  function openHistoryPage() {
    closeMenu();
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "history" });
  }

  function handleOpenHistory() {
    openHistoryPage();
  }

  function handleOpenSettings() {
    closeMenu();
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    if (panelView.kind !== "settings") {
      ensureExpandedSettingsPresentation();
    }
    setPanelView({ kind: "settings", page: "main" });
  }

  function openSettingsSubpage(
    page: Exclude<SettingsPage, "main" | "hooks" | "tokens" | "usage">,
  ) {
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page });
  }

  function navigateBackToSettingsMain() {
    setNavDirection("back");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page: "main" });
  }

  function openHooksPage(backTarget: "home" | "settings-main") {
    closeMenu();
    setHooksBackTarget(backTarget);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page: "hooks" });
  }

  function handleOpenHooks() {
    openHooksPage("home");
  }

  function handleOpenHooksFromSettings() {
    openHooksPage("settings-main");
  }

  function navigateBackFromHooks() {
    if (hooksBackTarget === "settings-main") {
      setNavDirection("back");
      setPanelAnimKey((key) => key + 1);
      setPanelView({ kind: "settings", page: "main" });
    } else {
      navigateBack();
    }
  }

  // Fade panel content out before the native window shrink starts. Called by
  // the island FSM's collapse path; `onExited` continues the collapse.
  function tryBeginPanelExit(onExited: () => void): boolean {
    if (panelExitingRef.current) {
      return false;
    }
    setPanelExiting(true);
    panelExitTimerRef.current = window.setTimeout(() => {
      panelExitTimerRef.current = null;
      setPanelExiting(false);
      onExited();
    }, PANEL_EXIT_MS);
    return true;
  }

  function cancelPanelExit() {
    if (panelExitTimerRef.current !== null) {
      window.clearTimeout(panelExitTimerRef.current);
      panelExitTimerRef.current = null;
    }
    if (panelExitingRef.current) {
      panelExitingRef.current = false;
      setPanelExiting(false);
    }
  }

  function clearPanelExitTimer() {
    if (panelExitTimerRef.current !== null) {
      window.clearTimeout(panelExitTimerRef.current);
      panelExitTimerRef.current = null;
    }
  }

  return {
    panelView,
    panelViewRef,
    setPanelView,
    setNavDirection,
    navigationSeqRef,
    hooksBackTarget,
    tokensBackTarget,
    usageBackTarget,
    navDirection,
    panelAnimKey,
    panelExiting,
    navigateToSession,
    navigateToSubagent,
    navigateToSubagentList,
    navigateBack,
    handleOpenTokensFromCounter,
    handleOpenTokensFromSettings,
    navigateBackFromTokens,
    handleOpenUsageFromSettings,
    navigateBackFromUsage,
    handleOpenClipboard,
    handleOpenHistory,
    handleOpenSettings,
    openSettingsSubpage,
    navigateBackToSettingsMain,
    openHooksPage,
    handleOpenHooks,
    handleOpenHooksFromSettings,
    navigateBackFromHooks,
    tryBeginPanelExit,
    cancelPanelExit,
    clearPanelExitTimer,
  };
}
