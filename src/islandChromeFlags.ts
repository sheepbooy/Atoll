// Pure derivation of the island's chrome state: which pieces are visible for
// the current phase/panel/layout combination, plus the derived subview rows
// and panel glow. Extracted verbatim from App.tsx.
import { collapsedBandHeight } from "./islandLayout";
import { PANEL_GLOW } from "./agents";
import type { NotchMetrics } from "./tauri";
import type { AgentKind, PanelView } from "./appTypes";
import type { CompactIndicatorMode } from "./displayPrefs";
import type { SessionSummary } from "./tauri/types";
import type { PermissionRequest } from "./tauri";
import type { LyricPayload } from "./tauri";
import type { CompactHeaderLayout } from "./compactLayout";

interface IslandChromeFlagsInput {
  phase: string;
  panelView: PanelView;
  collapsedMode: "micro" | "compact" | "dormant";
  usesMicroIsland: boolean;
  suppressPostCollapseSync: boolean;
  holdCompactAfterSubviewOpen: boolean;
  sessions: SessionSummary[];
  pendingCount: number;
  isPlanExpanded: boolean;
  notchMetrics: NotchMetrics;
  compactIndicator: CompactIndicatorMode;
  nowPlayingTrack: { artworkBase64: string | null | undefined } | null;
  artworkBackdropEnabled: boolean;
  lyricsEnabled: boolean;
  lyricsData: LyricPayload | null;
  selectedAgentRequest: PermissionRequest | null;
  selectedAgent: AgentKind | null;
  compactHeaderLayout: CompactHeaderLayout;
  tabAgents: AgentKind[];
}

export function deriveIslandChromeFlags({
  phase,
  panelView,
  collapsedMode,
  usesMicroIsland,
  suppressPostCollapseSync,
  holdCompactAfterSubviewOpen,
  sessions,
  pendingCount,
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
}: IslandChromeFlagsInput) {
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
    !suppressPostCollapseSync &&
    !holdCompactAfterSubviewOpen &&
    (collapsedMode === "dormant" ||
      (usesMicroIsland &&
        phase === "compact" &&
        sessions.length === 0 &&
        pendingCount === 0));
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
    (sessions.length > 0 || pendingCount > 0);
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
    pendingCount === 0;
  const isSettingsExpanded =
    isExpandedChrome &&
    (panelView.kind === "settings" ||
      panelView.kind === "clipboard" ||
      panelView.kind === "fileStation" ||
      panelView.kind === "history");
  const nativeExpandedPlan = isPlanExpanded && !isSettingsExpanded;
  const nativeExpandedSettings = isSettingsExpanded;
  const isSubview = isExpandedChrome && panelView.kind !== "home";
  const panelGlowAgent =
    selectedAgentRequest?.agent ?? selectedAgent ?? sessions[0]?.agent ?? null;
  const panelGlow = panelGlowAgent
    ? PANEL_GLOW[panelGlowAgent]
    : "rgba(111, 220, 255, 0.14)";
  // Logo shrinks on short menu-bar bands (e.g. 24pt on non-notched displays)
  // so it never clips vertically inside the collapsed capsule.
  const menuBarLogoSize = isExpanded
    ? 36
    : isMicro
      ? 24
      : Math.min(34, Math.max(18, Math.round(collapsedBandHeight(notchMetrics)) - 4));
  const subviewSession =
    panelView.kind === "session" || panelView.kind === "subagent" || panelView.kind === "subagentList"
      ? sessions.find((session) => session.sessionId === panelView.sessionId)
      : undefined;
  const subviewSubagent =
    panelView.kind === "subagent"
      ? subviewSession?.activeSubagents?.find((sub) => sub.agentId === panelView.agentId)
      : undefined;

  return {
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
  };
}
