import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  CSSProperties,
} from "react";
import {
  useTranslation,
} from "react-i18next";
import {
  getSnapshot,
  sendMediaCommand,
  getClipboardHistory,
  copyStagedPathsToClipboard,
  setIslandPresentation,
  setPreferredMonitor,
  openAgentApp,
  stageClipboardEntries,
  type IslandSnapshot,
  type PermissionRequest,
} from "./tauri";
import {
  analyzeHookHealth,
  deriveHeaderLogoDisplay,
  hookAttentionTitle,
  type HeaderLogoDisplay,
} from "./hookHealth";
import {
  seedConfiguredFromHookHealth,
} from "./hookAgentsConfigured";
import {
  NowPlayingCard,
} from "./NowPlayingCard";
import {
  deriveAppLogoState,
  deriveAtollActivity,
} from "./logoStates";
import { useAtollReaction } from "./useAtollReaction";
import { useFileStation } from "./hooks/useFileStation";
import { AtollLogo } from "./AtollLogo";
import { stashBellyLevel } from "./fileStationTiers";
import {
  computeMicroWindowWidth,
} from "./compactLayout";
import {
  getDemoMode,
  isFileStationDemoMode,
  isGifCaptureMode,
  shouldAutoExpandDemo,
} from "./demoSnapshot";
import {
  type AgentKind,
} from "./appTypes";
import {
  clampCompactIconLimit,
} from "./settingsStorage";
import {
  initialSnapshot,
} from "./snapshotDefaults";
import {
  collapsedBandHeight,
} from "./islandLayout";
import {
  isPlanModeCommand,
  snapshotHasPlanPending,
} from "./planMode";
import {
  UpdateNotice,
} from "./components/UpdateNotice";
import { useUpdater } from "./hooks/useUpdater";
import { useLyrics } from "./hooks/useLyrics";
import { useClipboardHistory } from "./hooks/useClipboardHistory";
import { useNowPlaying } from "./hooks/useNowPlaying";
import { useDisplayAndSettingsPrefs } from "./hooks/useDisplayAndSettingsPrefs";
import { useHookInstaller } from "./hooks/useHookInstaller";
import { useApprovals } from "./hooks/useApprovals";
import { usePanelNavigation } from "./hooks/usePanelNavigation";
import { useIslandPresentation } from "./hooks/useIslandPresentation";
import { useSnapshotStream } from "./hooks/useSnapshotStream";
import { useSessionActions } from "./hooks/useSessionActions";
import { usePricingData } from "./hooks/usePricingData";
import { useAppSettings } from "./hooks/useAppSettings";
import { useCompactLayout } from "./hooks/useCompactLayout";
import { useUsageSummary } from "./hooks/useUsageSummary";
import { useStashTakeover } from "./hooks/useStashTakeover";
import { useImeSync } from "./hooks/useImeSync";
import { useNativePresentationSync } from "./hooks/useNativePresentationSync";
import { deriveIslandChromeFlags } from "./islandChromeFlags";
import { IslandHeader } from "./components/IslandHeader";
import { ArtworkBackdrop } from "./components/ArtworkBackdrop";
import { IslandPanelRouter } from "./components/IslandPanelRouter";

