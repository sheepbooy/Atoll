// The island header: app logo, collapsed activity strip / subview navs /
// agent tabs, notch spacer, lyrics marquee, compact metrics row, and the
// expanded chrome (token counter, quick actions, more menu). Extracted
// verbatim from App.tsx.
import type { RefObject } from "react";
import { useTranslation } from "react-i18next";
import {
  Activity,
  Archive,
  ArrowUpCircle,
  Bell,
  ChevronUp,
  CircleDollarSign,
  ClipboardList,
  Clock,
  Download,
  Ellipsis,
  History,
  Inbox,
  Layers,
  Music,
  Power,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import {
  type IslandSnapshot,
  type NowPlayingTrack,
  type NotchMetrics,
} from "../tauri";
import type { AgentKind, PanelView } from "../appTypes";
import { AgentTabBar } from "./AgentTabBar";
import { CompactSessionStack } from "./CompactSessionStack";
import { HeaderLogo } from "./HeaderLogo";
import { SessionSubviewNav } from "./SessionSubviewNav";
import { SettingsPageNav, SettingsSubviewNav } from "./SettingsNavs";
import { LyricsMarquee, lyricsMatchTrack } from "../LyricsMarquee";
import type { PlaybackPositionSample } from "../hooks/useLyrics";
import { TokenCounter } from "../TokenCounter";
import { BluetoothBatteryRing } from "../BluetoothBatteryRing";
import type { UsageDisplayMode } from "../displayPrefs";
import type { SalarySettings } from "../salarySettings";
import type { SessionSummary } from "../tauri/types";
import type { AtollReaction } from "../AtollLogo";
import type { BluetoothDeviceBattery } from "../tauri";
import type { HeaderLogoDisplay } from "../hookHealth";
import type { LyricPayload } from "../tauri";
import type { PermissionRequest } from "../tauri";
import type { CompactHeaderLayout } from "../compactLayout";

interface IslandHeaderProps {
  t: ReturnType<typeof useTranslation>["t"];
  phase: string;
  panelView: PanelView;
  isExpanded: boolean;
  isExpandedChrome: boolean;
  isPresentationTransition: boolean;
  isMicro: boolean;
  isDormant: boolean;
  isSubview: boolean;
  showPanelAgentTabs: boolean;
  showAgentTabs: boolean;
  showCollapsedActivityStrip: boolean;
  showCompactHeaderMetrics: boolean;
  showMicroTokenCounter: boolean;
  showCompactTokenCounter: boolean;
  showCompactMediaIndicator: boolean;
  showCompactNotchSpacer: boolean;
  showLyricsMarquee: boolean;
  // Folded-island Bluetooth battery ring (right metrics row)
  bluetoothDevices: BluetoothDeviceBattery[];
  bluetoothBatteryEnabled: boolean;
  bluetoothAlertThreshold: number;

  startWindowDrag: (event: React.MouseEvent<HTMLElement>) => void;
  atollIndicatorRef: RefObject<HTMLSpanElement>;
  menuRef: RefObject<HTMLDivElement>;
  compactMediaThumbRef: RefObject<HTMLImageElement>;

  appLogoState: string;
  online: boolean;
  pendingCount: number;
  hooksNeedAttention: boolean;
  hookAttention: string;
  updateAvailable: boolean;
  updateVersion: string | null;
  handleOpenHooks: () => void;

  collapsedHeaderLogo: HeaderLogoDisplay;
  menuBarLogoSize: number;
  idleIntervalMin: number;
  idleDurationMin: number;
  logoReaction: AtollReaction | null;
  logoReactionKey: number;
  logoStashLevel: number;
  dragOverIsland: boolean;

  compactLeftSessions: SessionSummary[];
  compactRightSessions: SessionSummary[];
  compactLeftOverflow: number;
  compactRightOverflow: number;
  activeRequest: PermissionRequest | null;
  justResolved: boolean;

  subviewSession: SessionSummary | undefined;
  subviewSubagent:
    | {
        agentType: string;
        agentId: string;
      }
    | undefined;
  navigateBack: () => void;
  navigateBackFromHooks: () => void;
  navigateBackFromTokens: () => void;
  navigateBackFromUsage: () => void;
  navigateBackToSettingsMain: () => void;
  hooksBackTarget: string;
  tokensBackTarget: string;
  usageBackTarget: string;
  collapseIsland: (skipAnimation?: boolean) => void;
  openAgentApp: (
    agent: AgentKind,
    cwd: string,
    sessionId?: string,
  ) => Promise<void> | void;

  notchMetrics: NotchMetrics;
  tabAgents: AgentKind[];
  selectedAgent: AgentKind | null;
  pendingCountByAgent: Record<AgentKind, number>;
  handleSelectAgent: (agent: AgentKind) => void;

  lyricsData: LyricPayload | null;
  nowPlayingTrack: NowPlayingTrack | null;
  playbackPosition: PlaybackPositionSample | null;

  compactHeaderLayout: CompactHeaderLayout;
  activeSessionTokens: NonNullable<IslandSnapshot["activeSessionTokens"]>;
  activeSessionTokenTotal: number;
  activeSessionCostTotal: number;
  dailyTokens: NonNullable<IslandSnapshot["dailyTokens"]>;
  dailyTokenTotal: number;
  dailyCostTotal: number;
  foldedCounterDisplay: UsageDisplayMode;
  expandedCounterDisplay: UsageDisplayMode;
  /** Salary settings for the "salary" display mode of the two counters. */
  salary?: SalarySettings;
  maxCompactIcons: number;

  handleControlMouseDown: (event: React.MouseEvent<HTMLElement>) => void;
  handleOpenTokensFromCounter: () => void;
  stagedCount: number;
  handleOpenFileStation: () => void;
  handleOpenClipboard: () => void;
  handleOpenHistory: () => void;
  menuOpen: boolean;
  setMenuOpen: (open: boolean | ((open: boolean) => boolean)) => void;
  handleArchiveAll: () => void;
  handleOpenSettings: () => void;
  updateDownloading: boolean;
  updateDownloadProgress: number;
  updateChecking: boolean;
  handleInstallUpdate: () => void;
  handleCheckForUpdates: () => void;
  handleQuit: () => void;
  sessionsCount: number;
}

export function IslandHeader(props: IslandHeaderProps) {
  const {
    t,
    phase,
    panelView,
    isExpanded,
    isExpandedChrome,
    isPresentationTransition,
    isMicro,
    isDormant,
    isSubview,
    showPanelAgentTabs,
    showAgentTabs,
    showCollapsedActivityStrip,
    showCompactHeaderMetrics,
    showMicroTokenCounter,
    showCompactTokenCounter,
    showCompactMediaIndicator,
    showCompactNotchSpacer,
    showLyricsMarquee,
    bluetoothDevices,
    bluetoothBatteryEnabled,
    bluetoothAlertThreshold,
    startWindowDrag,
    atollIndicatorRef,
    menuRef,
    compactMediaThumbRef,
    appLogoState,
    online,
    pendingCount,
    hooksNeedAttention,
    hookAttention,
    updateAvailable,
    updateVersion,
    handleOpenHooks,
    collapsedHeaderLogo,
    menuBarLogoSize,
    idleIntervalMin,
    idleDurationMin,
    logoReaction,
    logoReactionKey,
    logoStashLevel,
    dragOverIsland,
    compactLeftSessions,
    compactRightSessions,
    compactLeftOverflow,
    compactRightOverflow,
    activeRequest,
    justResolved,
    subviewSession,
    subviewSubagent,
    navigateBack,
    navigateBackFromHooks,
    navigateBackFromTokens,
    navigateBackFromUsage,
    navigateBackToSettingsMain,
    hooksBackTarget,
    tokensBackTarget,
    usageBackTarget,
    collapseIsland,
    openAgentApp,
    notchMetrics,
    tabAgents,
    selectedAgent,
    pendingCountByAgent,
    handleSelectAgent,
    lyricsData,
    nowPlayingTrack,
    playbackPosition,
    compactHeaderLayout,
    activeSessionTokens,
    activeSessionTokenTotal,
    activeSessionCostTotal,
    dailyTokens,
    dailyTokenTotal,
    dailyCostTotal,
    foldedCounterDisplay,
    expandedCounterDisplay,
    salary,
    maxCompactIcons,
    handleControlMouseDown,
    handleOpenTokensFromCounter,
    stagedCount,
    handleOpenFileStation,
    handleOpenClipboard,
    handleOpenHistory,
    menuOpen,
    setMenuOpen,
    handleArchiveAll,
    handleOpenSettings,
    updateDownloading,
    updateDownloadProgress,
    updateChecking,
    handleInstallUpdate,
    handleCheckForUpdates,
    handleQuit,
    sessionsCount,
  } = props;
  const showExpandedTokenCounter = true;

  return (
    <header
      className={`island-header${showLyricsMarquee ? " has-lyrics" : ""}`}
      onMouseDown={startWindowDrag}
      title={isExpanded ? t("header.dragWindow") : t("header.hoverToOpen")}
    >
      <div
        className={`header-main ${showPanelAgentTabs ? "has-agent-tabs" : ""}${isSubview ? " has-subview-nav" : ""}`}
      >
        <span className="atoll-indicator-wrap" ref={atollIndicatorRef}>
          <span
            className={`atoll-indicator is-app-${appLogoState} ${online ? "is-online" : "is-offline"}${hooksNeedAttention ? " is-hook-attention" : ""}`}
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
                idleIntervalSec={idleIntervalMin * 60}
                idleDurationSec={idleDurationMin * 60}
                motionPaused={isPresentationTransition}
                reaction={logoReaction}
                reactionKey={logoReactionKey}
                stashLevel={logoStashLevel}
                mouthOpen={dragOverIsland}
              />
            </span>
          </span>
        </span>
        {showCollapsedActivityStrip ? (
          <>
            <span
              className={`listener-dot ${online ? "online" : ""}`}
              title={online ? t("header.listening") : t("header.offline")}
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
        ) : panelView.kind === "fileStation" ? (
          <SettingsPageNav
            onBack={navigateBack}
            backLabel={t("nav.back")}
            icon={<Inbox size={14} />}
            title={t("fileStation.title")}
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
              online={online}
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
          sample={playbackPosition}
        />
      ) : null}

      {showCompactHeaderMetrics || showMicroTokenCounter ? (
        <div
          className={`header-metrics${
            isMicro ? " is-micro-metrics" : ""
          }${isPresentationTransition ? ` is-${phase}` : ""}`}
        >
          {bluetoothBatteryEnabled &&
          !isPresentationTransition &&
          (showCompactHeaderMetrics || isMicro) ? (
            <BluetoothBatteryRing
              devices={bluetoothDevices}
              alertThreshold={bluetoothAlertThreshold}
            />
          ) : null}
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
                  : foldedCounterDisplay === "salary"
                    ? 0
                    : activeSessionTokenTotal
              }
              usage={activeSessionTokens}
              variant={isMicro ? "micro" : "compact"}
              displayMode={foldedCounterDisplay}
              salary={salary}
              suppressAnimations={isPresentationTransition}
              sessionCount={sessionsCount}
              maxCompactIcons={maxCompactIcons}
              compactTokenLevel={
                foldedCounterDisplay === "salary"
                  ? undefined
                  : compactHeaderLayout.tokenCompactLevel
              }
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
          {showCompactHeaderMetrics && pendingCount > 0 ? (
            <span className="pending-badge-slot">
              <span
                className="pending-badge"
                aria-label={t("header.pendingAria", { count: pendingCount })}
              >
                {pendingCount}
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
              expandedCounterDisplay === "cost"
                ? dailyCostTotal
                : expandedCounterDisplay === "salary"
                  ? 0
                  : dailyTokenTotal
            }
            usage={dailyTokens}
            variant="expanded"
            displayMode={expandedCounterDisplay}
            salary={salary}
            onClick={handleOpenTokensFromCounter}
          />
        ) : null}
        <button
          className={`icon-button header-stash-btn${stagedCount > 0 ? " has-stash" : ""}`}
          type="button"
          onClick={handleOpenFileStation}
          aria-label={t("fileStation.title")}
          tabIndex={isExpandedChrome ? 0 : -1}
        >
          <Inbox size={16} />
          {stagedCount > 0 ? (
            <span className="header-stash-badge">
              {stagedCount > 99 ? "99+" : stagedCount}
            </span>
          ) : null}
        </button>
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
  );
}
