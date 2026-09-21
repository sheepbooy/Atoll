// Panel routing for the expanded island: maps the active PanelView to the
// matching View component. Extracted verbatim from App.renderPanel so App.tsx
// stays a hook-orchestration shell; all behavior lives in the leaf Views.
import { useTranslation } from "react-i18next";
import i18n from "../i18n";
import {
  archiveSubagent,
  beginStagedFilesDrag,
  clearClipboardHistory,
  copyClipboardEntry,
  copyStagedFilesToClipboard,
  copyStagedPathsToClipboard,
  deactivateAtoll,
  getClipboardHistory,
  revealPath,
  stageClipboardEntries,
  toggleClipboardFavorite,
  type ApprovalNoticeMode,
  type ClipboardEntry,
  type GlobalShortcutView,
  type HookHealthSnapshot,
  type IslandSnapshot,
  type PermissionRequest,
  type SessionSummary,
  type StagedFile,
} from "../tauri";
import type {
  AgentKind,
  Decision,
  FoldedIslandSize,
  PanelView,
  SettingsPage,
} from "../appTypes";
import type { CompactIndicatorMode, UsageDisplayMode } from "../displayPrefs";
import type { ModelPricingEntry, pricingRateMap } from "../pricing";
import type { TokenUsage } from "../tauri/types";
import type { AppLanguage } from "../i18n";
import {
  ApprovalHistoryView,
} from "../ApprovalHistoryView";
import {
  ClipboardHistoryView,
} from "../ClipboardHistoryView";
import {
  FileStationView,
} from "../FileStationView";
import {
  RulesSettingsView,
} from "../RulesSettingsView";
import {
  ClipboardSettingsView,
  IslandSettingsView,
  MascotSettingsView,
  MediaSettingsView,
  NotificationSettingsView,
  SessionSettingsView,
  ShortcutSettingsView,
} from "../SettingsPages";
import {
  SettingsView,
} from "../SettingsView";
import {
  TokenHeatmapView,
} from "../TokenHeatmapView";
import {
  UsageSettingsView,
} from "../UsageSettingsView";
import {
  ApprovalCard,
} from "./ApprovalCard";
import {
  HooksView,
  type HookMenuAgent,
} from "./HooksView";
import {
  IdleView,
} from "./IdleView";
import {
  PlanApprovalCard,
} from "./PlanApprovalCard";
import {
  PlanQuestionCard,
} from "./PlanQuestionCard";
import {
  SessionChatView,
} from "./SessionChatView";
import {
  SessionListView,
} from "./SessionListView";
import {
  SubagentDetailView,
} from "./SubagentDetailView";
import {
  SubagentListView,
} from "./SubagentListView";
import {
  DEFAULT_GLOBAL_SHORTCUTS,
  withShortcutAction,
} from "../shortcuts";
import {
  clampCompactIconLimit,
  clampIdleDuration,
  clampIdleInterval,
  clampMaxSubagentDisplay,
  clampRetentionMinutes,
} from "../settingsStorage";
import { getPlanModeType } from "../planMode";
import { hookAgentNote, type HookAgentKey, type HookHealthAnalysis } from "../hookHealth";
import { IS_MACOS } from "../platform";
import type { AtollReaction } from "../AtollLogo";
import type { ShortcutAction } from "../tauri";

interface IslandPanelRouterProps {
  panelView: PanelView;
  sessions: SessionSummary[];
  sessionRequests: PermissionRequest[];
  navigationSeqRef: { current: number };
  setPanelView: (view: PanelView) => void;
  applySnapshot: (snapshot: IslandSnapshot, options?: { mergeHookHealth?: boolean }) => void;