export function App() {
  const { t, i18n: i18nInstance } = useTranslation();
  const { t: tSettings } = useTranslation("settings");
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(initialSnapshot);
  const snapshotRef = useRef(initialSnapshot);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const menuOpenRef = useRef(false);
  menuOpenRef.current = menuOpen;
  const {
    updateNotice,
    updateAvailable,
    updateVersion,
    updateDownloading,
    updateDownloadProgress,
    updateChecking,
    dismissUpdateNotice,
    handleCheckForUpdates,
    handleInstallUpdate,
  } = useUpdater({ closeMenu: () => setMenuOpen(false) });

  const {
    language,
    approvalNoticeMode,
    globalShortcutView,
    handleChangeLanguage,
    handleChangeApprovalNoticeMode,
    handleChangeGlobalShortcutConfig,
  } = useAppSettings();

  const [sessionRequests, setSessionRequests] = useState<PermissionRequest[]>([]);

  const { lyricsData, playbackPosition, lyricsEnabled, handleChangeLyricsEnabled } =
    useLyrics();
  const {
    clipboardHistory,
    clipboardEnabled,
    clipboardLimit,
    clipboardAutoStage,
    setClipboardHistory,
    handleChangeClipboardEnabled,
    handleChangeClipboardLimit,
    handleChangeClipboardAutoStage,
  } = useClipboardHistory();
  const {
    nowPlayingTrack,
    mediaCardEnabled,
    artworkBackdropEnabled,
    artworkIsDark,
    handleChangeMediaCardEnabled,
    handleChangeArtworkBackdropEnabled,
  } = useNowPlaying();
  const {
    maxCompactIcons,
    setMaxCompactIcons,
    retentionMinutes,
    setRetentionMinutes,
    subagentRetentionMinutes,
    setSubagentRetentionMinutes,
    maxSubagentDisplay,
    setMaxSubagentDisplay,
    idleIntervalMin,
    setIdleIntervalMin,
    idleDurationMin,
    setIdleDurationMin,
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
    preferredMonitorName,
    setPreferredMonitorNameState,
  } = useDisplayAndSettingsPrefs();

  const [hookHealthHydrated, setHookHealthHydrated] = useState(false);
  const sessions = snapshot.sessions;

  function refreshClipboardHistory() {
    getClipboardHistory()
      .then(setClipboardHistory)
      .catch(() => undefined);
  }

  const fsmRef = useRef<{
    expandIsland: () => void;
    ensureExpandedSettingsPresentation: () => void;
  } | null>(null);

  // Layout mirrors (assigned after the cascade memos below; the FSM reads
  // them from event handlers, so they only need to exist before that call).
  const collapsedModeRef = useRef<"micro" | "compact" | "dormant">("compact");
  const collapsedWindowWidthRef = useRef(0);
  const compactLeftPaneWidthRef = useRef(0);
  const microPresentationWidthRef = useRef(computeMicroWindowWidth(0, 0, 0));

  const {
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
    handleOpenFileStation,
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
  } = usePanelNavigation({
    sessions,
    setSessionRequests,
    expandIsland: () => fsmRef.current!.expandIsland(),
    ensureExpandedSettingsPresentation: () =>
      fsmRef.current!.ensureExpandedSettingsPresentation(),
    refreshClipboardHistory,
    closeMenu: () => setMenuOpen(false),
  });

  const {
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
    dismissedPlanRequestIdsRef,
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
  } = useIslandPresentation({
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
    closeMenu: () => setMenuOpen(false),
  });
  fsmRef.current = {
    expandIsland,
    ensureExpandedSettingsPresentation,
  };

  const { applySnapshot, invalidatePendingSnapshotLoads } = useSnapshotStream({
    snapshotRef,
    setSnapshot,
    phaseRef,
    expandIsland,
    scheduleIdleCollapse,
    frozenCollapseWidthRef,
    collapseIsland,
    openHooksPage,
    suppressHoverExpandRef,
    dismissedPlanRequestIdsRef,
    panelViewRef,
    collapsedModeRef,
    collapsedWindowWidthRef,
    compactLeftPaneWidthRef,
    setHookHealthHydrated,
  });

  const {
    handleQuit,
    handleArchiveAll,
    handleArchiveSession,
    handlePinSession,
    handleArchiveCompletedSubagents,
  } = useSessionActions({
    setMenuOpen,
    applySnapshot,
    panelViewRef,
    navigationSeqRef,
    setPanelView,
  });

  async function handleChangePreferredMonitor(name: string | null) {
    const previous = preferredMonitorName;
    setPreferredMonitorNameState(name);
    try {
      setPreferredMonitorNameState(await setPreferredMonitor(name));
    } catch (error) {
      console.error("[Atoll] set preferred monitor failed", error);
      setPreferredMonitorNameState(previous);
      return;
    }
    // The selector lives in the expanded settings panel: snap the island onto
    // the newly chosen display instead of waiting for the next transition.
    const settingsExpanded =
      panelViewRef.current.kind === "settings" ||
      panelViewRef.current.kind === "clipboard" ||
      panelViewRef.current.kind === "fileStation" ||
      panelViewRef.current.kind === "history";
    if (
      settingsExpanded &&
      (phaseRef.current === "expanded" || phaseRef.current === "opening")
    ) {
      const idleExpanded =
        snapshotRef.current.pendingCount === 0 &&
        snapshotRef.current.sessions.length === 0;
      const planExpanded = snapshotHasPlanPending(snapshotRef.current);
      await setIslandPresentation(
        "expanded",
        collapsedWindowWidthRef.current,
        idleExpanded,
        compactLeftPaneWidthRef.current,
        false,
        true,
        planExpanded && !settingsExpanded,
        settingsExpanded,
      ).catch(() => undefined);
      // The new display may have different notch metrics; the snap re-derived
      // them on the backend, so mirror the refresh in the webview layout.
      await refreshNotchMetrics();
    }
  }

  const {
    hookBusy,
    hookInstallError,
    setHookInstallError,
    configuredHookAgents,
    setConfiguredHookAgents,
    applyHookInstallSnapshot,
    handleInstallClaudeHooks,
    handleInstallCodexHooks,
    handleInstallZcodeHooks,
    handleInstallGeminiHooks,
    handleInstallOpencodeHooks,
    handleInstallCursorHooks,
    handleInstallAllHooks,
    handleUninstallClaudeHooks,
    handleUninstallCodexHooks,
    handleUninstallZcodeHooks,
    handleUninstallGeminiHooks,
    handleUninstallOpencodeHooks,
    handleUninstallCursorHooks,
    handleUninstallHooks,
    handleRemoveCompetingClaudeHooks,
  } = useHookInstaller({
    applySnapshot,
    snapshotRef,
    invalidatePendingSnapshotLoads,
    collapseIsland,
    markHookHealthHydrated: () => setHookHealthHydrated(true),
    closeMenu: () => setMenuOpen(false),
  });

  const {
    selectedAgent,
    setSelectedAgent,
    selectedAgentRef,
    pricingModels,
    setPricingModels,
    pricingRates,
    tabAgents,
    handleSelectAgent,
  } = usePricingData({
    snapshot,
    activeRequest: snapshot.activeRequest,
    panelView,
    navigationSeqRef,
    setPanelView,
  });

  const { busyDecision, justResolved, resolveActive, resolveRequest } = useApprovals({
    snapshot,
    snapshotRef,
    panelView,
    selectedAgentRef,
    menuOpenRef,
    navigationSeqRef,
    applySnapshot,
    collapseIsland,
    scheduleIdleCollapse,
    setSessionRequests,
  });

  const activeRequest = snapshot.activeRequest;

  const hookHealthAnalysis = useMemo(
    () =>
      analyzeHookHealth(snapshot.hookHealth, {
        configuredAgents: configuredHookAgents,
      }),
    [snapshot.hookHealth, configuredHookAgents],
  );
  const claudeHookStatus = snapshot.hookHealth?.claude ?? null;
  const codexHookStatus = snapshot.hookHealth?.codex ?? null;
  const cursorHookStatus = snapshot.hookHealth?.cursor ?? null;
  const zcodeHookStatus = snapshot.hookHealth?.zcode ?? null;
  const geminiHookStatus = snapshot.hookHealth?.gemini ?? null;
  const opencodeHookStatus = snapshot.hookHealth?.opencode ?? null;
  const hookAttention = hookAttentionTitle(
    hookHealthAnalysis,
    hookHealthHydrated,
  );
  const atollActivity = useMemo(
    () => {
      if (!hookHealthHydrated) return "idle";
      return deriveAtollActivity({
        online: snapshot.online,
        pendingCount: snapshot.pendingCount,
        sessionCount: sessions.length,
      });
    },
    [hookHealthHydrated, snapshot.online, snapshot.pendingCount, sessions.length],
  );
  const appLogoState = useMemo(
    () =>
      deriveAppLogoState({
        online: snapshot.online,
        pendingCount: snapshot.pendingCount,
        sessionCount: sessions.length,
      }),
    [snapshot.online, snapshot.pendingCount, sessions.length],
  );
  const { reaction: atollReaction, reactionKey: atollReactionKey } =
    useAtollReaction(appLogoState);
  const {
    stagedFiles,
    stagedCount,
    dragOverIsland,
    stashReaction,
    stashReactionKey,
    playStashReaction,
    celebrateStage,
    removeStaged,
    clearStaged,
    lastStageResult,
  } = useFileStation();

  // ── 剪贴板 ↔ 中转站联动 ──
  // 入站：把剪贴板条目存入中转站；有实际入站才播吃零食动画 + toast。
  const handleStageClipboardEntry = useCallback(
    (id: string) =>
      stageClipboardEntries([id])
        .then((result) => {
          if (!result || result.added === 0) {
            return false;
          }
          celebrateStage(result);
          return true;
        })
        .catch(() => false),
    [celebrateStage],
  );
  // 出站：中转站行/多选复制路径文本（进入剪贴板，开启历史时一并记录）。
  const handleCopyStagedPaths = useCallback((ids: string[]) => {
    copyStagedPathsToClipboard(ids).catch(() => undefined);
  }, []);
  // 文件中转站反应（吃/吐）优先于状态跃迁反应；二者共用 reactionKey 重放机制。
  const logoReaction = stashReaction ?? atollReaction;
  const logoReactionKey = stashReaction ? stashReactionKey : atollReactionKey;
  const logoStashLevel = stashBellyLevel(stagedCount);

  // ── Atoll 吃/吐接管时刻：logo 从左上角原位放大占满整岛，播完缩回原位 ──
  // 拖入文件 → 分档 eat 反应；从面板行拖出文件 → spit。takeover 期间盖住
  // header 与面板；退出时向 header logo 原位缩回（位移/缩放由测量写入
  // CSS 变量，退出动画时长 340ms 须与 styles.css atoll-takeover-vanish 同步）。
  const islandRef = useRef<HTMLElement | null>(null);
  const atollIndicatorRef = useRef<HTMLSpanElement | null>(null);
  const {
    takeover,
    takeoverExiting,
    takeoverElRef,
    stashToast,
  } = useStashTakeover({
    stashReaction,
    stashReactionKey,
    lastStageResult,
    islandRef,
    atollIndicatorRef,
    t,
  });
  const headerLogo = useMemo(
    () =>
      deriveHeaderLogoDisplay(hookHealthAnalysis, atollActivity, {
        hookHealthKnown: hookHealthHydrated,
      }),
    [hookHealthAnalysis, atollActivity, hookHealthHydrated],
  );
  const collapsedHeaderLogo = useMemo((): HeaderLogoDisplay => {
    if (phase !== "micro" || headerLogo.kind === "atoll") {
      return headerLogo;
    }
    return { kind: "atoll", activity: "dead" };
  }, [headerLogo, phase]);
  const {
    dailyTokens,
    dailyTokenTotal,
    activeSessionTokens,
    activeSessionTokenTotal,
    dailyCostTotal,
    activeSessionCostTotal,
    usageDisplaySummary,
    settingsTodayLabel,
  } = useUsageSummary({
    snapshot,
    pricingRates,
    foldedCounterDisplay,
    expandedCounterDisplay,
    settingsBadgeDisplay,
    heatmapDisplay,
  });
  const selectedAgentRequest = useMemo(() => {
    if (!selectedAgent) return activeRequest;
    const fromRecent = snapshot.recent.find(
      (request) =>
        request.status === "pending" && request.agent === selectedAgent,
    );
    if (fromRecent) return fromRecent;
    if (activeRequest?.agent === selectedAgent) return activeRequest;
    return null;
  }, [selectedAgent, snapshot.recent, activeRequest]);

  const isPlanExpanded = useMemo(() => {
    if (!selectedAgentRequest) return false;
    return isPlanModeCommand(selectedAgentRequest.command);
  }, [selectedAgentRequest]);

  const filteredSessions = useMemo(() => {
    if (!selectedAgent) return sessions;
    return sessions.filter((session) => session.agent === selectedAgent);
  }, [sessions, selectedAgent]);

  const pendingCountByAgent = useMemo(() => {
    const counts: Record<AgentKind, number> = {
      claude: 0,
      codex: 0,
      cursor: 0,
      zcode: 0,
      gemini: 0,
      opencode: 0,
      other: 0,
    };
    for (const session of sessions) {
      counts[session.agent] += session.pendingCount;
    }
    return counts;
  }, [sessions]);

  const {
    maxCompactIconLimit,
    collapsedWindowWidth,
    collapsedMode,
    compactHeaderLayout,
    compactLeftPaneWidth,
  } = useCompactLayout({
    notchMetrics,
    sessions,
    maxCompactIcons,
    activeSessionTokenTotal,
    pendingCount: snapshot.pendingCount,
    nowPlayingTrack,
    compactIndicator,
    lyricsEnabled,
    lyricsData,
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
  });

  useEffect(() => {
    if (!hookHealthHydrated) return;
    setConfiguredHookAgents(seedConfiguredFromHookHealth(snapshot.hookHealth));
  }, [hookHealthHydrated, snapshot.hookHealth]);

  useImeSync();

  useEffect(() => {
    const demoMode = getDemoMode();
    if (!demoMode || !shouldAutoExpandDemo(demoMode)) return;
    const timer = window.setTimeout(() => {
      expandIsland();
    }, 120);
    return () => window.clearTimeout(timer);
  }, []);

  // Browser demo: land directly on the file station panel (`?demo=fileStation`).
  useEffect(() => {
    if (!isFileStationDemoMode()) return;
    const timer = window.setTimeout(() => {
      handleOpenFileStation();
    }, 260);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!isGifCaptureMode()) return;
    const compactHeight = collapsedBandHeight(notchMetrics);
    document.documentElement.style.setProperty(
      "--gif-window-w",
      `${collapsedWindowWidth}px`,
    );
    document.documentElement.style.setProperty(
      "--gif-window-h",
      `${compactHeight}px`,
    );
    document.documentElement.dataset.gifCompactWidth = String(collapsedWindowWidth);
    document.documentElement.dataset.gifCompactHeight = String(compactHeight);
  }, [collapsedWindowWidth, notchMetrics]);

  useEffect(() => {
    if (!menuOpen) return;

    function closeOnPointerDown(event: PointerEvent) {
      if (!menuRef.current?.contains(event.target as Node)) {
        setMenuOpen(false);
      }
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        setMenuOpen(false);
      }
    }

    document.addEventListener("pointerdown", closeOnPointerDown);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnPointerDown);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [menuOpen]);

  useEffect(() => {
    setMaxCompactIcons((current) =>
      clampCompactIconLimit(current, maxCompactIconLimit),
    );
  }, [maxCompactIconLimit]);

  const hasIncompleteSubagents = useMemo(
    () =>
      sessions.some((session) =>
        session.activeSubagents?.some((sub) => !sub.completedAt),
      ),
    [sessions],
  );

  useEffect(() => {
    if (phase !== "expanded" || !hasIncompleteSubagents) {
      return;
    }
    const interval = window.setInterval(() => {
      getSnapshot()
        .then((nextSnapshot) => {
          applySnapshot(nextSnapshot, { mergeHookHealth: true });
        })
        .catch(() => undefined);
    }, 2000);
    return () => window.clearInterval(interval);
  }, [phase, hasIncompleteSubagents]);

  const hooksNeedSetup =
    hookHealthHydrated && hookHealthAnalysis.needsFirstTimeSetup;
  const hooksNeedAttention =
    hookHealthHydrated &&
    (hookHealthAnalysis.needsFirstTimeSetup || hookHealthAnalysis.needsReconnect);
  const hooksSetupSummary = hookHealthAnalysis.summary;

  const {
    showAgentTabs,
    showPanelAgentTabs,
    isOpening,
    isClosing,
    isPresentationTransition,
    isExpanded,
    isExpandedChrome,
    isMicro,
    isDormant,
    showCompactHeaderMetrics,
    showMicroTokenCounter,
    showCompactTokenCounter,
    showCompactMediaIndicator,
    showArtworkBackdrop,
    showLyricsMarquee,
    showCollapsedActivityStrip,
    showCompactNotchSpacer,
    compactLeftSessions,
    compactRightSessions,
    compactLeftOverflow,
    compactRightOverflow,
    isIdleExpanded,
    isSettingsExpanded,
    nativeExpandedPlan,
    nativeExpandedSettings,
    isSubview,
    panelGlow,
    menuBarLogoSize,
    subviewSession,
    subviewSubagent,
  } = deriveIslandChromeFlags({
    phase,
    panelView,
    collapsedMode,
    usesMicroIsland: usesMicroIslandRef.current,
    suppressPostCollapseSync: suppressPostCollapseSyncRef.current,
    holdCompactAfterSubviewOpen: holdCompactAfterSubviewOpenRef.current,
    sessions,
    pendingCount: snapshot.pendingCount,
    isPlanExpanded,
    notchMetrics,
    compactIndicator,
    nowPlayingTrack,
    artworkBackdropEnabled,
    lyricsEnabled,
    lyricsData,
    selectedAgentRequest,
    selectedAgent,
    compactHeaderLayout,
    tabAgents,
  });

  // Keep Rust-side compact metrics current while expanded so collapse targets
  // the latest width without a follow-up resize animation.

  useNativePresentationSync({
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
  });

  return (
    <main className="stage">
      <section
        ref={islandRef}
        className={`island is-${phase} ${isExpanded ? "is-expanded" : ""} ${isIdleExpanded ? "is-idle" : ""} ${isPlanExpanded ? "is-plan" : ""} ${isSettingsExpanded ? "is-settings" : ""} ${isMicro ? "is-micro" : ""} ${isDormant ? "is-dormant" : ""} ${snapshot.pendingCount > 0 ? "has-pending" : ""} ${isExpandedChrome && panelView.kind !== "home" ? "is-subview" : ""} ${panelView.kind === "session" || panelView.kind === "subagent" || panelView.kind === "subagentList" ? "is-session-subview" : ""}${panelExiting ? " is-panel-exiting" : ""}`}
        style={{ "--panel-glow": panelGlow } as CSSProperties}
        aria-label={t("app.name")}
        tabIndex={0}
        onClick={handleIslandClick}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onFocusCapture={handleIslandFocus}
        onBlurCapture={handleIslandBlur}
      >
        {showArtworkBackdrop && nowPlayingTrack?.artworkBase64 ? (
          <ArtworkBackdrop
            nowPlayingTrack={nowPlayingTrack}
            artworkBackdropOrigin={artworkBackdropOrigin}
            artworkBackdropRevealed={artworkBackdropRevealed}
            artworkBackdropExitFade={artworkBackdropExitFade}
            artworkIsDark={artworkIsDark}
          />
        ) : null}
        <IslandHeader
    t={t}
    phase={phase}
    panelView={panelView}
    isExpanded={isExpanded}
    isExpandedChrome={isExpandedChrome}
    isPresentationTransition={isPresentationTransition}
    isMicro={isMicro}
    isDormant={isDormant}
    isSubview={isSubview}
    showPanelAgentTabs={showPanelAgentTabs}
    showAgentTabs={showAgentTabs}
    showCollapsedActivityStrip={showCollapsedActivityStrip}
    showCompactHeaderMetrics={showCompactHeaderMetrics}
    showMicroTokenCounter={showMicroTokenCounter}
    showCompactTokenCounter={showCompactTokenCounter}
    showCompactMediaIndicator={showCompactMediaIndicator}
    showCompactNotchSpacer={showCompactNotchSpacer}
    showLyricsMarquee={showLyricsMarquee}
    startWindowDrag={startWindowDrag}
    atollIndicatorRef={atollIndicatorRef}
    menuRef={menuRef}
    compactMediaThumbRef={compactMediaThumbRef}
    appLogoState={appLogoState}
    hooksNeedAttention={hooksNeedAttention}
    hookAttention={hookAttention}
    updateAvailable={updateAvailable}
    updateVersion={updateVersion}
    handleOpenHooks={handleOpenHooks}
    collapsedHeaderLogo={collapsedHeaderLogo}
    menuBarLogoSize={menuBarLogoSize}
    idleIntervalMin={idleIntervalMin}
    idleDurationMin={idleDurationMin}
    logoReaction={logoReaction}
    logoReactionKey={logoReactionKey}
    logoStashLevel={logoStashLevel}
    dragOverIsland={dragOverIsland}
    compactLeftSessions={compactLeftSessions}
    compactRightSessions={compactRightSessions}
    compactLeftOverflow={compactLeftOverflow}
    compactRightOverflow={compactRightOverflow}
    activeRequest={activeRequest}
    justResolved={justResolved}
    subviewSession={subviewSession}
    subviewSubagent={subviewSubagent}
    navigateBack={navigateBack}
    navigateBackFromHooks={navigateBackFromHooks}
    navigateBackFromTokens={navigateBackFromTokens}
    navigateBackFromUsage={navigateBackFromUsage}
    navigateBackToSettingsMain={navigateBackToSettingsMain}
    hooksBackTarget={hooksBackTarget}
    tokensBackTarget={tokensBackTarget}
    usageBackTarget={usageBackTarget}
    collapseIsland={collapseIsland}
    openAgentApp={openAgentApp}
    notchMetrics={notchMetrics}
    tabAgents={tabAgents}
    selectedAgent={selectedAgent}
    pendingCountByAgent={pendingCountByAgent}
    handleSelectAgent={handleSelectAgent}
    lyricsData={lyricsData}
    nowPlayingTrack={nowPlayingTrack}
    playbackPosition={playbackPosition}
    compactHeaderLayout={compactHeaderLayout}
    activeSessionTokens={activeSessionTokens}
    activeSessionTokenTotal={activeSessionTokenTotal}
    activeSessionCostTotal={activeSessionCostTotal}
    dailyTokens={dailyTokens}
    dailyTokenTotal={dailyTokenTotal}
    dailyCostTotal={dailyCostTotal}
    foldedCounterDisplay={foldedCounterDisplay}
    expandedCounterDisplay={expandedCounterDisplay}
    maxCompactIcons={maxCompactIcons}
    handleControlMouseDown={handleControlMouseDown}
    handleOpenTokensFromCounter={handleOpenTokensFromCounter}
    stagedCount={stagedCount}
    handleOpenFileStation={handleOpenFileStation}
    handleOpenClipboard={handleOpenClipboard}
    handleOpenHistory={handleOpenHistory}
    menuOpen={menuOpen}
    setMenuOpen={setMenuOpen}
    handleArchiveAll={handleArchiveAll}
    handleOpenSettings={handleOpenSettings}
    updateDownloading={updateDownloading}
    updateDownloadProgress={updateDownloadProgress}
    updateChecking={updateChecking}
    handleInstallUpdate={handleInstallUpdate}
    handleCheckForUpdates={handleCheckForUpdates}
    handleQuit={handleQuit}
    online={snapshot.online}
    pendingCount={snapshot.pendingCount}
    sessionsCount={sessions.length}
        />

        {!isPresentationTransition ? (
          <div
            className="island-panel"
            data-nav={navDirection ?? undefined}
          >
            <div key={panelAnimKey} className="island-panel-content">
                          <IslandPanelRouter
              panelView={panelView}
              sessions={sessions}
              sessionRequests={sessionRequests}
              navigationSeqRef={navigationSeqRef}
              setPanelView={setPanelView}
              applySnapshot={applySnapshot}
              hookHealth={snapshot.hookHealth}
              hookBusy={hookBusy}
              hookInstallError={hookInstallError}
              handleInstallClaudeHooks={handleInstallClaudeHooks}
              handleInstallCodexHooks={handleInstallCodexHooks}
              handleInstallZcodeHooks={handleInstallZcodeHooks}
              handleInstallGeminiHooks={handleInstallGeminiHooks}
              handleInstallOpencodeHooks={handleInstallOpencodeHooks}
              handleInstallCursorHooks={handleInstallCursorHooks}
              handleInstallAllHooks={handleInstallAllHooks}
              handleUninstallClaudeHooks={handleUninstallClaudeHooks}
              handleUninstallCodexHooks={handleUninstallCodexHooks}
              handleUninstallZcodeHooks={handleUninstallZcodeHooks}
              handleUninstallGeminiHooks={handleUninstallGeminiHooks}
              handleUninstallOpencodeHooks={handleUninstallOpencodeHooks}
              handleUninstallCursorHooks={handleUninstallCursorHooks}
              handleUninstallHooks={handleUninstallHooks}
              handleRemoveCompetingClaudeHooks={handleRemoveCompetingClaudeHooks}
              selectedAgentRequest={selectedAgentRequest}
              busyDecision={busyDecision}
              filteredSessions={filteredSessions}
              resolveActive={resolveActive}
              collapseIsland={collapseIsland}
              justResolved={justResolved}
              isExpandedChrome={isExpandedChrome}
              navigateToSession={navigateToSession}
              navigateToSubagent={navigateToSubagent}
              navigateToSubagentList={navigateToSubagentList}
              handleArchiveSession={handleArchiveSession}
              handleArchiveCompletedSubagents={handleArchiveCompletedSubagents}
              handlePinSession={handlePinSession}
              maxSubagentDisplay={maxSubagentDisplay}
              setMaxSubagentDisplay={setMaxSubagentDisplay}
              clipboardHistory={clipboardHistory}
              clipboardEnabled={clipboardEnabled}
              setClipboardHistory={setClipboardHistory}
              stageClipboardEntry={handleStageClipboardEntry}
              stagedFiles={stagedFiles}
              removeStaged={removeStaged}
              clearStaged={clearStaged}
              playStashReaction={playStashReaction}
              copyStagedPaths={handleCopyStagedPaths}
              dailyTokens={dailyTokens}
              dailyTokensByModel={snapshot.dailyTokensByModel}
              heatmapDisplay={heatmapDisplay}
              pricingRates={pricingRates}
              pricingModels={pricingModels}
              setPricingModels={setPricingModels}
              foldedCounterDisplay={foldedCounterDisplay}
              expandedCounterDisplay={expandedCounterDisplay}
              settingsBadgeDisplay={settingsBadgeDisplay}
              setFoldedCounterDisplay={setFoldedCounterDisplay}
              setExpandedCounterDisplay={setExpandedCounterDisplay}
              setSettingsBadgeDisplay={setSettingsBadgeDisplay}
              setHeatmapDisplay={setHeatmapDisplay}
              maxCompactIcons={maxCompactIcons}
              maxCompactIconLimit={maxCompactIconLimit}
              setMaxCompactIcons={setMaxCompactIcons}
              supportsMicroIsland={supportsMicroIsland}
              foldedIslandSize={foldedIslandSize}
              handleChangeFoldedIslandSize={handleChangeFoldedIslandSize}
              compactIndicator={compactIndicator}
              setCompactIndicatorState={setCompactIndicatorState}
              preferredMonitorName={preferredMonitorName}
              handleChangePreferredMonitor={handleChangePreferredMonitor}
              mediaCardEnabled={mediaCardEnabled}
              handleChangeMediaCardEnabled={handleChangeMediaCardEnabled}
              artworkBackdropEnabled={artworkBackdropEnabled}
              handleChangeArtworkBackdropEnabled={handleChangeArtworkBackdropEnabled}
              lyricsEnabled={lyricsEnabled}
              handleChangeLyricsEnabled={handleChangeLyricsEnabled}
              clipboardLimit={clipboardLimit}
              handleChangeClipboardEnabled={handleChangeClipboardEnabled}
              handleChangeClipboardLimit={handleChangeClipboardLimit}
              clipboardAutoStage={clipboardAutoStage}
              handleChangeClipboardAutoStage={handleChangeClipboardAutoStage}
              retentionMinutes={retentionMinutes}
              setRetentionMinutes={setRetentionMinutes}
              subagentRetentionMinutes={subagentRetentionMinutes}
              setSubagentRetentionMinutes={setSubagentRetentionMinutes}
              idleIntervalMin={idleIntervalMin}
              setIdleIntervalMin={setIdleIntervalMin}
              idleDurationMin={idleDurationMin}
              setIdleDurationMin={setIdleDurationMin}
              approvalNoticeMode={approvalNoticeMode}
              handleChangeApprovalNoticeMode={handleChangeApprovalNoticeMode}
              globalShortcutView={globalShortcutView}
              handleChangeGlobalShortcutConfig={handleChangeGlobalShortcutConfig}
              language={language}
              handleChangeLanguage={handleChangeLanguage}
              handleOpenHooksFromSettings={handleOpenHooksFromSettings}
              handleOpenTokensFromSettings={handleOpenTokensFromSettings}
              handleOpenUsageFromSettings={handleOpenUsageFromSettings}
              openSettingsSubpage={openSettingsSubpage}
              settingsTodayLabel={settingsTodayLabel}
              usageDisplaySummary={usageDisplaySummary}
              hooksSetupSummary={hooksSetupSummary}
              hooksNeedAttention={hooksNeedAttention}
              hooksNeedSetup={hooksNeedSetup}
              hookHealthAnalysis={hookHealthAnalysis}
              handleOpenHooks={handleOpenHooks}
              launchAtLogin={launchAtLogin}
              launchAtLoginBusy={launchAtLoginBusy}
              handleChangeLaunchAtLogin={handleChangeLaunchAtLogin}
            />
            </div>
            {isExpandedChrome && mediaCardEnabled && nowPlayingTrack ? (
              <div className="island-panel-footer">
                <NowPlayingCard
                  track={nowPlayingTrack}
                  livePosition={playbackPosition}
                  onCommand={(cmd) => {
                    sendMediaCommand(cmd).catch(() => undefined);
                  }}
                />
              </div>
            ) : null}
          </div>
        ) : null}
        {updateNotice ? (
          <UpdateNotice version={updateNotice} onDismiss={dismissUpdateNotice} />
        ) : null}
        {takeover ? (
          <div
            ref={takeoverElRef}
            className={`atoll-takeover${takeoverExiting ? " is-exiting" : ""}`}
            aria-hidden="true"
          >
            <div className="atoll-takeover-logo">
              <AtollLogo
                activity="idle"
                size={0}
                reaction={takeover.reaction}
                reactionKey={takeover.key}
                stashLevel={logoStashLevel}
                motionPaused={isPresentationTransition}
              />
            </div>
          </div>
        ) : null}
        {stashToast ? (
          <div key={stashToast.key} className="stash-toast" role="status" data-no-drag>
            <span className="stash-toast-text">{stashToast.text}</span>
          </div>
        ) : dragOverIsland ? (
          <div className="stash-feed-hint" role="status" data-no-drag>
            <span className="stash-toast-text">{t("fileStation.feedHint")}</span>
          </div>
        ) : null}
      </section>
    </main>
  );
}
