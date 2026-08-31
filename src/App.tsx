import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  CSSProperties,
  FocusEvent,
  MouseEvent,
} from "react";
import {
  Archive,
  ArrowUpCircle,
  Activity,
  ChevronUp,
  CircleDollarSign,
  ClipboardList,
  Clock,
  Download,
  Ellipsis,
  Bell,
  History,
  Layers,
  Music,
  Power,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  getCurrentWindow,
} from "@tauri-apps/api/window";
import {
  toPng,
} from "html-to-image";
import {
  getSnapshot,
  normalizeSnapshot,
  getSessionRequests,
  onIslandHoverChanged,
  onIslandOpenRequested,
  onIslandPresentationSettled,
  onCaptureCollapseRequested,
  onCaptureOpenHooksRequested,
  onCaptureScreenshotRequested,
  captureProvideScreenshot,
  onSnapshotChanged,
  getMediaCardEnabled,
  getArtworkBackdropEnabled,
  getApprovalNoticeMode,
  setApprovalNoticeMode,
  setNotificationLanguage,
  onNowPlayingChanged,
  sendMediaCommand,
  setMediaCardEnabled,
  setArtworkBackdropEnabled,
  getClipboardHistory,
  getClipboardHistoryEnabled,
  getClipboardHistoryLimit,
  onClipboardHistoryChanged,
  copyClipboardEntry,
  clearClipboardHistory,
  setClipboardHistoryEnabled,
  setClipboardHistoryLimit,
  toggleClipboardFavorite,
  archiveAllResolved,
  archiveSession,
  archiveSubagent,
  archiveCompletedSubagents,
  pinSession,
  deactivateAtoll,
  quitAtoll,
  resolvePermissionRequest,
  setIslandPresentation,
  setImeActive,
  setCompactLayout,
  usesMicroIsland,
  usesMicroIslandSync,
  setSessionAutoApprove,
  getNotchMetrics,
  installClaudeHooks,
  uninstallClaudeHooks,
  removeCompetingClaudeHooks,
  installCodexHooks,
  uninstallCodexHooks,
  installCursorHooks,
  uninstallCursorHooks,
  installZcodeHooks,
  uninstallZcodeHooks,
  installGeminiHooks,
  uninstallGeminiHooks,
  setSessionRetention,
  setSubagentRetention,
  openAgentApp,
  isAutostartEnabled,
  enableAutostart,
  disableAutostart,
  getLyricsEnabled,
  setLyricsEnabled,
  onLyricsChanged,
  onLyricsPosition,
  getCurrentLyrics,
  getGlobalShortcutConfig,
  setGlobalShortcutConfig,
  type IslandSnapshot,
  type PermissionRequest,
  type HookStatus,
  type HookHealthSnapshot,
  type NowPlayingTrack,
  type ApprovalNoticeMode,
  type ClipboardEntry,
  type NotchMetrics,
  type LyricPayload,
  type GlobalShortcutConfig,
  type GlobalShortcutView,
  type ShortcutAction,
} from "./tauri";
import {
  checkAppUpdate,
  getAppVersion,
  installAppUpdate,
  UPDATE_INITIAL_DELAY_MS,
  UPDATE_RECHECK_MS,
  type AppUpdateState,
} from "./appUpdate";
import {
  analyzeHookHealth,
  deriveHeaderLogoDisplay,
  hookAgentNote,
  hookAttentionTitle,
  mergeHookHealthPreferReady,
  type HeaderLogoDisplay,
} from "./hookHealth";
import i18n from "./i18n";
import {
  changeAppLanguage,
  readLanguage,
  type AppLanguage,
} from "./i18n";
import {
  markAllHookAgentsConfigured,
  markHookAgentConfigured,
  readConfiguredHookAgents,
  seedConfiguredFromHookHealth,
} from "./hookAgentsConfigured";
import {
  beginCollapse,
  beginExpand,
  COLLAPSE_ANIMATION_MS,
  finishExpand,
  IDLE_COLLAPSE_DELAY_MS,
  MICRO_SHRINK_DELAY_MS,
  PANEL_EXIT_MS,
  PRESENTATION_SETTLE_FALLBACK_MS,
  RESOLVE_FEEDBACK_MS,
  type PresentationPhase,
} from "./islandPresentation";
import {
  NowPlayingCard,
} from "./NowPlayingCard";
import {
  ClipboardHistoryView,
} from "./ClipboardHistoryView";
import {
  ApprovalHistoryView,
} from "./ApprovalHistoryView";
import {
  LyricsMarquee,
  lyricsMatchTrack,
} from "./LyricsMarquee";
import {
  ClipboardSettingsView,
  IslandSettingsView,
  MascotSettingsView,
  NotificationSettingsView,
  ShortcutSettingsView,
  MAX_CLIPBOARD_LIMIT,
  MediaSettingsView,
  MIN_CLIPBOARD_LIMIT,
  SessionSettingsView,
} from "./SettingsPages";
import {
  SettingsView,
} from "./SettingsView";
import {
  deriveAppLogoState,
  deriveAtollActivity,
} from "./logoStates";
import {
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  computeMaxCompactIconLimit,
  computeMicroWindowWidth,
} from "./compactLayout";
import {
  TokenCounter,
} from "./TokenCounter";
import {
  TokenHeatmapView,
} from "./TokenHeatmapView";
import {
  formatCompactTokenCount,
} from "./tokenCounterFormat";
import {
  formatCompactCost,
} from "./costFormat";
import {
  EXPANDED_COUNTER_DISPLAY_KEY,
  FOLDED_COUNTER_DISPLAY_KEY,
  HEATMAP_DISPLAY_KEY,
  readCompactIndicator,
  readDisplayMode,
  SETTINGS_BADGE_DISPLAY_KEY,
  writeCompactIndicator,
  writeDisplayMode,
  type CompactIndicatorMode,
  type UsageDisplayMode,
} from "./displayPrefs";
import {
  byModelCostUsd,
  getPricing,
  pricingRateMap,
  type ModelPricingEntry,
} from "./pricing";
import {
  UsageSettingsView,
} from "./UsageSettingsView";
import {
  getDemoMode,
  isGifCaptureMode,
  shouldAutoExpandDemo,
} from "./demoSnapshot";
import {
  manageAsyncUnlisten,
} from "./asyncUnlisten";
import {
  DEFAULT_GLOBAL_SHORTCUTS,
  withShortcutAction,
} from "./shortcuts";
import {
  type Decision,
  type AgentKind,
  type PanelView,
  type SettingsPage,
  type FoldedIslandSize,
  type ArtworkBackdropOrigin,
} from "./appTypes";
import {
  sampleArtworkIsDark,
} from "./artwork";
import {
  COMPACT_ICON_SETTING_KEY,
  FOLDED_ISLAND_SIZE_SETTING_KEY,
  RETENTION_SETTING_KEY,
  SUBAGENT_RETENTION_SETTING_KEY,
  MAX_SUBAGENT_DISPLAY_SETTING_KEY,
  IDLE_INTERVAL_SETTING_KEY,
  IDLE_DURATION_SETTING_KEY,
  markSettingsInitialized,
  clampCompactIconLimit,
  readCompactIconLimit,
  readFoldedIslandSize,
  clampRetentionMinutes,
  readRetentionMinutes,
  readSubagentRetentionMinutes,
  clampMaxSubagentDisplay,
  readMaxSubagentDisplay,
  clampIdleInterval,
  readIdleInterval,
  clampIdleDuration,
  readIdleDuration,
} from "./settingsStorage";
import {
  ZERO_TOKEN_USAGE,
  EMPTY_NOTCH_METRICS,
  initialSnapshot,
} from "./snapshotDefaults";
import {
  agentSortRank,
  PANEL_GLOW,
} from "./agents";
import {
  COMPACT_WINDOW_HEIGHT,
  applyWindowMetrics,
  compactPresentationKey,
  microPresentationWidth,
  shouldRestInMicro,
  shouldUseMicroIsland,
  resolveCollapsedMode,
  expandedPresentationKey,
} from "./islandLayout";
import {
  isPlanModeCommand,
  snapshotHasPlanPending,
  getPlanModeType,
} from "./planMode";
import {
  isImeTextTarget,
  isTextEntryActive,
} from "./imeHelpers";
import {
  IS_MACOS,
} from "./platform";
import {
  UpdateNotice,
} from "./components/UpdateNotice";
import { useUpdater } from "./hooks/useUpdater";
import { useLyrics } from "./hooks/useLyrics";
import { useClipboardHistory } from "./hooks/useClipboardHistory";
import { useNowPlaying } from "./hooks/useNowPlaying";
import { useDisplayAndSettingsPrefs } from "./hooks/useDisplayAndSettingsPrefs";
import { useHookInstaller } from "./hooks/useHookInstaller";
import {
  CompactSessionStack,
} from "./components/CompactSessionStack";
import {
  AgentTabBar,
} from "./components/AgentTabBar";
import {
  SessionListView,
} from "./components/SessionListView";
import {
  PlanQuestionCard,
} from "./components/PlanQuestionCard";
import {
  PlanApprovalCard,
} from "./components/PlanApprovalCard";
import {
  ApprovalCard,
} from "./components/ApprovalCard";
import {
  SessionSubviewNav,
} from "./components/SessionSubviewNav";
import {
  SettingsPageNav,
  SettingsSubviewNav,
} from "./components/SettingsNavs";
import {
  SessionChatView,
} from "./components/SessionChatView";
import {
  SubagentListView,
} from "./components/SubagentListView";
import {
  SubagentDetailView,
} from "./components/SubagentDetailView";
import {
  HooksView,
  formatHookInstallErrorMessage,
  type HookMenuAgent,
} from "./components/HooksView";
import {
  HeaderLogo,
} from "./components/HeaderLogo";
import {
  IdleView,
} from "./components/IdleView";