  // Hook registration
  hookHealth: HookHealthSnapshot;
  hookBusy: HookAgentKey | "all" | false;
  hookInstallError: string | null;
  handleInstallClaudeHooks: () => void;
  handleInstallCodexHooks: () => void;
  handleInstallZcodeHooks: () => void;
  handleInstallGeminiHooks: () => void;
  handleInstallOpencodeHooks: () => void;
  handleInstallCursorHooks: () => void;
  handleInstallAllHooks: () => void;
  handleUninstallClaudeHooks: () => void;
  handleUninstallCodexHooks: () => void;
  handleUninstallZcodeHooks: () => void;
  handleUninstallGeminiHooks: () => void;
  handleUninstallOpencodeHooks: () => void;
  handleUninstallCursorHooks: () => void;
  handleUninstallHooks: () => void;
  handleRemoveCompetingClaudeHooks: () => void;

  // Approvals
  selectedAgentRequest: PermissionRequest | null;
  busyDecision: Decision | null;
  filteredSessions: SessionSummary[];
  resolveActive: (
    request: PermissionRequest,
    decision: "approved" | "denied",
    alwaysAllow?: boolean,
    note?: string,
    ruleScope?: import("../tauri").ApprovalRuleScope,
  ) => void;
  collapseIsland: (skipAnimation?: boolean) => void;
  justResolved: boolean;
  isExpandedChrome: boolean;

  // Session list actions
  navigateToSession: (sessionId: string) => void;
  navigateToSubagent: (sessionId: string, agentId: string) => void;
  navigateToSubagentList: (sessionId: string) => void;
  handleArchiveSession: (sessionId: string) => void;
  handleArchiveCompletedSubagents: (sessionId: string) => void;
  handlePinSession: (sessionId: string, pinned: boolean) => void;
  maxSubagentDisplay: number;
  setMaxSubagentDisplay: (value: number) => void;

  // Clipboard panel
  clipboardHistory: ClipboardEntry[];
  clipboardEnabled: boolean;
  setClipboardHistory: (entries: ClipboardEntry[]) => void;
  /** Stage a clipboard history entry into the file station. */
  stageClipboardEntry: (id: string) => Promise<boolean>;

  // File station panel
  stagedFiles: StagedFile[];
  removeStaged: (id: string) => void;
  clearStaged: () => void;
  playStashReaction: (reaction: AtollReaction) => void;
  /** Copy staged paths as newline-joined text. */
  copyStagedPaths: (ids: string[]) => void;

  // Settings: usage & pricing
  dailyTokens: TokenUsage;
  dailyTokensByModel: IslandSnapshot["dailyTokensByModel"];
  heatmapDisplay: UsageDisplayMode;
  pricingRates: ReturnType<typeof pricingRateMap>;
  pricingModels: ModelPricingEntry[];
  setPricingModels: (models: ModelPricingEntry[]) => void;
  foldedCounterDisplay: UsageDisplayMode;
  expandedCounterDisplay: UsageDisplayMode;
  settingsBadgeDisplay: UsageDisplayMode;
  setFoldedCounterDisplay: (mode: UsageDisplayMode) => void;
  setExpandedCounterDisplay: (mode: UsageDisplayMode) => void;
  setSettingsBadgeDisplay: (mode: UsageDisplayMode) => void;
  setHeatmapDisplay: (mode: UsageDisplayMode) => void;

  // Settings: island
  maxCompactIcons: number;
  maxCompactIconLimit: number;
  setMaxCompactIcons: (value: number) => void;
  supportsMicroIsland: boolean;
  foldedIslandSize: FoldedIslandSize;
  handleChangeFoldedIslandSize: (small: boolean) => void;
  compactIndicator: CompactIndicatorMode;
  setCompactIndicatorState: (mode: CompactIndicatorMode) => void;
  preferredMonitorName: string | null;
  handleChangePreferredMonitor: (name: string | null) => void;

  // Settings: media & clipboard & sessions & mascot
  mediaCardEnabled: boolean;
  handleChangeMediaCardEnabled: (enabled: boolean) => void;
  artworkBackdropEnabled: boolean;
  handleChangeArtworkBackdropEnabled: (enabled: boolean) => void;
  lyricsEnabled: boolean;
  handleChangeLyricsEnabled: (enabled: boolean) => void;
  clipboardLimit: number;
  handleChangeClipboardLimit: (limit: number) => void;
  handleChangeClipboardEnabled: (enabled: boolean) => void;
  clipboardAutoStage: boolean;
  handleChangeClipboardAutoStage: (enabled: boolean) => void;
  retentionMinutes: number;
  setRetentionMinutes: (minutes: number) => void;
  subagentRetentionMinutes: number;
  setSubagentRetentionMinutes: (minutes: number) => void;
  idleIntervalMin: number;
  setIdleIntervalMin: (value: number) => void;
  idleDurationMin: number;
  setIdleDurationMin: (value: number) => void;

  // Settings: notifications, shortcuts, language, main page
  approvalNoticeMode: ApprovalNoticeMode;
  handleChangeApprovalNoticeMode: (mode: ApprovalNoticeMode) => void;
  globalShortcutView: GlobalShortcutView | null;
  handleChangeGlobalShortcutConfig: (config: GlobalShortcutView["config"]) => void;
  language: AppLanguage;
  handleChangeLanguage: (language: AppLanguage) => void;
  handleOpenHooksFromSettings: () => void;
  handleOpenTokensFromSettings: () => void;
  handleOpenUsageFromSettings: () => void;
  openSettingsSubpage: (
    page: "sessions" | "media" | "island" | "clipboard" | "mascot" | "notifications" | "shortcuts" | "rules",
  ) => void;
  settingsTodayLabel: string;
  usageDisplaySummary: string;
  hooksSetupSummary: string;
  hooksNeedAttention: boolean;
  hooksNeedSetup: boolean;
  hookHealthAnalysis: HookHealthAnalysis;
  handleOpenHooks: () => void;

  // Settings: autostart
  launchAtLogin: boolean;
  launchAtLoginBusy: boolean;
  handleChangeLaunchAtLogin: (enabled: boolean) => void;
}