export function App() {
  const { t, i18n: i18nInstance } = useTranslation();
  const { t: tSettings } = useTranslation("settings");
  const [language, setLanguage] = useState<AppLanguage>(() => readLanguage());
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(initialSnapshot);
  const snapshotRef = useRef(initialSnapshot);
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
  const snapshotLoadSeqRef = useRef(0);
  const lastNativePresentationKeyRef = useRef<string | null>(null);
  const [busyDecision, setBusyDecision] = useState<Decision | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const busyRef = useRef<Decision | null>(null);
  busyRef.current = busyDecision;
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
    handleInstallCursorHooks,
    handleInstallAllHooks,
    handleUninstallClaudeHooks,
    handleUninstallCodexHooks,
    handleUninstallZcodeHooks,
    handleUninstallGeminiHooks,
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


  const [panelView, setPanelView] = useState<PanelView>({ kind: "home" });
  const panelViewRef = useRef<PanelView>({ kind: "home" });
  panelViewRef.current = panelView;
  const [sessionRequests, setSessionRequests] = useState<PermissionRequest[]>([]);
  const navigationSeqRef = useRef(0);
  const [hooksBackTarget, setHooksBackTarget] = useState<"home" | "settings-main">("home");
  const [tokensBackTarget, setTokensBackTarget] = useState<"home" | "settings-main">("home");
  const [usageBackTarget, setUsageBackTarget] = useState<"home" | "settings-main">("home");
  const [selectedAgent, setSelectedAgent] = useState<AgentKind | null>(null);
  const [notchMetrics, setNotchMetrics] = useState<NotchMetrics>(EMPTY_NOTCH_METRICS);
  const [pricingModels, setPricingModels] = useState<ModelPricingEntry[]>([]);
  const [approvalNoticeMode, setApprovalNoticeModeState] =
    useState<ApprovalNoticeMode>("interrupt");
  const [globalShortcutView, setGlobalShortcutView] = useState<GlobalShortcutView | null>(null);
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
  const { lyricsData, playbackPosition, lyricsEnabled, handleChangeLyricsEnabled } =
    useLyrics();
  const {
    clipboardHistory,
    clipboardEnabled,
    clipboardLimit,
    setClipboardHistory,
    handleChangeClipboardEnabled,
    handleChangeClipboardLimit,
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
    idleIntervalSec,
    setIdleIntervalSec,
    idleDurationSec,
    setIdleDurationSec,
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
  } = useDisplayAndSettingsPrefs();

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
  const [justResolved, setJustResolved] = useState(false);
  const [hookHealthHydrated, setHookHealthHydrated] = useState(false);
  const [navDirection, setNavDirection] = useState<"forward" | "back" | null>(null);
  const [panelAnimKey, setPanelAnimKey] = useState(0);
  const [panelExiting, setPanelExiting] = useState(false);
  const panelExitTimerRef = useRef<number | null>(null);
  const prevPendingRef = useRef(0);
  const lastSyncedRequestIdRef = useRef<string | null>(null);
  const selectedAgentRef = useRef<AgentKind | null>(null);
  selectedAgentRef.current = selectedAgent;

  const activeRequest = snapshot.activeRequest;
  const sessions = snapshot.sessions;
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
  const dailyTokens = snapshot.dailyTokens ?? ZERO_TOKEN_USAGE;
  const dailyTokenTotal = dailyTokens.inputTokens + dailyTokens.outputTokens;
  const activeSessionTokens = snapshot.activeSessionTokens ?? ZERO_TOKEN_USAGE;
  const activeSessionTokenTotal =
    activeSessionTokens.inputTokens + activeSessionTokens.outputTokens;
  const pricingRates = useMemo(() => pricingRateMap(pricingModels), [pricingModels]);
  const dailyCostTotal = useMemo(
    () => byModelCostUsd(snapshot.dailyTokensByModel, pricingRates),
    [snapshot.dailyTokensByModel, pricingRates],
  );
  const activeSessionCostTotal = useMemo(
    () => byModelCostUsd(snapshot.activeSessionTokensByModel, pricingRates),
    [snapshot.activeSessionTokensByModel, pricingRates],
  );
  const usageDisplaySummary = useMemo(() => {
    const modes = [
      foldedCounterDisplay,
      expandedCounterDisplay,
      settingsBadgeDisplay,
      heatmapDisplay,
    ];
    const costCount = modes.filter((mode) => mode === "cost").length;
    if (costCount === 0) return tSettings("usage.summaryTokens");
    if (costCount === modes.length) return tSettings("usage.summaryCost");
    return tSettings("usage.summaryMixedCost", { count: costCount });
  }, [
    foldedCounterDisplay,
    expandedCounterDisplay,
    settingsBadgeDisplay,
    heatmapDisplay,
    tSettings,
    i18nInstance.language,
  ]);
  const settingsTodayLabel = useMemo(
    () =>
      settingsBadgeDisplay === "cost"
        ? dailyCostTotal > 0
          ? tSettings("usage.todayCost", {
              amount: formatCompactCost(dailyCostTotal, 0, dailyCostTotal),
            })
          : tSettings("usage.noPricedUsage")
        : dailyTokenTotal > 0
          ? tSettings("usage.todayTokens", {
              amount: formatCompactTokenCount(
                dailyTokenTotal,
                dailyTokenTotal >= 1_000 ? 1 : 0,
                dailyTokenTotal,
              ),
            })
          : tSettings("usage.noUsageYet"),
    [
      settingsBadgeDisplay,
      dailyCostTotal,
      dailyTokenTotal,
      tSettings,
      i18nInstance.language,
    ],
  );
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
        snapshot.pendingCount,
        nowPlayingTrack?.artworkBase64 != null,
        compactIndicator === "media" || compactIndicator === "both",
        lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0,
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      snapshot.pendingCount,
      nowPlayingTrack?.artworkBase64,
      compactIndicator,
      lyricsEnabled,
      lyricsData,
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
    snapshot.pendingCount,
    phase,
    // When lyrics are active, stay in compact mode (not dormant) so the
    // header has room for the lyrics column. Dormant mode is too narrow.
    lyricsEnabled && lyricsData != null && lyricsData.lines.length > 0 && !notchMetrics.hasNotch,
  );
  const collapsedMode: "micro" | "compact" | "dormant" =
    (suppressPostCollapseSyncRef.current ||
      holdCompactAfterSubviewOpenRef.current) &&
    (rawCollapsedMode === "dormant" || rawCollapsedMode === "micro")
      ? "compact"
      : rawCollapsedMode;
  const tabAgents = useMemo(() => {
    const seen = new Set<AgentKind>();
    sessions.forEach((session) => seen.add(session.agent));
    if (activeRequest) {
      seen.add(activeRequest.agent);
    }
    return Array.from(seen).sort(
      (a, b) => agentSortRank[a] - agentSortRank[b],
    );
  }, [sessions, activeRequest]);

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
      other: 0,
    };
    for (const session of sessions) {
      counts[session.agent] += session.pendingCount;
    }
    return counts;
  }, [sessions]);

  const stableHeaderLayoutRef = useRef(
    computeCompactHeaderLayout(
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      snapshot.pendingCount,
    ),
  );
  const compactHeaderLayout = useMemo(() => {
    const computed = computeCompactHeaderLayout(
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      snapshot.pendingCount,
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
    snapshot.pendingCount,
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

  const collapsedModeRef = useRef<"micro" | "compact" | "dormant">("compact");
  collapsedModeRef.current = collapsedMode;
  const collapsedWindowWidthRef = useRef(collapsedWindowWidth);
  collapsedWindowWidthRef.current = collapsedWindowWidth;
  const compactLeftPaneWidthRef = useRef(compactLeftPaneWidth);
  compactLeftPaneWidthRef.current = compactLeftPaneWidth;
  const microPresentationWidthRef = useRef(
    computeMicroWindowWidth(0, 0, 0),
  );
  microPresentationWidthRef.current = microPresentationWidth(
    sessions.length,
    activeSessionTokenTotal,
    compactHeaderLayout.tokenCompactLevel,
  );

  useEffect(() => {
    if (!hookHealthHydrated) return;
    setConfiguredHookAgents(seedConfiguredFromHookHealth(snapshot.hookHealth));
  }, [hookHealthHydrated, snapshot.hookHealth]);

  useEffect(() => {
    let composing = false;

    const syncIme = (active: boolean) => {
      void setImeActive(active);
    };

    const onFocusIn = (event: Event) => {
      if (isImeTextTarget(event.target)) {
        syncIme(true);
      }
    };
    const onFocusOut = (event: globalThis.FocusEvent) => {
      if (!isImeTextTarget(event.target) || composing) {
        return;
      }
      if (isImeTextTarget(event.relatedTarget)) {
        return;
      }
      syncIme(false);
    };
    const onCompositionStart = () => {
      composing = true;
      syncIme(true);
    };
    const onCompositionEnd = () => {
      composing = false;
      syncIme(isTextEntryActive());
    };

    document.addEventListener("focusin", onFocusIn);
    document.addEventListener("focusout", onFocusOut);
    document.addEventListener("compositionstart", onCompositionStart);
    document.addEventListener("compositionend", onCompositionEnd);
    return () => {
      document.removeEventListener("focusin", onFocusIn);
      document.removeEventListener("focusout", onFocusOut);
      document.removeEventListener("compositionstart", onCompositionStart);
      document.removeEventListener("compositionend", onCompositionEnd);
      syncIme(false);
    };
  }, []);

  useEffect(() => {
    getPricing()
      .then((response) => setPricingModels(response.models))
      .catch(() => undefined);
  }, []);


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
    setSessionRetention(readRetentionMinutes()).catch(() => undefined);
    getApprovalNoticeMode()
      .then(setApprovalNoticeModeState)
      .catch(() => undefined);
    getGlobalShortcutConfig()
      .then(setGlobalShortcutView)
      .catch(() => undefined);
    setNotificationLanguage(readLanguage()).catch(() => undefined);
    const unsubscribe = manageAsyncUnlisten(
      onSnapshotChanged((nextSnapshot) => {
        applySnapshot(nextSnapshot, { mergeHookHealth: true });
        setHookHealthHydrated(true);
      }),
    );
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
      unsubscribeHover();
      unsubscribeOpen();
      unsubscribeSettled();
      unsubscribeCapture();
      unsubscribeCaptureHooks();
      unsubscribeScreenshot();
      clearTransitionWork();
      clearIdleTimer();
    };
  }, []);

  useEffect(() => {
    const demoMode = getDemoMode();
    if (!demoMode || !shouldAutoExpandDemo(demoMode)) return;
    const timer = window.setTimeout(() => {
      expandIsland();
    }, 120);
    return () => window.clearTimeout(timer);
  }, []);

  useEffect(() => {
    if (!isGifCaptureMode()) return;
    document.documentElement.style.setProperty(
      "--gif-window-w",
      `${collapsedWindowWidth}px`,
    );
    document.documentElement.style.setProperty(
      "--gif-window-h",
      `${COMPACT_WINDOW_HEIGHT}px`,
    );
    document.documentElement.dataset.gifCompactWidth = String(collapsedWindowWidth);
    document.documentElement.dataset.gifCompactHeight = String(COMPACT_WINDOW_HEIGHT);
  }, [collapsedWindowWidth]);

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
    function handleKeyDown(event: KeyboardEvent) {
      if (busyRef.current) return;
      if (menuOpenRef.current) return;
      if ((event.target as HTMLElement).tagName === "INPUT" || (event.target as HTMLElement).tagName === "TEXTAREA") return;

      const snapshot = snapshotRef.current;
      const agent = selectedAgentRef.current;
      const targetRequest = agent
        ? snapshot.recent.find(
            (request) => request.status === "pending" && request.agent === agent,
          ) ?? (snapshot.activeRequest?.agent === agent ? snapshot.activeRequest : null)
        : snapshot.activeRequest;
      if (!targetRequest) return;

      if (event.key === "Enter" && event.shiftKey) {
        event.preventDefault();
        resolveActive(targetRequest, "approved", true);
      } else if (event.key === "Enter") {
        event.preventDefault();
        resolveActive(targetRequest, "approved");
      } else if (event.key === "Backspace" || event.key === "Delete") {
        event.preventDefault();
        resolveActive(targetRequest, "denied");
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    const prev = prevPendingRef.current;
    prevPendingRef.current = snapshot.pendingCount;
    if (prev > 0 && snapshot.pendingCount === 0) {
      setJustResolved(true);
      const timer = window.setTimeout(() => setJustResolved(false), 1300);
      return () => window.clearTimeout(timer);
    }
  }, [snapshot.pendingCount]);

  useEffect(() => {
    if (tabAgents.length === 0) {
      setSelectedAgent(null);
      lastSyncedRequestIdRef.current = null;
      return;
    }

    // New pending request: force-select its agent and return to home so the
    // approval card is visible (not stuck on another agent's tab/subview).
    if (activeRequest?.id && activeRequest.id !== lastSyncedRequestIdRef.current) {
      lastSyncedRequestIdRef.current = activeRequest.id;
      setSelectedAgent(activeRequest.agent);
      if (panelView.kind !== "home") {
        ++navigationSeqRef.current;
        setPanelView({ kind: "home" });
      }
      return;
    }

    if (!activeRequest) {
      lastSyncedRequestIdRef.current = null;
    }

    if (selectedAgent && tabAgents.includes(selectedAgent)) {
      return;
    }
    setSelectedAgent(activeRequest?.agent ?? tabAgents[0]);
  }, [tabAgents, selectedAgent, activeRequest?.id, activeRequest?.agent, panelView.kind]);

  useEffect(() => {
    setMaxCompactIcons((current) =>
      clampCompactIconLimit(current, maxCompactIconLimit),
    );
  }, [maxCompactIconLimit]);


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

  async function resolveActive(
    request: PermissionRequest | null,
    decision: Decision,
    alwaysApprove = false,
    note = "",
  ) {
    if (!request) return;

    setBusyDecision(decision);
    try {
      const resolveWork = (async () => {
        if (alwaysApprove) {
          await setSessionAutoApprove(request.session, true);
        }
        return resolvePermissionRequest(request.id, decision, note);
      })();
      // Hold the snapshot update until the resolve feedback animation can play.
      const [, nextSnapshot] = await Promise.all([
        new Promise<void>((resolve) => {
          window.setTimeout(resolve, RESOLVE_FEEDBACK_MS);
        }),
        resolveWork,
      ]);
      applySnapshot(nextSnapshot);
      if (nextSnapshot.pendingCount === 0) {
        collapseIsland(true);
        deactivateAtoll(request.agent, request.session, request.cwd).catch(
          () => undefined,
        );
      }
    } finally {
      setBusyDecision(null);
    }
  }

  async function resolveRequest(id: string, decision: Decision, note = "") {
    const resolvedRequest =
      snapshotRef.current.activeRequest?.id === id
        ? snapshotRef.current.activeRequest
        : snapshotRef.current.recent.find((item) => item.id === id);
    setBusyDecision(decision);
    try {
      const seq = ++navigationSeqRef.current;
      const nextSnapshot = await resolvePermissionRequest(id, decision, note);
      applySnapshot(nextSnapshot);
      if (panelView.kind === "session" && navigationSeqRef.current === seq) {
        const requests = await getSessionRequests(panelView.sessionId).catch(() => []);
        if (navigationSeqRef.current === seq) {
          setSessionRequests(requests);
        }
      }
      if (nextSnapshot.pendingCount === 0) {
        scheduleIdleCollapse();
        deactivateAtoll(
          resolvedRequest?.agent,
          resolvedRequest?.session,
          resolvedRequest?.cwd,
        ).catch(() => undefined);
      }
    } finally {
      setBusyDecision(null);
    }
  }

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
    if (panelExitTimerRef.current !== null) {
      window.clearTimeout(panelExitTimerRef.current);
      panelExitTimerRef.current = null;
    }
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
    if (panelExitTimerRef.current !== null) {
      window.clearTimeout(panelExitTimerRef.current);
      panelExitTimerRef.current = null;
    }
    if (panelExiting) {
      setPanelExiting(false);
    }

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
    setMenuOpen(false);

    // Fade panel content out before the native window shrink starts.
    if (phaseRef.current === "expanded" && !panelExiting) {
      setPanelExiting(true);
      panelExitTimerRef.current = window.setTimeout(() => {
        panelExitTimerRef.current = null;
        setPanelExiting(false);
        collapseIslandNow(releaseFocus);
      }, PANEL_EXIT_MS);
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

  function invalidatePendingSnapshotLoads() {
    snapshotLoadSeqRef.current += 1;
  }

  async function handleChangeLanguage(nextLanguage: AppLanguage) {
    setLanguage(nextLanguage);
    setNotificationLanguage(nextLanguage).catch(() => undefined);
    await changeAppLanguage(nextLanguage);
  }

  const handleChangeApprovalNoticeMode = useCallback((mode: ApprovalNoticeMode) => {
    setApprovalNoticeModeState(mode);
    setApprovalNoticeMode(mode).catch(() => undefined);
  }, []);

  // Optimistically apply the edit, then adopt the backend view: it carries the
  // per-action registration errors (hotkey taken, invalid accelerator) that the
  // settings rows render.
  const handleChangeGlobalShortcutConfig = useCallback(
    (next: GlobalShortcutConfig) => {
      setGlobalShortcutView((prev) => (prev ? { ...prev, config: next, errors: {} } : prev));
      setGlobalShortcutConfig(next)
        .then(setGlobalShortcutView)
        .catch(() => undefined);
    },
    [],
  );

  const hookMenuAgents: HookMenuAgent[] = [
    {
      key: "claude",
      label: "Claude Code",
      status: claudeHookStatus,
      note: claudeHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: claudeHookStatus.settingsPath,
            note: hookAgentNote("claude"),
          })
        : i18n.t("register.claude", {
            ns: "hooks",
            note: hookAgentNote("claude"),
          }),
      onInstall: handleInstallClaudeHooks,
      onUninstall: handleUninstallClaudeHooks,
      onRemoveCompetingHooks: handleRemoveCompetingClaudeHooks,
    },
    {
      key: "codex",
      label: "Codex",
      status: codexHookStatus,
      note: codexHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: codexHookStatus.settingsPath,
            note: hookAgentNote("codex"),
          })
        : i18n.t("register.codex", {
            ns: "hooks",
            note: hookAgentNote("codex"),
          }),
      onInstall: handleInstallCodexHooks,
      onUninstall: handleUninstallCodexHooks,
    },
    {
      key: "cursor",
      label: "Cursor",
      status: cursorHookStatus,
      note: cursorHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: cursorHookStatus.settingsPath,
            note: hookAgentNote("cursor"),
          })
        : i18n.t("register.cursor", {
            ns: "hooks",
            note: hookAgentNote("cursor"),
          }),
      onInstall: handleInstallCursorHooks,
      onUninstall: handleUninstallCursorHooks,
    },
    {
      key: "zcode",
      label: "ZCode",
      status: zcodeHookStatus,
      note: zcodeHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: zcodeHookStatus.settingsPath,
            note: hookAgentNote("zcode"),
          })
        : i18n.t("register.zcode", {
            ns: "hooks",
            note: hookAgentNote("zcode"),
          }),
      onInstall: handleInstallZcodeHooks,
      onUninstall: handleUninstallZcodeHooks,
    },
    {
      key: "gemini",
      label: "Gemini CLI",
      status: geminiHookStatus,
      note: geminiHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: geminiHookStatus.settingsPath,
            note: hookAgentNote("gemini"),
          })
        : i18n.t("register.gemini", {
            ns: "hooks",
            note: hookAgentNote("gemini"),
          }),
      onInstall: handleInstallGeminiHooks,
      onUninstall: handleUninstallGeminiHooks,
    },
  ];

  const hooksNeedSetup =
    hookHealthHydrated && hookHealthAnalysis.needsFirstTimeSetup;
  const hooksNeedAttention =
    hookHealthHydrated &&
    (hookHealthAnalysis.needsFirstTimeSetup || hookHealthAnalysis.needsReconnect);
  const hooksSetupSummary = hookHealthAnalysis.summary;

  async function handleQuit() {
    setMenuOpen(false);
    await quitAtoll().catch(() => undefined);
  }

  async function handleArchiveAll() {
    setMenuOpen(false);
    const nextSnapshot = await archiveAllResolved().catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handleArchiveSession(sessionId: string) {
    const nextSnapshot = await archiveSession(sessionId).catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handlePinSession(sessionId: string, pinned: boolean) {
    const nextSnapshot = await pinSession(sessionId, pinned).catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handleArchiveCompletedSubagents(sessionId: string) {
    const nextSnapshot = await archiveCompletedSubagents(sessionId).catch(() => null);
    if (!nextSnapshot) {
      return;
    }
    applySnapshot(nextSnapshot);
    const currentView = panelViewRef.current;
    if (
      currentView.kind === "subagent"
      && currentView.sessionId === sessionId
      && !nextSnapshot.sessions
        .find((session) => session.sessionId === sessionId)
        ?.activeSubagents?.some((sub) => sub.agentId === currentView.agentId)
    ) {
      ++navigationSeqRef.current;
      setPanelView({ kind: "home" });
    }
  }

  function handleSelectAgent(agent: AgentKind) {
    setSelectedAgent(agent);
    if (panelView.kind !== "home") {
      ++navigationSeqRef.current;
      setPanelView({ kind: "home" });
    }
  }

  function openTokensPage(backTarget: "home" | "settings-main") {
    setMenuOpen(false);
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
    if (phaseRef.current === "expanded" && panelView.kind !== "settings") {
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
    setMenuOpen(false);
    setUsageBackTarget(backTarget);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "settings", page: "usage" });
  }

  function openClipboardPage() {
    setMenuOpen(false);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "clipboard" });
    getClipboardHistory()
      .then(setClipboardHistory)
      .catch(() => undefined);
  }

  function handleOpenClipboard() {
    openClipboardPage();
  }

  function openHistoryPage() {
    setMenuOpen(false);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    setPanelView({ kind: "history" });
  }

  function handleOpenHistory() {
    openHistoryPage();
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

  function handleOpenSettings() {
    setMenuOpen(false);
    setNavDirection("forward");
    setPanelAnimKey((key) => key + 1);
    if (phaseRef.current === "expanded" && panelView.kind !== "settings") {
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
    setMenuOpen(false);
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

  const isOpening = phase === "opening";
  const isClosing = phase === "closing";
  const isPresentationTransition = isOpening || isClosing;
  const isExpanded = phase === "opening" || phase === "expanded";
  const isExpandedChrome = phase === "expanded";
  const showAgentTabs = isExpandedChrome && tabAgents.length > 1;
  const showPanelAgentTabs =
    isExpandedChrome && panelView.kind === "home" && tabAgents.length > 1;
  const isMicro = phase === "micro";
  const isDormant =
    !isExpanded &&
    !isMicro &&
    !suppressPostCollapseSyncRef.current &&
    !holdCompactAfterSubviewOpenRef.current &&
    (collapsedMode === "dormant" ||
      (usesMicroIslandRef.current &&
        phase === "compact" &&
        sessions.length === 0 &&
        snapshot.pendingCount === 0));
  const showCompactHeaderMetrics =
    !isMicro && !isDormant && !isExpanded && !isPresentationTransition;
  const showMicroTokenCounter =
    isMicro && !isPresentationTransition && sessions.length > 0 &&
    (compactIndicator === "tokens" || compactIndicator === "both");
  const showCompactTokenCounter =
    sessions.length > 0 &&
    (compactIndicator === "tokens" || compactIndicator === "both");
  const showCompactMediaIndicator =
    (showCompactHeaderMetrics || isMicro) &&
    !isPresentationTransition &&
    (compactIndicator === "media" || compactIndicator === "both") &&
    nowPlayingTrack?.artworkBase64 != null;
  const showArtworkBackdrop =
    artworkBackdropEnabled &&
    nowPlayingTrack?.artworkBase64 != null &&
    (isExpanded || phase === "closing");
  const showExpandedTokenCounter = true;
  // Lyrics occupy a dedicated middle grid column — only on non-notched
  // displays (the notch area is physically invisible, so lyrics there would
  // be hidden). Show in both compact and dormant idle states so the user
  // sees lyrics even with no active agent sessions.
  const showLyricsMarquee =
    lyricsEnabled &&
    lyricsData != null &&
    lyricsData.lines.length > 0 &&
    !isMicro &&
    !isExpanded &&
    !isPresentationTransition &&
    !notchMetrics.hasNotch;
  const showCollapsedActivityStrip =
    !isDormant &&
    !isExpanded &&
    !isPresentationTransition &&
    (sessions.length > 0 || snapshot.pendingCount > 0);
  const showCompactNotchSpacer =
    collapsedMode === "compact" && !isExpanded && notchMetrics.hasNotch;
  const compactLeftSessions = sessions.slice(0, compactHeaderLayout.leftIconCount);
  const compactRightSessions = sessions.slice(
    compactHeaderLayout.leftIconCount,
    compactHeaderLayout.leftIconCount + compactHeaderLayout.rightIconCount,
  );
  const compactLeftOverflow =
    compactHeaderLayout.overflowCount > 0 &&
    compactHeaderLayout.rightIconCount === 0
      ? compactHeaderLayout.overflowCount
      : 0;
  const compactRightOverflow =
    compactHeaderLayout.overflowCount > 0 &&
    compactHeaderLayout.rightIconCount > 0
      ? compactHeaderLayout.overflowCount
      : 0;
  const isIdleExpanded =
    isExpandedChrome &&
    panelView.kind === "home" &&
    sessions.length === 0 &&
    snapshot.pendingCount === 0;
  const isSettingsExpanded =
    isExpandedChrome &&
    (panelView.kind === "settings" ||
      panelView.kind === "clipboard" ||
      panelView.kind === "history");
  const nativeExpandedPlan = isPlanExpanded && !isSettingsExpanded;
  const nativeExpandedSettings = isSettingsExpanded;
  const isSubview = isExpandedChrome && panelView.kind !== "home";
  const panelGlowAgent =
    selectedAgentRequest?.agent ?? selectedAgent ?? sessions[0]?.agent ?? null;
  const panelGlow = panelGlowAgent
    ? PANEL_GLOW[panelGlowAgent]
    : "rgba(111, 220, 255, 0.14)";
  const menuBarLogoSize = isExpanded ? 36 : isMicro ? 24 : 34;
  const subviewSession =
    panelView.kind === "session" || panelView.kind === "subagent" || panelView.kind === "subagentList"
      ? sessions.find((session) => session.sessionId === panelView.sessionId)
      : undefined;
  const subviewSubagent =
    panelView.kind === "subagent"
      ? subviewSession?.activeSubagents?.find((sub) => sub.agentId === panelView.agentId)
      : undefined;

  // Keep Rust-side compact metrics current while expanded so collapse targets
  // the latest width without a follow-up resize animation.
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

  // Keep the native window in sync when compact/expanded layout inputs change.
  // collapseIsland / expandIsland pre-mark the matching key so we do not replay
  // the same native animation right after a user-driven transition finishes.
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

  function renderPanel() {
    if (panelView.kind === "subagent") {
      if (!subviewSubagent) {
        return null;
      }
      return (
        <SubagentDetailView
          agentId={subviewSubagent.agentId}
          agent={subviewSession?.agent ?? "other"}
          agentType={subviewSubagent.agentType}
          startedAt={subviewSubagent.startedAt}
          completedAt={subviewSubagent.completedAt ?? null}
          lastMessage={subviewSubagent.lastMessage ?? null}
          transcriptPath={subviewSubagent.agentTranscriptPath ?? null}
          onArchive={async () => {
            const next = await archiveSubagent(subviewSubagent.agentId).catch(() => null);
            if (next) {
              applySnapshot(next);
              ++navigationSeqRef.current;
              setPanelView({ kind: "home" });
            }
          }}
        />
      );
    }

    if (panelView.kind === "subagentList") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      if (!session) return null;
      return (
        <SubagentListView
          subagents={session.activeSubagents ?? []}
          agent={session.agent}
          onSelectSubagent={(agentId) => navigateToSubagent(panelView.sessionId, agentId)}
          onArchiveCompletedSubagents={() => handleArchiveCompletedSubagents(panelView.sessionId)}
        />
      );
    }

    if (panelView.kind === "session") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      return (
        <SessionChatView
          sessionId={panelView.sessionId}
          transcriptPath={session?.transcriptPath ?? null}
          requests={sessionRequests}
          agent={session?.agent ?? "cursor"}
        />
      );
    }

    if (panelView.kind === "clipboard") {
      return (
        <ClipboardHistoryView
          entries={clipboardHistory}
          enabled={clipboardEnabled}
          onCopy={(id) => {
            copyClipboardEntry(id).catch(() => undefined);
          }}
          onClear={() => {
            clearClipboardHistory()
              .then(() => getClipboardHistory())
              .then(setClipboardHistory)
              .catch(() => undefined);
          }}
          onToggleFavorite={(id) => {
            toggleClipboardFavorite(id)
              .then((changed) => {
                if (!changed) return;
                return getClipboardHistory().then(setClipboardHistory);
              })
              .catch(() => undefined);
          }}
        />
      );
    }

    if (panelView.kind === "history") {
      return <ApprovalHistoryView />;
    }

    if (panelView.kind === "settings") {
      if (panelView.page === "hooks") {
        return (
          <HooksView
            agents={hookMenuAgents}
            hookBusy={hookBusy}
            hookInstallError={hookInstallError}
            onInstallAll={handleInstallAllHooks}
            onUninstallAll={handleUninstallHooks}
          />
        );
      }

      if (panelView.page === "tokens") {
        return (
          <TokenHeatmapView
            todayTokens={dailyTokens}
            todayTokensByModel={snapshot.dailyTokensByModel}
            displayMode={heatmapDisplay}
            pricingRates={pricingRates}
          />
        );
      }

      if (panelView.page === "usage") {
        return (
          <UsageSettingsView
            foldedCounterDisplay={foldedCounterDisplay}
            expandedCounterDisplay={expandedCounterDisplay}
            settingsBadgeDisplay={settingsBadgeDisplay}
            heatmapDisplay={heatmapDisplay}
            onChangeFoldedCounterDisplay={setFoldedCounterDisplay}
            onChangeExpandedCounterDisplay={setExpandedCounterDisplay}
            onChangeSettingsBadgeDisplay={setSettingsBadgeDisplay}
            onChangeHeatmapDisplay={setHeatmapDisplay}
            pricingModels={pricingModels}
            onPricingModelsChange={setPricingModels}
          />
        );
      }

      if (panelView.page === "island") {
        return (
          <IslandSettingsView
            maxCompactIcons={maxCompactIcons}
            maxCompactIconLimit={maxCompactIconLimit}
            onChangeMaxCompactIcons={(nextValue) =>
              setMaxCompactIcons(clampCompactIconLimit(nextValue, maxCompactIconLimit))
            }
            showFoldedIslandSizeSetting={supportsMicroIsland}
            foldedIslandSize={foldedIslandSize}
            onChangeFoldedIslandSize={handleChangeFoldedIslandSize}
            maxSubagentDisplay={maxSubagentDisplay}
            onChangeMaxSubagentDisplay={(nextValue) =>
              setMaxSubagentDisplay(clampMaxSubagentDisplay(nextValue))
            }
            showCompactIndicator={IS_MACOS}
            compactIndicator={compactIndicator}
            onChangeCompactIndicator={setCompactIndicatorState}
          />
        );
      }

      if (panelView.page === "media") {
        return (
          <MediaSettingsView
            mediaCardEnabled={mediaCardEnabled}
            onChangeMediaCardEnabled={handleChangeMediaCardEnabled}
            artworkBackdropEnabled={artworkBackdropEnabled}
            onChangeArtworkBackdropEnabled={handleChangeArtworkBackdropEnabled}
            lyricsEnabled={lyricsEnabled}
            onChangeLyricsEnabled={handleChangeLyricsEnabled}
          />
        );
      }

      if (panelView.page === "clipboard") {
        return (
          <ClipboardSettingsView
            clipboardHistoryEnabled={clipboardEnabled}
            onChangeClipboardHistoryEnabled={handleChangeClipboardEnabled}
            clipboardLimit={clipboardLimit}
            onChangeClipboardLimit={handleChangeClipboardLimit}
          />
        );
      }

      if (panelView.page === "sessions") {
        return (
          <SessionSettingsView
            retentionMinutes={retentionMinutes}
            onChangeRetentionMinutes={(nextValue) =>
              setRetentionMinutes(clampRetentionMinutes(nextValue))
            }
            subagentRetentionMinutes={subagentRetentionMinutes}
            onChangeSubagentRetentionMinutes={(nextValue) =>
              setSubagentRetentionMinutes(clampRetentionMinutes(nextValue))
            }
          />
        );
      }

      if (panelView.page === "mascot") {
        return (
          <MascotSettingsView
            idleIntervalSec={idleIntervalSec}
            onChangeIdleInterval={(v) => setIdleIntervalSec(clampIdleInterval(v))}
            idleDurationSec={idleDurationSec}
            onChangeIdleDuration={(v) => setIdleDurationSec(clampIdleDuration(v))}
          />
        );
      }

      if (panelView.page === "notifications") {
        return (
          <NotificationSettingsView
            mode={approvalNoticeMode}
            onChangeMode={handleChangeApprovalNoticeMode}
          />
        );
      }

      if (panelView.page === "shortcuts") {
        return (
          <ShortcutSettingsView
            config={globalShortcutView?.config ?? DEFAULT_GLOBAL_SHORTCUTS}
            errors={globalShortcutView?.errors}
            onChangeEnabled={(enabled) =>
              handleChangeGlobalShortcutConfig({
                ...(globalShortcutView?.config ?? DEFAULT_GLOBAL_SHORTCUTS),
                enabled,
              })
            }
            onChangeAccelerator={(action: ShortcutAction, value: string) =>
              handleChangeGlobalShortcutConfig(
                withShortcutAction(
                  globalShortcutView?.config ?? DEFAULT_GLOBAL_SHORTCUTS,
                  action,
                  value,
                ),
              )
            }
          />
        );
      }

      return (
        <SettingsView
          launchAtLogin={launchAtLogin}
          launchAtLoginBusy={launchAtLoginBusy}
          onChangeLaunchAtLogin={handleChangeLaunchAtLogin}
          language={language}
          onChangeLanguage={handleChangeLanguage}
          onOpenHooks={handleOpenHooksFromSettings}
          onOpenTokens={handleOpenTokensFromSettings}
          onOpenUsage={handleOpenUsageFromSettings}
          onOpenIsland={() => openSettingsSubpage("island")}
          onOpenMedia={() => openSettingsSubpage("media")}
          onOpenClipboard={() => openSettingsSubpage("clipboard")}
          onOpenSessions={() => openSettingsSubpage("sessions")}
          onOpenMascot={() => openSettingsSubpage("mascot")}
          onOpenNotifications={() => openSettingsSubpage("notifications")}
          onOpenShortcuts={() => openSettingsSubpage("shortcuts")}
          noticeModeLabel={tSettings(
            approvalNoticeMode === "notify"
              ? "notice.modeNotify"
              : "notice.modeInterrupt",
          )}
          todayLabel={settingsTodayLabel}
          usageDisplaySummary={usageDisplaySummary}
          hooksSummary={hooksSetupSummary}
          hooksNeedAttention={hooksNeedAttention}
          hooksAllConnected={hookHealthAnalysis.allConnected}
          showMediaSettings={IS_MACOS}
          mediaCardEnabled={mediaCardEnabled}
          clipboardHistoryEnabled={clipboardEnabled}
          shortcutsEnabled={globalShortcutView?.config.enabled ?? true}
        />
      );
    }

    if (selectedAgentRequest) {
      const planModeType = getPlanModeType(selectedAgentRequest);
      const handlePlanResolve = (nextSnapshot: IslandSnapshot) => {
        applySnapshot(nextSnapshot);
        if (nextSnapshot.pendingCount === 0) {
          collapseIsland(true);
          deactivateAtoll(
            selectedAgentRequest.agent,
            selectedAgentRequest.session,
            selectedAgentRequest.cwd,
          ).catch(() => undefined);
        }
      };

      if (planModeType === "question") {
        return (
          <PlanQuestionCard
            request={selectedAgentRequest}
            onResolve={handlePlanResolve}
          />
        );
      }

      if (planModeType === "exitPlan") {
        return (
          <PlanApprovalCard
            request={selectedAgentRequest}
            onResolve={handlePlanResolve}
          />
        );
      }

      return (
        <ApprovalCard
          request={selectedAgentRequest}
          busyDecision={busyDecision}
          sessions={filteredSessions}
          onApprove={() => resolveActive(selectedAgentRequest, "approved")}
          onDeny={() => resolveActive(selectedAgentRequest, "denied")}
          onAlwaysApprove={() => resolveActive(selectedAgentRequest, "approved", true)}
          onViewSession={navigateToSession}
        />
      );
    }

    if (filteredSessions.length > 0) {
      return (
        <SessionListView
          sessions={filteredSessions}
          activeRequest={selectedAgentRequest}
          justResolved={justResolved}
          isExpanded={isExpandedChrome}
          maxSubagentDisplay={maxSubagentDisplay}
          onSelectSession={navigateToSession}
          onSelectSubagent={navigateToSubagent}
          onArchiveSession={handleArchiveSession}
          onArchiveCompletedSubagents={handleArchiveCompletedSubagents}
          onPinSession={handlePinSession}
          onViewSubagentList={navigateToSubagentList}
        />
      );
    }

    return (
      <IdleView
        needsHookSetup={hooksNeedSetup}
        needsReconnect={hookHealthAnalysis.needsReconnect}
        disconnectedAgents={hookHealthAnalysis.disconnectedAgents}
        retrustAgents={hookHealthAnalysis.retrustAgents}
        onOpenHooks={handleOpenHooks}
      />
    );
  }

  return (
    <main className="stage">
      <section
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
          <div
            className={`island-artwork-backdrop${artworkBackdropOrigin ? " has-origin" : ""}${
              artworkBackdropRevealed ? " is-revealed" : ""
            }${artworkBackdropExitFade ? " is-exit-fade" : ""}${artworkIsDark ? " is-dark-art" : ""}`}
            style={
              artworkBackdropOrigin
                ? ({
                    // Percent geometry relative to the live window: as the
                    // native window shrinks during collapse, the backdrop
                    // rides proportionally toward the thumb instead of
                    // snapping to pixel coordinates measured pre-expand.
                    "--ab-left": `${(artworkBackdropOrigin.x / artworkBackdropOrigin.winW) * 100}%`,
                    "--ab-top": `${(artworkBackdropOrigin.y / artworkBackdropOrigin.winH) * 100}%`,
                    "--ab-w": `${(artworkBackdropOrigin.w / artworkBackdropOrigin.winW) * 100}%`,
                    "--ab-h": `${(artworkBackdropOrigin.h / artworkBackdropOrigin.winH) * 100}%`,
                  } as CSSProperties)
                : undefined
            }
            aria-hidden
          >
            <div className="island-artwork-backdrop-scale">
              <div
                className="island-artwork-backdrop-img"
                style={{
                  backgroundImage: `url(data:image/jpeg;base64,${nowPlayingTrack.artworkBase64})`,
                }}
              />
              <div
                className="island-artwork-backdrop-ghost"
                style={{
                  backgroundImage: `url(data:image/jpeg;base64,${nowPlayingTrack.artworkBase64})`,
                }}
              />
              <div className="island-artwork-backdrop-scrim" />
            </div>
          </div>
        ) : null}
        <header
          className={`island-header${showLyricsMarquee ? " has-lyrics" : ""}`}
          onMouseDown={startWindowDrag}
          title={isExpanded ? t("header.dragWindow") : t("header.hoverToOpen")}
        >
          <div
            className={`header-main ${showPanelAgentTabs ? "has-agent-tabs" : ""}${isSubview ? " has-subview-nav" : ""}`}
          >
            <span className="atoll-indicator-wrap">
              <span
                className={`atoll-indicator is-app-${appLogoState} ${snapshot.online ? "is-online" : "is-offline"}${hooksNeedAttention ? " is-hook-attention" : ""}`}
                title={
                  updateAvailable
                    ? t("update.available", { version: updateVersion })
                    : hookAttention
                }
                role={hooksNeedAttention ? "button" : undefined}
                tabIndex={hooksNeedAttention ? 0 : undefined}
                onClick={
                  hooksNeedAttention
                    ? (event) => {
                        event.stopPropagation();
                        handleOpenHooks();
                      }
                    : undefined
                }
                onKeyDown={
                  hooksNeedAttention
                    ? (event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          event.stopPropagation();
                          handleOpenHooks();
                        }
                      }
                    : undefined
                }
                data-no-drag
              >
                <span className="atoll-indicator-inner">
                  <HeaderLogo
                    display={collapsedHeaderLogo}
                    size={menuBarLogoSize}
                    idleIntervalSec={idleIntervalSec * 60}
                    idleDurationSec={idleDurationSec * 60}
                    motionPaused={isPresentationTransition}
                  />
                </span>
              </span>
            </span>
            {showCollapsedActivityStrip ? (
              <>
                <span
                  className={`listener-dot ${snapshot.online ? "online" : ""}`}
                  title={snapshot.online ? t("header.listening") : t("header.offline")}
                />
                {!isMicro ? (
                  <CompactSessionStack
                    sessions={compactLeftSessions}
                    overflowCount={compactLeftOverflow}
                    activeRequest={activeRequest}
                    justResolved={justResolved}
                  />
                ) : null}
              </>
            ) : panelView.kind === "subagent" ? (
              <SessionSubviewNav
                cwd={subviewSubagent?.agentType ?? ""}
                agent={subviewSession?.agent}
                sessionId={subviewSession?.sessionId}
                sessionHost={subviewSession?.sessionHost}
                onBack={navigateBack}
                onOpenExternal={() => {
                  collapseIsland(true);
                  void openAgentApp(
                    subviewSession?.agent ?? "other",
                    subviewSession?.cwd ?? "",
                    subviewSession?.sessionId,
                  );
                }}
              />
            ) : panelView.kind === "subagentList" ? (
              <SessionSubviewNav
                cwd="Subagents"
                agent={subviewSession?.agent}
                sessionId={subviewSession?.sessionId}
                sessionHost={subviewSession?.sessionHost}
                onBack={navigateBack}
                onOpenExternal={() => {
                  collapseIsland(true);
                  void openAgentApp(
                    subviewSession?.agent ?? "other",
                    subviewSession?.cwd ?? "",
                    subviewSession?.sessionId,
                  );
                }}
              />
            ) : panelView.kind === "session" ? (
              <SessionSubviewNav
                cwd={subviewSession?.cwd ?? ""}
                agent={subviewSession?.agent}
                sessionId={subviewSession?.sessionId}
                sessionHost={subviewSession?.sessionHost}
                onBack={navigateBack}
                onOpenExternal={() => {
                  collapseIsland(true);
                  void openAgentApp(
                    subviewSession?.agent ?? "other",
                    subviewSession?.cwd ?? "",
                    subviewSession?.sessionId,
                  );
                }}
              />
            ) : panelView.kind === "clipboard" ? (
              <SettingsPageNav
                onBack={navigateBack}
                backLabel={t("nav.back")}
                icon={<ClipboardList size={14} />}
                title={t("clipboard.title")}
              />
            ) : panelView.kind === "history" ? (
              <SettingsPageNav
                onBack={navigateBack}
                backLabel={t("nav.back")}
                icon={<History size={14} />}
                title={t("history.title")}
              />
            ) : panelView.kind === "settings" && panelView.page === "hooks" ? (
              <SettingsPageNav
                onBack={navigateBackFromHooks}
                backLabel={hooksBackTarget === "settings-main" ? t("nav.settings") : t("nav.back")}
                icon={<Download size={14} />}
                title={t("title", { ns: "hooks" })}
              />
            ) : panelView.kind === "settings" && panelView.page === "tokens" ? (
              <SettingsPageNav
                onBack={navigateBackFromTokens}
                backLabel={tokensBackTarget === "settings-main" ? t("nav.settings") : t("nav.back")}
                icon={<Activity size={14} />}
                title={t("nav.tokenActivity")}
              />
            ) : panelView.kind === "settings" && panelView.page === "usage" ? (
              <SettingsPageNav
                onBack={navigateBackFromUsage}
                backLabel={usageBackTarget === "settings-main" ? t("nav.settings") : t("nav.back")}
                icon={<CircleDollarSign size={14} />}
                title={t("nav.displayPricing")}
              />
            ) : panelView.kind === "settings" && panelView.page === "island" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<Layers size={14} />}
                title={t("nav.island")}
              />
            ) : panelView.kind === "settings" && panelView.page === "media" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<Music size={14} />}
                title={t("nav.media")}
              />
            ) : panelView.kind === "settings" && panelView.page === "clipboard" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<ClipboardList size={14} />}
                title={t("nav.clipboard")}
              />
            ) : panelView.kind === "settings" && panelView.page === "sessions" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<Clock size={14} />}
                title={t("nav.sessions")}
              />
            ) : panelView.kind === "settings" && panelView.page === "mascot" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<Sparkles size={14} />}
                title={t("nav.mascot")}
              />
            ) : panelView.kind === "settings" && panelView.page === "notifications" ? (
              <SettingsPageNav
                onBack={navigateBackToSettingsMain}
                backLabel={t("nav.settings")}
                icon={<Bell size={14} />}
                title={t("nav.notifications")}
              />
            ) : panelView.kind === "settings" ? (
              <SettingsSubviewNav onBack={navigateBack} />
            ) : showPanelAgentTabs ? (
              <div
                className={`header-agent-tabs${notchMetrics.hasNotch ? " header-agent-tabs--compact" : ""}`}
                data-no-drag
              >
                <AgentTabBar
                  agents={tabAgents}
                  selectedAgent={selectedAgent}
                  pendingCountByAgent={pendingCountByAgent}
                  showTabs={showAgentTabs}
                  compact={notchMetrics.hasNotch}
                  online={snapshot.online}
                  onSelectAgent={handleSelectAgent}
                />
              </div>
            ) : null}
          </div>

          {showCompactNotchSpacer ? (
            <span className="header-notch-spacer" aria-hidden="true" />
          ) : null}

          {showLyricsMarquee ? (
            <LyricsMarquee
              // Until the payload for the *current* track arrives (fetched
              // on track change), render no lines — the marquee keeps its
              // column-mounted placeholder instead of showing the previous
              // track's lyrics against this track's position.
              lines={lyricsMatchTrack(lyricsData, nowPlayingTrack) ? lyricsData!.lines : []}
              position={playbackPosition?.position ?? null}
              playing={playbackPosition?.playing ?? false}
            />
          ) : null}

          {showCompactHeaderMetrics || showMicroTokenCounter ? (
            <div
              className={`header-metrics${
                isMicro ? " is-micro-metrics" : ""
              }${isPresentationTransition ? ` is-${phase}` : ""}`}
            >
              {showCompactHeaderMetrics && compactRightSessions.length > 0 ? (
                <CompactSessionStack
                  placement="right"
                  sessions={compactRightSessions}
                  overflowCount={compactRightOverflow}
                  activeRequest={activeRequest}
                  justResolved={justResolved}
                />
              ) : null}
              {showCompactTokenCounter ? (
                <TokenCounter
                  value={
                    foldedCounterDisplay === "cost"
                      ? activeSessionCostTotal
                      : activeSessionTokenTotal
                  }
                  usage={activeSessionTokens}
                  variant={isMicro ? "micro" : "compact"}
                  displayMode={foldedCounterDisplay}
                  suppressAnimations={isPresentationTransition}
                  sessionCount={sessions.length}
                  maxCompactIcons={maxCompactIcons}
                  compactTokenLevel={compactHeaderLayout.tokenCompactLevel}
                />
              ) : null}
              {showCompactMediaIndicator && nowPlayingTrack?.artworkBase64 ? (
                <img
                  ref={compactMediaThumbRef}
                  className="compact-media-thumb"
                  src={`data:image/jpeg;base64,${nowPlayingTrack.artworkBase64}`}
                  alt=""
                  draggable={false}
                />
              ) : null}
              {showCompactHeaderMetrics && snapshot.pendingCount > 0 ? (
                <span className="pending-badge-slot">
                  <span
                    className="pending-badge"
                    aria-label={t("header.pendingAria", { count: snapshot.pendingCount })}
                  >
                    {snapshot.pendingCount}
                  </span>
                </span>
              ) : null}
            </div>
          ) : null}

          {isExpandedChrome &&
          panelView.kind !== "session" &&
          panelView.kind !== "subagent" &&
          panelView.kind !== "subagentList" ? (
          <div
            className="header-actions"
            data-no-drag
            ref={menuRef}
            onMouseDown={handleControlMouseDown}
          >
            {showExpandedTokenCounter && !isDormant ? (
              <TokenCounter
                value={
                  expandedCounterDisplay === "cost" ? dailyCostTotal : dailyTokenTotal
                }
                usage={dailyTokens}
                variant="expanded"
                displayMode={expandedCounterDisplay}
                onClick={handleOpenTokensFromCounter}
              />
            ) : null}
            <button
              className="icon-button"
              type="button"
              onClick={handleOpenClipboard}
              aria-label={t("clipboard.title")}
              tabIndex={isExpandedChrome ? 0 : -1}
            >
              <ClipboardList size={16} />
            </button>
            <button
              className="icon-button"
              type="button"
              onClick={handleOpenHistory}
              aria-label={t("history.title")}
              tabIndex={isExpandedChrome ? 0 : -1}
            >
              <History size={16} />
            </button>
            <button
              className="icon-button"
              type="button"
              onClick={() => collapseIsland(true)}
              aria-label={t("header.collapse")}
              tabIndex={isExpandedChrome ? 0 : -1}
            >
              <ChevronUp size={16} />
            </button>
            <button
              className={`icon-button${updateAvailable ? " has-update" : ""}`}
              type="button"
              onClick={() => setMenuOpen((open) => !open)}
              aria-label={t("header.moreOptions")}
              aria-expanded={menuOpen}
              tabIndex={isExpandedChrome ? 0 : -1}
            >
              <Ellipsis size={17} />
            </button>
            {menuOpen ? (
              <div className="more-menu" role="menu">
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleOpenHooks}
                >
                  <Download size={14} />
                  {t("menu.agentHooks")}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleArchiveAll}
                >
                  <Archive size={14} />
                  {t("menu.archiveAll")}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleOpenSettings}
                >
                  <Settings2 size={14} />
                  {t("menu.settings")}
                </button>
                {updateDownloading ? (
                  <button type="button" role="menuitem" disabled>
                    <RefreshCw size={14} />
                    {t("update.downloading", {
                      percent: Math.round(updateDownloadProgress * 100),
                    })}
                  </button>
                ) : updateAvailable ? (
                  <button
                    type="button"
                    role="menuitem"
                    className="accent"
                    onClick={handleInstallUpdate}
                  >
                    <ArrowUpCircle size={14} />
                    {t("update.updateTo", { version: updateVersion })}
                  </button>
                ) : (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={handleCheckForUpdates}
                    disabled={updateChecking}
                  >
                    <RefreshCw size={14} />
                    {updateChecking ? t("update.checking") : t("update.checkForUpdates")}
                  </button>
                )}
                <div className="menu-separator" />
                <button
                  type="button"
                  role="menuitem"
                  className="danger"
                  onClick={handleQuit}
                >
                  <Power size={14} />
                  {t("menu.quit")}
                </button>
              </div>
            ) : null}
          </div>
          ) : null}

        </header>

        {!isPresentationTransition ? (
          <div
            className="island-panel"
            data-nav={navDirection ?? undefined}
          >
            <div key={panelAnimKey} className="island-panel-content">
              {renderPanel()}
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
      </section>
    </main>
  );
}