export function IslandPanelRouter(props: IslandPanelRouterProps) {
  const {
    panelView,
    sessions,
    sessionRequests,
    navigationSeqRef,
    setPanelView,
    applySnapshot,
    hookHealth,
    hookBusy,
    hookInstallError,
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
    selectedAgentRequest,
    busyDecision,
    filteredSessions,
    resolveActive,
    collapseIsland,
    justResolved,
    isExpandedChrome,
    navigateToSession,
    navigateToSubagent,
    navigateToSubagentList,
    handleArchiveSession,
    handleArchiveCompletedSubagents,
    handlePinSession,
    maxSubagentDisplay,
    setMaxSubagentDisplay,
    clipboardHistory,
    clipboardEnabled,
    setClipboardHistory,
    stageClipboardEntry,
    stagedFiles,
    removeStaged,
    clearStaged,
    playStashReaction,
    copyStagedPaths,
    dailyTokens,
    dailyTokensByModel,
    heatmapDisplay,
    pricingRates,
    pricingModels,
    setPricingModels,
    foldedCounterDisplay,
    expandedCounterDisplay,
    settingsBadgeDisplay,
    setFoldedCounterDisplay,
    setExpandedCounterDisplay,
    setSettingsBadgeDisplay,
    setHeatmapDisplay,
    maxCompactIcons,
    maxCompactIconLimit,
    setMaxCompactIcons,
    supportsMicroIsland,
    foldedIslandSize,
    handleChangeFoldedIslandSize,
    compactIndicator,
    setCompactIndicatorState,
    preferredMonitorName,
    handleChangePreferredMonitor,
    mediaCardEnabled,
    handleChangeMediaCardEnabled,
    artworkBackdropEnabled,
    handleChangeArtworkBackdropEnabled,
    lyricsEnabled,
    handleChangeLyricsEnabled,
    clipboardLimit,
    handleChangeClipboardLimit,
    handleChangeClipboardEnabled,
    clipboardAutoStage,
    handleChangeClipboardAutoStage,
    retentionMinutes,
    setRetentionMinutes,
    subagentRetentionMinutes,
    setSubagentRetentionMinutes,
    idleIntervalMin,
    setIdleIntervalMin,
    idleDurationMin,
    setIdleDurationMin,
    approvalNoticeMode,
    handleChangeApprovalNoticeMode,
    globalShortcutView,
    handleChangeGlobalShortcutConfig,
    language,
    handleChangeLanguage,
    handleOpenHooksFromSettings,
    handleOpenTokensFromSettings,
    handleOpenUsageFromSettings,
    openSettingsSubpage,
    settingsTodayLabel,
    usageDisplaySummary,
    hooksSetupSummary,
    hooksNeedAttention,
    hooksNeedSetup,
    hookHealthAnalysis,
    handleOpenHooks,
    launchAtLogin,
    launchAtLoginBusy,
    handleChangeLaunchAtLogin,
  } = props;
  const { t: tSettings } = useTranslation("settings");

  const claudeHookStatus = hookHealth?.claude ?? null;
  const codexHookStatus = hookHealth?.codex ?? null;
  const cursorHookStatus = hookHealth?.cursor ?? null;
  const zcodeHookStatus = hookHealth?.zcode ?? null;
  const geminiHookStatus = hookHealth?.gemini ?? null;
  const opencodeHookStatus = hookHealth?.opencode ?? null;

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
    {
      key: "opencode",
      label: "OpenCode",
      status: opencodeHookStatus,
      note: opencodeHookStatus.settingsPath
        ? i18n.t("register.withPath", {
            ns: "hooks",
            path: opencodeHookStatus.settingsPath,
            note: hookAgentNote("opencode"),
          })
        : i18n.t("register.opencode", {
            ns: "hooks",
            note: hookAgentNote("opencode"),
          }),
      onInstall: handleInstallOpencodeHooks,
      onUninstall: handleUninstallOpencodeHooks,
    },
  ];

  const subviewSession =
    panelView.kind === "session" || panelView.kind === "subagent" || panelView.kind === "subagentList"
      ? sessions.find((session) => session.sessionId === panelView.sessionId)
      : undefined;
  const subviewSubagent =
    panelView.kind === "subagent"
      ? subviewSession?.activeSubagents?.find((sub) => sub.agentId === panelView.agentId)
      : undefined;

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
        onStageEntry={stageClipboardEntry}
      />
    );
  }

  if (panelView.kind === "fileStation") {
    return (
      <FileStationView
        files={stagedFiles}
        onCopy={(id) => {
          copyStagedFilesToClipboard([id]).catch(() => undefined);
        }}
        onCopyPaths={(ids) => {
          copyStagedPaths(ids);
        }}
        onReveal={(path) => {
          revealPath(path).catch(() => undefined);
        }}
        onRemove={removeStaged}
        onClear={clearStaged}
        onDragOut={(ids) => {
          playStashReaction("spit");
          beginStagedFilesDrag(ids).catch(() => undefined);
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
          todayTokensByModel={dailyTokensByModel}
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
          preferredMonitorName={preferredMonitorName}
          onChangePreferredMonitor={handleChangePreferredMonitor}
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
          clipboardAutoStage={clipboardAutoStage}
          onChangeClipboardAutoStage={handleChangeClipboardAutoStage}
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
          idleIntervalMin={idleIntervalMin}
          onChangeIdleInterval={(v) => setIdleIntervalMin(clampIdleInterval(v))}
          idleDurationMin={idleDurationMin}
          onChangeIdleDuration={(v) => setIdleDurationMin(clampIdleDuration(v))}
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

    if (panelView.page === "rules") {
      return <RulesSettingsView />;
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
        onOpenRules={() => openSettingsSubpage("rules")}
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
        onCreateRule={(scope) => resolveActive(selectedAgentRequest, "approved", false, "", scope)}
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
