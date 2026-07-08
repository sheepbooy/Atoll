import {
  CSSProperties,
  FocusEvent,
  KeyboardEvent as ReactKeyboardEvent,
  memo,
  MouseEvent,
  PointerEvent as ReactPointerEvent,
  UIEvent as ReactUIEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";

const IS_WINDOWS =
  typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);

const DECISION_SHORTCUTS = {
  deny: IS_WINDOWS ? "Del" : "⌫",
  approve: IS_WINDOWS ? "Enter" : "↵",
  always: IS_WINDOWS ? "Shift+Enter" : "⇧↵",
} as const;
import {
  Archive,
  ArrowLeft,
  ArrowUpCircle,
  Activity,
  Check,
  CheckCheck,
  ChevronRight,
  ChevronUp,
  CircleCheck,
  Download,
  Ellipsis,
  ExternalLink,
  FolderClosed,
  Hammer,
  HelpCircle,
  Layers,
  Pin,
  PinOff,
  Power,
  RefreshCw,
  Settings2,
  TriangleAlert,
  Trash2,
  X,
} from "lucide-react";
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
  CLAUDE_DESKTOP_HOOK_NOTE,
  CODEX_DESKTOP_HOOK_NOTE,
  CURSOR_HOOK_NOTE,
  deriveHeaderLogoDisplay,
  hookAttentionTitle,
  hookRetrustNote,
  hookStatusIssue,
  isHookReady,
  mergeHookHealthPreferReady,
  type HeaderLogoDisplay,
  type HookAgentKey,
} from "./hookHealth";
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
  finishCollapse,
  finishExpand,
  IDLE_COLLAPSE_DELAY_MS,
  MICRO_SHRINK_DELAY_MS,
  PresentationPhase,
} from "./islandPresentation";
import { AgentMascot, AGENT_ACCENT } from "./AgentMascot";
import type { ClawdMood } from "./ClawdMascot";
import { getSessionColor, getSubagentColor, getSubagentMood } from "./subagentIdentity";
import { AtollLogo, type AtollActivity } from "./AtollLogo";
import { deriveAppLogoState, deriveAtollActivity } from "./logoStates";
import {
  ABSOLUTE_MAX_COMPACT_ICONS,
  COMPACT_HEADER_GAP,
  COMPACT_METRICS_GAP,
  COMPACT_NOTCH_INNER_GAP,
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  computeMaxCompactIconLimit,
  MIN_MAX_COMPACT_ICONS,
} from "./compactLayout";
import { TokenCounter } from "./TokenCounter";
import { TokenHeatmapView } from "./TokenHeatmapView";
import { formatCompactTokenCount } from "./tokenCounterFormat";
import { getDemoMode, isGifCaptureMode, shouldAutoExpandDemo } from "./demoSnapshot";
import { manageAsyncUnlisten } from "./asyncUnlisten";
import { toPng } from "html-to-image";

import {
  getSnapshot,
  normalizeSnapshot,
  getSessionRequests,
  getSessionChat,
  getSessionTranscript,
  IslandSnapshot,
  TokenUsage,
  onIslandHoverChanged,
  onIslandOpenRequested,
  onCaptureCollapseRequested,
  onCaptureOpenHooksRequested,
  onCaptureScreenshotRequested,
  captureProvideScreenshot,
  onSnapshotChanged,
  PermissionRequest,
  SessionSummary,
  ChatMessage,
  HookStatus,
  HookHealthSnapshot,
  EMPTY_HOOK_HEALTH,
  archiveAllResolved,
  archiveSession,
  archiveSubagent,
  archiveCompletedSubagents,
  pinSession,
  SubagentSummary,
  deactivateAtoll,
  quitAtoll,
  resolvePermissionRequest,
  resolvePermissionWithInput,
  setIslandPresentation,
  setCompactLayout,
  usesMicroIsland,
  usesMicroIslandSync,
  setSessionAutoApprove,
  getNotchMetrics,
  NotchMetrics,
  installClaudeHooks,
  uninstallClaudeHooks,
  installCodexHooks,
  uninstallCodexHooks,
  installCursorHooks,
  uninstallCursorHooks,
  getSessionRetention,
  setSessionRetention,
  setSubagentRetention,
  openAgentApp,
  type SessionHost,
  openUrl,
  isAutostartEnabled,
  enableAutostart,
  disableAutostart,
} from "./tauri";

type Decision = "approved" | "denied";
type AgentKind = PermissionRequest["agent"];
type PanelView =
  | { kind: "home" }
  | { kind: "session"; sessionId: string }
  | { kind: "subagent"; sessionId: string; agentId: string }
  | { kind: "subagentList"; sessionId: string }
  | { kind: "settings"; page: "main" | "hooks" | "tokens" };
type FoldedIslandSize = "small" | "regular";

const COMPACT_ICON_SETTING_KEY = "atoll.maxCompactIcons";
const FOLDED_ISLAND_SIZE_SETTING_KEY = "atoll.foldedIslandSize";
const RETENTION_SETTING_KEY = "atoll.sessionRetentionMinutes";
const SUBAGENT_RETENTION_SETTING_KEY = "atoll.subagentRetentionMinutes";
const MAX_SUBAGENT_DISPLAY_SETTING_KEY = "atoll.maxSubagentDisplay";
const DEFAULT_MAX_COMPACT_ICONS = 3;
const DEFAULT_MAX_SUBAGENT_DISPLAY = 3;
const MIN_MAX_SUBAGENT_DISPLAY = 1;
const MAX_MAX_SUBAGENT_DISPLAY = 10;
const DEFAULT_RETENTION_MINUTES = 15;
const DEFAULT_SUBAGENT_RETENTION_MINUTES = 10;
const MIN_RETENTION_MINUTES = 1;
const MAX_RETENTION_MINUTES = 60;
const IDLE_INTERVAL_SETTING_KEY = "atoll.idleIntervalMin";
const IDLE_DURATION_SETTING_KEY = "atoll.idleDurationMin";
const DEFAULT_IDLE_INTERVAL_MIN = 10;
const MIN_IDLE_INTERVAL_MIN = 1;
const MAX_IDLE_INTERVAL_MIN = 60;
const DEFAULT_IDLE_DURATION_MIN = 20;
const MIN_IDLE_DURATION_MIN = 1;
const MAX_IDLE_DURATION_MIN = 60;
const SETTINGS_INITIALIZED_KEY = "atoll.settingsInitialized";
const ZERO_TOKEN_USAGE: TokenUsage = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
};
const EMPTY_NOTCH_METRICS: NotchMetrics = {
  hasNotch: false,
  width: 0,
  height: 0,
};

function clampCompactIconLimit(
  value: number,
  max = ABSOLUTE_MAX_COMPACT_ICONS,
) {
  return Math.min(max, Math.max(MIN_MAX_COMPACT_ICONS, Math.round(value)));
}

function readStoredSetting(
  key: string,
  defaultValue: number,
  clamp: (value: number) => number,
) {
  if (typeof window === "undefined") return defaultValue;
  try {
    const stored = window.localStorage.getItem(key);
    if (stored === null || stored.trim() === "") return defaultValue;
    const raw = Number(stored);
    if (!Number.isFinite(raw)) return defaultValue;
    return clamp(raw);
  } catch {
    return defaultValue;
  }
}

function markSettingsInitialized() {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(SETTINGS_INITIALIZED_KEY, "1");
  } catch {
    // ignore local storage errors
  }
}

function migrateLegacySettings() {
  if (typeof window === "undefined") return;
  try {
    if (window.localStorage.getItem(SETTINGS_INITIALIZED_KEY) === "1") return;

    const stored = {
      icons: window.localStorage.getItem(COMPACT_ICON_SETTING_KEY),
      retention: window.localStorage.getItem(RETENTION_SETTING_KEY),
      interval: window.localStorage.getItem(IDLE_INTERVAL_SETTING_KEY),
      duration: window.localStorage.getItem(IDLE_DURATION_SETTING_KEY),
    };
    const hasAnyStored = Object.values(stored).some(
      (value) => value !== null && value.trim() !== "",
    );

    // First launch with the old bug wrote every slider to its minimum (1).
    if (
      hasAnyStored &&
      stored.icons === "1" &&
      stored.retention === "1" &&
      stored.interval === "1" &&
      stored.duration === "1"
    ) {
      window.localStorage.setItem(
        COMPACT_ICON_SETTING_KEY,
        String(DEFAULT_MAX_COMPACT_ICONS),
      );
      window.localStorage.setItem(
        RETENTION_SETTING_KEY,
        String(DEFAULT_RETENTION_MINUTES),
      );
      window.localStorage.setItem(
        IDLE_INTERVAL_SETTING_KEY,
        String(DEFAULT_IDLE_INTERVAL_MIN),
      );
      window.localStorage.setItem(
        IDLE_DURATION_SETTING_KEY,
        String(DEFAULT_IDLE_DURATION_MIN),
      );
    }

    if (hasAnyStored) {
      markSettingsInitialized();
    }
  } catch {
    // ignore local storage errors
  }
}

if (typeof window !== "undefined") {
  migrateLegacySettings();
}

function readCompactIconLimit() {
  return readStoredSetting(
    COMPACT_ICON_SETTING_KEY,
    DEFAULT_MAX_COMPACT_ICONS,
    (value) => clampCompactIconLimit(value),
  );
}

function readFoldedIslandSize(): FoldedIslandSize {
  if (typeof window === "undefined") return "small";
  try {
    const stored = window.localStorage.getItem(FOLDED_ISLAND_SIZE_SETTING_KEY);
    return stored === "regular" ? "regular" : "small";
  } catch {
    return "small";
  }
}

function clampRetentionMinutes(value: number) {
  return Math.min(
    MAX_RETENTION_MINUTES,
    Math.max(MIN_RETENTION_MINUTES, Math.round(value)),
  );
}

function readRetentionMinutes() {
  return readStoredSetting(
    RETENTION_SETTING_KEY,
    DEFAULT_RETENTION_MINUTES,
    clampRetentionMinutes,
  );
}

function readSubagentRetentionMinutes() {
  return readStoredSetting(
    SUBAGENT_RETENTION_SETTING_KEY,
    DEFAULT_SUBAGENT_RETENTION_MINUTES,
    clampRetentionMinutes,
  );
}

function clampMaxSubagentDisplay(value: number) {
  return Math.min(
    MAX_MAX_SUBAGENT_DISPLAY,
    Math.max(MIN_MAX_SUBAGENT_DISPLAY, Math.round(value)),
  );
}

function readMaxSubagentDisplay() {
  return readStoredSetting(
    MAX_SUBAGENT_DISPLAY_SETTING_KEY,
    DEFAULT_MAX_SUBAGENT_DISPLAY,
    clampMaxSubagentDisplay,
  );
}

function clampIdleInterval(v: number) {
  return Math.min(MAX_IDLE_INTERVAL_MIN, Math.max(MIN_IDLE_INTERVAL_MIN, Math.round(v)));
}
function readIdleInterval() {
  return readStoredSetting(
    IDLE_INTERVAL_SETTING_KEY,
    DEFAULT_IDLE_INTERVAL_MIN,
    clampIdleInterval,
  );
}

function clampIdleDuration(v: number) {
  return Math.min(MAX_IDLE_DURATION_MIN, Math.max(MIN_IDLE_DURATION_MIN, Math.round(v)));
}
function readIdleDuration() {
  return readStoredSetting(
    IDLE_DURATION_SETTING_KEY,
    DEFAULT_IDLE_DURATION_MIN,
    clampIdleDuration,
  );
}

const initialSnapshot: IslandSnapshot = {
  online: false,
  pendingCount: 0,
  archivedCount: 0,
  activeRequest: null,
  recent: [],
  sessions: [],
  dailyTokens: ZERO_TOKEN_USAGE,
  activeSessionTokens: ZERO_TOKEN_USAGE,
  hookHealth: EMPTY_HOOK_HEALTH,
};

const agentLabels: Record<AgentKind, string> = {
  claude: "Claude",
  codex: "Codex",
  cursor: "Cursor",
  gemini: "Gemini",
  other: "Agent",
};

const agentTone: Record<AgentKind, string> = {
  claude: "coral",
  codex: "cyan",
  cursor: "violet",
  gemini: "lime",
  other: "neutral",
};

const agentSortRank: Record<AgentKind, number> = {
  claude: 0,
  codex: 1,
  cursor: 2,
  gemini: 2,
  other: 3,
};
const agentMascotAccent = (agent: AgentKind) => AGENT_ACCENT[agent]?.accent;
const agentMascotDark = (agent: AgentKind) => AGENT_ACCENT[agent]?.accentDark;

type RiskLevel = "danger" | "caution";

const DANGER_PATTERNS: RegExp[] = [
  /\brm\s+(-\w*\s+)*-?\w*[rf]\w*[rf]/i,
  /\bsudo\b/i,
  /git\s+push\b[^\n]*(--force\b|\s-f\b|--force-with-lease\b)/i,
  /git\s+reset\s+--hard\b/i,
  /\bdd\s+if=/i,
  /\bmkfs\b/i,
  /:\(\)\s*\{[^}]*\}\s*;\s*:/,
  /chmod\s+-?\w*\s*777\b/i,
  /(curl|wget)[^|]*\|\s*(sudo\s+)?(ba|z|fi)?sh\b/i,
  />\s*\/dev\/(sd|disk|null|zero)/i,
  /\bkill(all)?\b|\bkill\s+-9\b/i,
  /\b(shutdown|reboot|halt|poweroff)\b/i,
  /\bDROP\s+(TABLE|DATABASE)\b/i,
  /\bTRUNCATE\s+TABLE\b/i,
  /Remove-Item\b[^\n]*(-Recurse|-Force)/i,
  /\bdel\s+\/f\b/i,
  /\bformat\s+[a-z]:/i,
];

const CAUTION_PATTERNS: RegExp[] = [
  /\brm\s+-/i,
  /\bgit\s+clean\b/i,
  /\bgit\s+checkout\s+--\s/i,
  /\b(npm|pnpm|yarn|bun)\s+(install|i|ci|add|remove)\b/i,
  /\b(mv|chmod|chown|ln)\b/i,
  /\bdocker\b[^\n]*\b(rm|rmi|prune|down|stop)\b/i,
  /\b(brew|apt|apt-get|yum|dnf|pacman)\s+(install|remove|uninstall)\b/i,
  /\bpowershell\b[^\n]*(-ExecutionPolicy|-EncodedCommand)/i,
  />>?\s*[^\s|&]/,
];

function assessRisk(command: string): RiskLevel | null {
  if (DANGER_PATTERNS.some((pattern) => pattern.test(command))) return "danger";
  if (CAUTION_PATTERNS.some((pattern) => pattern.test(command))) return "caution";
  return null;
}

const riskLabels: Record<RiskLevel, string> = {
  danger: "High risk",
  caution: "Review",
};

function deriveSessionMood(
  session: SessionSummary,
  activeRequest: PermissionRequest | null,
  justResolved: boolean,
): ClawdMood {
  if (activeRequest && activeRequest.session === session.sessionId) {
    return assessRisk(activeRequest.command) === "danger" ? "worried" : "alert";
  }
  if (session.pendingCount > 0) return "alert";
  if (justResolved) return "happy";
  return "calm";
}

// Keep in sync with COMPACT_WINDOW_HEIGHT in src-tauri/src/lib.rs.
const COMPACT_WINDOW_HEIGHT = 36;
// Keep in sync with MICRO_WINDOW_WIDTH / MICRO_WINDOW_HEIGHT in src-tauri/src/lib.rs.
const MICRO_WINDOW_WIDTH = 96;
const MICRO_WINDOW_HEIGHT = 32;
// Keep in sync with NOTCH_COVER_PADDING in src-tauri/src/lib.rs.
const NOTCH_COVER_PADDING = 16;

// Keep in sync with EXPANDED_IDLE_WINDOW_HEIGHT in src-tauri/src/lib.rs.
const EXPANDED_IDLE_WINDOW_HEIGHT = 240;

function applyWindowMetrics(notch: NotchMetrics) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--compact-height", `${COMPACT_WINDOW_HEIGHT}px`);
  root.style.setProperty("--micro-height", `${MICRO_WINDOW_HEIGHT}px`);
  root.style.setProperty(
    "--expanded-idle-height",
    `${EXPANDED_IDLE_WINDOW_HEIGHT}px`,
  );
  const coverHeight = notch.hasNotch
    ? Math.max(0, notch.height + NOTCH_COVER_PADDING)
    : 0;
  root.style.setProperty("--notch-height", `${coverHeight}px`);
  root.style.setProperty("--notch-width", `${Math.max(0, notch.width)}px`);
  if (notch.leftAreaWidth) {
    root.style.setProperty(
      "--notch-left-area-width",
      `${Math.max(0, notch.leftAreaWidth)}px`,
    );
  } else {
    root.style.removeProperty("--notch-left-area-width");
  }
  root.style.setProperty("--compact-notch-inner-gap", `${COMPACT_NOTCH_INNER_GAP}px`);
  root.style.setProperty(
    "--compact-header-gap",
    `${notch.hasNotch ? 0 : COMPACT_HEADER_GAP}px`,
  );
  root.style.setProperty("--compact-metrics-gap", `${COMPACT_METRICS_GAP}px`);
  root.classList.toggle("has-notch", notch.hasNotch);
}

function compactPresentationKey(
  mode: "micro" | "compact" | "dormant",
  width: number,
  leftWidth: number,
): string {
  if (mode === "micro") return `micro:${width}`;
  return mode === "dormant" ? "dormant" : `compact:${width}:${leftWidth}`;
}

function microPresentationWidth(): number {
  return MICRO_WINDOW_WIDTH;
}

function shouldRestInMicro(usesMicro: boolean): boolean {
  return usesMicro;
}

function shouldUseMicroIsland(
  supportsMicroIsland: boolean,
  foldedIslandSize: FoldedIslandSize,
): boolean {
  return supportsMicroIsland && foldedIslandSize === "small";
}

function resolveCollapsedMode(
  usesMicro: boolean,
  supportsMicroIsland: boolean,
  sessionCount: number,
  pendingCount: number,
  phase: PresentationPhase,
): "micro" | "compact" | "dormant" {
  if (phase === "micro") return "micro";
  if (shouldRestInMicro(usesMicro)) return "compact";
  if (supportsMicroIsland) return "compact";
  if (sessionCount === 0 && pendingCount === 0) return "dormant";
  return "compact";
}

function expandedPresentationKey(idle: boolean): string {
  return `expanded:${idle}`;
}

export function App() {
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
  const shrinkInFlightRef = useRef(false);
  const focusedRef = useRef(false);
  const suppressHoverExpandRef = useRef(false);
  const transitionTimerRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  const frozenCollapseWidthRef = useRef<number | null>(null);
  const frozenCollapseLeftWidthRef = useRef<number | null>(null);
  const suppressPostCollapseSyncRef = useRef(false);
  const holdCompactAfterSubviewOpenRef = useRef(false);
  const expandCollapseAnchorRef = useRef<{
    width: number;
    leftWidth: number;
  } | null>(null);
  const snapshotLoadSeqRef = useRef(0);
  const lastNativePresentationKeyRef = useRef<string | null>(null);
  const [busyDecision, setBusyDecision] = useState<Decision | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const [updateState, setUpdateState] = useState<AppUpdateState>({ status: "idle" });
  const [updateNotice, setUpdateNotice] = useState<string | null>(null);
  const updateCheckInFlightRef = useRef(false);
  const updateNoticeTimerRef = useRef<number | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const busyRef = useRef<Decision | null>(null);
  busyRef.current = busyDecision;
  const menuOpenRef = useRef(false);
  menuOpenRef.current = menuOpen;

  const [panelView, setPanelView] = useState<PanelView>({ kind: "home" });
  const panelViewRef = useRef<PanelView>({ kind: "home" });
  panelViewRef.current = panelView;
  const [sessionRequests, setSessionRequests] = useState<PermissionRequest[]>([]);
  const navigationSeqRef = useRef(0);
  const [hookBusy, setHookBusy] = useState(false);
  const [hookInstallError, setHookInstallError] = useState<string | null>(null);
  const [hooksBackTarget, setHooksBackTarget] = useState<"home" | "settings-main">("home");
  const [tokensBackTarget, setTokensBackTarget] = useState<"home" | "settings-main">("home");
  const [selectedAgent, setSelectedAgent] = useState<AgentKind | null>(null);
  const [notchMetrics, setNotchMetrics] = useState<NotchMetrics>(EMPTY_NOTCH_METRICS);
  const [maxCompactIcons, setMaxCompactIcons] = useState<number>(() => readCompactIconLimit());
  const [retentionMinutes, setRetentionMinutes] = useState<number>(() => readRetentionMinutes());
  const [subagentRetentionMinutes, setSubagentRetentionMinutes] = useState<number>(() => readSubagentRetentionMinutes());
  const [maxSubagentDisplay, setMaxSubagentDisplay] = useState<number>(() => readMaxSubagentDisplay());
  const [idleIntervalSec, setIdleIntervalSec] = useState<number>(() => readIdleInterval());
  const [idleDurationSec, setIdleDurationSec] = useState<number>(() => readIdleDuration());
  const [launchAtLogin, setLaunchAtLogin] = useState(false);
  const [launchAtLoginBusy, setLaunchAtLoginBusy] = useState(false);
  const [justResolved, setJustResolved] = useState(false);
  const [hookHealthHydrated, setHookHealthHydrated] = useState(false);
  const [configuredHookAgents, setConfiguredHookAgents] = useState(() =>
    readConfiguredHookAgents(),
  );
  const prevPendingRef = useRef(0);
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
  const dailyTokens = snapshot.dailyTokens ?? ZERO_TOKEN_USAGE;
  const dailyTokenTotal = dailyTokens.inputTokens + dailyTokens.outputTokens;
  const activeSessionTokens = snapshot.activeSessionTokens ?? ZERO_TOKEN_USAGE;
  const activeSessionTokenTotal =
    activeSessionTokens.inputTokens + activeSessionTokens.outputTokens;
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
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      snapshot.pendingCount,
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

  const filteredSessions = useMemo(() => {
    if (!selectedAgent) return sessions;
    return sessions.filter((session) => session.agent === selectedAgent);
  }, [sessions, selectedAgent]);

  const pendingCountByAgent = useMemo(() => {
    const counts: Record<AgentKind, number> = {
      claude: 0,
      codex: 0,
      cursor: 0,
      gemini: 0,
      other: 0,
    };
    for (const session of sessions) {
      counts[session.agent] += session.pendingCount;
    }
    return counts;
  }, [sessions]);

  const compactHeaderLayout = useMemo(
    () =>
      computeCompactHeaderLayout(
        notchMetrics,
        sessions.length,
        maxCompactIcons,
        activeSessionTokenTotal,
        snapshot.pendingCount,
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      activeSessionTokenTotal,
      snapshot.pendingCount,
    ],
  );

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

  useEffect(() => {
    if (!hookHealthHydrated) return;
    setConfiguredHookAgents(seedConfiguredFromHookHealth(snapshot.hookHealth));
  }, [hookHealthHydrated, snapshot.hookHealth]);

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
      onIslandOpenRequested(() => {
        suppressHoverExpandRef.current = false;
        expandIsland();
        scheduleIdleCollapse();
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
          await setIslandPresentation(
            "expanded",
            collapsedWindowWidthRef.current,
            idleExpanded,
            compactLeftPaneWidthRef.current,
            false,
            true,
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
    if (!updateNotice) {
      return;
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        dismissUpdateNotice();
      }
    }

    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [updateNotice]);

  useEffect(() => {
    return () => {
      if (updateNoticeTimerRef.current !== null) {
        window.clearTimeout(updateNoticeTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const runSilentCheck = async () => {
      if (updateCheckInFlightRef.current) {
        return;
      }
      updateCheckInFlightRef.current = true;
      const result = await checkAppUpdate();
      updateCheckInFlightRef.current = false;
      if (cancelled || result.status === "error") {
        return;
      }
      setUpdateState(result);
    };

    const initialTimer = window.setTimeout(() => {
      if (!cancelled) {
        void runSilentCheck();
      }
    }, UPDATE_INITIAL_DELAY_MS);

    const intervalId = window.setInterval(() => {
      if (!cancelled) {
        void runSilentCheck();
      }
    }, UPDATE_RECHECK_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(initialTimer);
      window.clearInterval(intervalId);
    };
  }, []);

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
      return;
    }
    if (selectedAgent && tabAgents.includes(selectedAgent)) {
      return;
    }
    setSelectedAgent(activeRequest?.agent ?? tabAgents[0]);
  }, [tabAgents, selectedAgent, activeRequest?.agent]);

  useEffect(() => {
    setMaxCompactIcons((current) =>
      clampCompactIconLimit(current, maxCompactIconLimit),
    );
  }, [maxCompactIconLimit]);

  useEffect(() => {
    try {
      window.localStorage.setItem(
        COMPACT_ICON_SETTING_KEY,
        String(maxCompactIcons),
      );
    } catch {
      // ignore local storage errors
    }
  }, [maxCompactIcons]);

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

  useEffect(() => {
    try {
      window.localStorage.setItem(
        RETENTION_SETTING_KEY,
        String(retentionMinutes),
      );
    } catch {
      // ignore local storage errors
    }
    setSessionRetention(retentionMinutes).catch(() => undefined);
  }, [retentionMinutes]);

  useEffect(() => {
    try {
      window.localStorage.setItem(SUBAGENT_RETENTION_SETTING_KEY, String(subagentRetentionMinutes));
    } catch {}
    setSubagentRetention(subagentRetentionMinutes).catch(() => undefined);
  }, [subagentRetentionMinutes]);

  useEffect(() => {
    try {
      window.localStorage.setItem(MAX_SUBAGENT_DISPLAY_SETTING_KEY, String(maxSubagentDisplay));
    } catch {
      // ignore local storage errors
    }
  }, [maxSubagentDisplay]);

  useEffect(() => {
    try { window.localStorage.setItem(IDLE_INTERVAL_SETTING_KEY, String(idleIntervalSec)); } catch {}
  }, [idleIntervalSec]);

  useEffect(() => {
    try { window.localStorage.setItem(IDLE_DURATION_SETTING_KEY, String(idleDurationSec)); } catch {}
  }, [idleDurationSec]);

  useEffect(() => {
    isAutostartEnabled()
      .then(setLaunchAtLogin)
      .catch(() => undefined);
  }, []);

  function handleChangeFoldedIslandSize(small: boolean) {
    const nextSize: FoldedIslandSize = small ? "small" : "regular";
    foldedIslandSizeRef.current = nextSize;
    usesMicroIslandRef.current = shouldUseMicroIsland(
      supportsMicroIslandRef.current,
      nextSize,
    );
    setFoldedIslandSize(nextSize);
  }

  async function handleChangeLaunchAtLogin(enabled: boolean) {
    if (launchAtLoginBusy) {
      return;
    }

    const previous = launchAtLogin;
    setLaunchAtLogin(enabled);
    setLaunchAtLoginBusy(true);
    try {
      if (enabled) {
        await enableAutostart();
      } else {
        await disableAutostart();
      }
    } catch (error) {
      setLaunchAtLogin(previous);
      console.error("[Atoll] autostart toggle failed", error);
    } finally {
      setLaunchAtLoginBusy(false);
    }
  }

  useEffect(() => {
    markSettingsInitialized();
  }, []);

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
      if (alwaysApprove) {
        await setSessionAutoApprove(request.session, true);
      }
      const nextSnapshot = await resolvePermissionRequest(request.id, decision, note);
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
    setPanelView({ kind: "subagent", sessionId, agentId });
  }

  function navigateToSubagentList(sessionId: string) {
    setPanelView({ kind: "subagentList", sessionId });
  }

  function navigateBack() {
    ++navigationSeqRef.current;
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
    const microWidth = microPresentationWidth();
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

  async function expandIsland() {
    clearIdleTimer();
    holdCompactAfterSubviewOpenRef.current = false;

    const next = beginExpand(phaseRef.current);
    if (next === phaseRef.current) return;
    clearTransitionWork();
    expandCollapseAnchorRef.current = {
      width: collapsedWindowWidthRef.current,
      leftWidth: compactLeftPaneWidthRef.current,
    };

    const idleExpanded =
      snapshotRef.current.pendingCount === 0 &&
      snapshotRef.current.sessions.length === 0;
    lastNativePresentationKeyRef.current = expandedPresentationKey(idleExpanded);
    setPresentationPhase(next);
    const nativeTransition = setIslandPresentation(
      "expanded",
      collapsedWindowWidthRef.current,
      idleExpanded,
      compactLeftPaneWidthRef.current,
    );
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      if (phaseRef.current !== "opening") return;

      try {
        await nativeTransition;
        if (phaseRef.current === "opening") {
          setPresentationPhase(finishExpand("opening"));
        }
      } catch {
        setPresentationPhase(usesMicroIslandRef.current ? "micro" : "compact");
      }
    }, COLLAPSE_ANIMATION_MS);
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
    setPanelView({ kind: "home" });

    const compactWidth = collapseCompactWidth();
    const compactLeftWidth = collapseCompactLeftWidth();
    const naturalCollapseMode = collapsePresentationMode();
    const wasSessionSubview =
      leavingPanel === "session" || leavingPanel === "subagent" || leavingPanel === "subagentList";
    const collapseMode =
      wasSessionSubview &&
      (naturalCollapseMode === "dormant" || naturalCollapseMode === "micro")
        ? "compact"
        : naturalCollapseMode;
    const collapsePresentationWidth =
      collapseMode === "micro" ? microPresentationWidth() : compactWidth;

    lastNativePresentationKeyRef.current = compactPresentationKey(
      collapseMode,
      collapsePresentationWidth,
      compactLeftWidth,
    );

    const nativeTransition =
      collapseMode === "micro"
        ? setIslandPresentation("micro", microPresentationWidth())
        : collapseMode === "dormant"
          ? setIslandPresentation("dormant")
          : setIslandPresentation(
              "compact",
              compactWidth,
              undefined,
              compactLeftWidth,
            );
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      if (phaseRef.current !== "closing") return;

      try {
        await nativeTransition;
        if (phaseRef.current === "closing") {
          if (collapseMode === "micro") {
            await setIslandPresentation(
              "micro",
              microPresentationWidth(),
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
          if (wasSessionSubview) {
            suppressPostCollapseSyncRef.current = true;
            holdCompactAfterSubviewOpenRef.current = true;
          }
          setPresentationPhase(collapseMode === "micro" ? "micro" : "compact");
        }
      } catch {
        releaseFrozenCollapseMetrics();
        setPresentationPhase("expanded");
      } finally {
        releaseFrozenCollapseMetrics();
        suppressHoverExpandRef.current = false;
      }
    }, COLLAPSE_ANIMATION_MS);
  }

  function scheduleIdleCollapse() {
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
    scheduleIdleCollapse();
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

  function applyHookInstallSnapshot(statuses: Partial<Record<"claude" | "codex" | "cursor", HookStatus>>) {
    invalidatePendingSnapshotLoads();
    const installedHealth: HookHealthSnapshot = {
      claude: statuses.claude ?? snapshotRef.current.hookHealth.claude,
      codex: statuses.codex ?? snapshotRef.current.hookHealth.codex,
      cursor: statuses.cursor ?? snapshotRef.current.hookHealth.cursor,
    };
    const optimisticHookHealth = mergeHookHealthPreferReady(
      snapshotRef.current.hookHealth,
      installedHealth,
    );
    applySnapshot({
      ...snapshotRef.current,
      hookHealth: optimisticHookHealth,
      online: true,
    });
    setHookHealthHydrated(true);
    return getSnapshot()
      .catch(() => null)
      .then((nextSnapshot) => {
        if (!nextSnapshot) return;
        applySnapshot({
          ...nextSnapshot,
          hookHealth: mergeHookHealthPreferReady(
            nextSnapshot.hookHealth,
            installedHealth,
          ),
          online: nextSnapshot.online || true,
        });
      });
  }

  async function handleInstallClaudeHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installClaudeHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("claude"));
      }
      await applyHookInstallSnapshot({ claude: status });
      if (status.installed) {
        collapseIsland(true);
      }
    } catch (error) {
      setHookInstallError(formatHookInstallError("Claude Code", error));
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallCodexHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installCodexHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("codex"));
      }
      await applyHookInstallSnapshot({ codex: status });
      if (status.installed) {
        setHookInstallError(null);
      } else {
        setHookInstallError("Codex hooks were not saved. Check permissions on ~/.codex/hooks.json.");
      }
    } catch (error) {
      setHookInstallError(formatHookInstallError("Codex", error));
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallAllHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      setConfiguredHookAgents(markAllHookAgentsConfigured());
      const [claudeStatus, codexStatus, cursorStatus] = await Promise.all([
        installClaudeHooks(),
        installCodexHooks(),
        installCursorHooks(),
      ]);
      await applyHookInstallSnapshot({
        claude: claudeStatus,
        codex: codexStatus,
        cursor: cursorStatus,
      });
      if (claudeStatus.installed || codexStatus.installed || cursorStatus.installed) {
        collapseIsland(true);
      }
      const failures = [
        !claudeStatus.installed ? "Claude Code" : null,
        !codexStatus.installed ? "Codex" : null,
        !cursorStatus.installed ? "Cursor" : null,
      ].filter(Boolean);
      if (failures.length > 0) {
        setHookInstallError(`Could not install hooks for: ${failures.join(", ")}.`);
      }
    } catch (error) {
      setHookInstallError(formatHookInstallError("Agent hooks", error));
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallClaudeHooks() {
    setMenuOpen(false);
    setHookBusy(true);
    try {
      const status = await uninstallClaudeHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            claude: status,
          },
        });
      }
    } catch {
      // keep previous status
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallCodexHooks() {
    setMenuOpen(false);
    setHookBusy(true);
    try {
      const status = await uninstallCodexHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            codex: status,
          },
        });
      }
    } catch {
      // keep previous status
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallHooks() {
    setMenuOpen(false);
    setHookBusy(true);
    try {
      const [claudeStatus, codexStatus, cursorStatus] = await Promise.all([
        uninstallClaudeHooks(),
        uninstallCodexHooks(),
        uninstallCursorHooks(),
      ]);
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            claude: claudeStatus,
            codex: codexStatus,
            cursor: cursorStatus,
          },
        });
      }
    } catch {
      // keep previous status
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallCursorHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installCursorHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("cursor"));
      }
      await applyHookInstallSnapshot({ cursor: status });
      if (status.installed) {
        setHookInstallError(null);
      } else {
        setHookInstallError("Cursor hooks were not saved. Check permissions on ~/.cursor/hooks.json.");
      }
    } catch (error) {
      setHookInstallError(formatHookInstallError("Cursor", error));
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallCursorHooks() {
    setMenuOpen(false);
    setHookBusy(true);
    try {
      const status = await uninstallCursorHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            cursor: status,
          },
        });
      }
    } catch {
      // keep previous status
    } finally {
      setHookBusy(false);
    }
  }

  const hookMenuAgents: HookMenuAgent[] = [
    {
      key: "claude",
      label: "Claude Code",
      status: claudeHookStatus,
      note: claudeHookStatus.settingsPath
        ? `Registers hooks in ${claudeHookStatus.settingsPath}. ${CLAUDE_DESKTOP_HOOK_NOTE}`
        : `Registers Claude Code hooks for permission approval. ${CLAUDE_DESKTOP_HOOK_NOTE}`,
      onInstall: handleInstallClaudeHooks,
      onUninstall: handleUninstallClaudeHooks,
    },
    {
      key: "codex",
      label: "Codex",
      status: codexHookStatus,
      note: codexHookStatus.settingsPath
        ? `Registers hooks in ${codexHookStatus.settingsPath}. ${CODEX_DESKTOP_HOOK_NOTE}`
        : `Registers Codex hooks for permission approval. ${CODEX_DESKTOP_HOOK_NOTE}`,
      onInstall: handleInstallCodexHooks,
      onUninstall: handleUninstallCodexHooks,
    },
    {
      key: "cursor",
      label: "Cursor",
      status: cursorHookStatus,
      note: cursorHookStatus.settingsPath
        ? `Registers hooks in ${cursorHookStatus.settingsPath}. ${CURSOR_HOOK_NOTE}`
        : `Registers Cursor hooks for permission approval. ${CURSOR_HOOK_NOTE}`,
      onInstall: handleInstallCursorHooks,
      onUninstall: handleUninstallCursorHooks,
    },
  ];

  const hooksNeedSetup =
    hookHealthHydrated && hookHealthAnalysis.needsFirstTimeSetup;
  const hooksNeedAttention =
    hookHealthHydrated &&
    (hookHealthAnalysis.needsFirstTimeSetup || hookHealthAnalysis.needsReconnect);
  const hooksSetupSummary = hookHealthAnalysis.summary;

  const updateAvailable = updateState.status === "available";
  const updateVersion =
    updateState.status === "available" || updateState.status === "downloading"
      ? updateState.version
      : null;
  const updateDownloading = updateState.status === "downloading";
  const updateDownloadProgress =
    updateState.status === "downloading" ? updateState.progress : 0;
  const updateChecking = updateState.status === "checking";

  function dismissUpdateNotice() {
    setUpdateNotice(null);
    if (updateNoticeTimerRef.current !== null) {
      window.clearTimeout(updateNoticeTimerRef.current);
      updateNoticeTimerRef.current = null;
    }
  }

  function showUpdateNotice(version: string) {
    dismissUpdateNotice();
    setUpdateNotice(version);
    updateNoticeTimerRef.current = window.setTimeout(() => {
      dismissUpdateNotice();
    }, 5000);
  }

  async function runUpdateCheck() {
    if (updateCheckInFlightRef.current || updateDownloading) {
      return;
    }
    updateCheckInFlightRef.current = true;
    setUpdateState({ status: "checking" });
    const result = await checkAppUpdate();
    updateCheckInFlightRef.current = false;
    if (result.status === "error") {
      setUpdateState({ status: "idle" });
      return;
    }
    setUpdateState(result);
  }

  async function handleCheckForUpdates() {
    setMenuOpen(false);
    if (updateCheckInFlightRef.current || updateDownloading) {
      return;
    }
    updateCheckInFlightRef.current = true;
    setUpdateState({ status: "checking" });
    const result = await checkAppUpdate();
    updateCheckInFlightRef.current = false;
    if (result.status === "error") {
      setUpdateState({ status: "idle" });
      return;
    }
    setUpdateState(result);
    if (result.status === "idle") {
      const version = (await getAppVersion()) ?? "0.0.0";
      showUpdateNotice(version);
    }
  }

  async function handleInstallUpdate() {
    if (!updateAvailable || !updateVersion) {
      return;
    }
    setMenuOpen(false);
    setUpdateState({ status: "downloading", version: updateVersion, progress: 0 });
    try {
      await installAppUpdate((progress) => {
        setUpdateState({ status: "downloading", version: updateVersion, progress });
      });
    } catch {
      setUpdateState({ status: "idle" });
    }
  }

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
    setPanelView({ kind: "settings", page: "tokens" });
  }

  function handleOpenTokensFromCounter() {
    if (panelView.kind === "settings" && panelView.page === "tokens") return;
    if (isIdleExpanded) {
      lastNativePresentationKeyRef.current = expandedPresentationKey(false);
      syncNativeIslandPresentation("expanded", undefined, false).catch(
        () => undefined,
      );
    }
    openTokensPage(panelView.kind === "settings" ? "settings-main" : "home");
  }

  function handleOpenTokensFromSettings() {
    openTokensPage("settings-main");
  }

  function navigateBackFromTokens() {
    if (tokensBackTarget === "settings-main") {
      setPanelView({ kind: "settings", page: "main" });
    } else {
      navigateBack();
    }
  }

  function handleOpenSettings() {
    setMenuOpen(false);
    setPanelView({ kind: "settings", page: "main" });
  }

  function openHooksPage(backTarget: "home" | "settings-main") {
    setMenuOpen(false);
    setHooksBackTarget(backTarget);
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
    isMicro && !isPresentationTransition && sessions.length > 0;
  const showCompactTokenCounter = sessions.length > 0;
  const showExpandedTokenCounter = true;
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
  const isSubview = isExpandedChrome && panelView.kind !== "home";
  const menuBarLogoSize = isExpanded ? 36 : isMicro ? 30 : 34;
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
      const microWidth = microPresentationWidth();
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
      const key = expandedPresentationKey(isIdleExpanded);
      if (lastNativePresentationKeyRef.current === key) return;
      lastNativePresentationKeyRef.current = key;
      syncNativeIslandPresentation("expanded", undefined, isIdleExpanded).catch(
        () => undefined,
      );
    }
  }, [
    phase,
    collapsedWindowWidth,
    compactLeftPaneWidth,
    collapsedMode,
    isIdleExpanded,
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
        return <TokenHeatmapView todayTokens={dailyTokens} />;
      }

      return (
        <SettingsView
          maxCompactIcons={maxCompactIcons}
          maxCompactIconLimit={maxCompactIconLimit}
          onChangeMaxCompactIcons={(nextValue) =>
            setMaxCompactIcons(clampCompactIconLimit(nextValue, maxCompactIconLimit))
          }
          showFoldedIslandSizeSetting={supportsMicroIsland}
          foldedIslandSize={foldedIslandSize}
          onChangeFoldedIslandSize={handleChangeFoldedIslandSize}
          retentionMinutes={retentionMinutes}
          onChangeRetentionMinutes={(nextValue) =>
            setRetentionMinutes(clampRetentionMinutes(nextValue))
          }
          subagentRetentionMinutes={subagentRetentionMinutes}
          onChangeSubagentRetentionMinutes={(nextValue) =>
            setSubagentRetentionMinutes(clampRetentionMinutes(nextValue))
          }
          maxSubagentDisplay={maxSubagentDisplay}
          onChangeMaxSubagentDisplay={(nextValue) =>
            setMaxSubagentDisplay(clampMaxSubagentDisplay(nextValue))
          }
          idleIntervalSec={idleIntervalSec}
          onChangeIdleInterval={(v) => setIdleIntervalSec(clampIdleInterval(v))}
          idleDurationSec={idleDurationSec}
          onChangeIdleDuration={(v) => setIdleDurationSec(clampIdleDuration(v))}
          launchAtLogin={launchAtLogin}
          launchAtLoginBusy={launchAtLoginBusy}
          onChangeLaunchAtLogin={handleChangeLaunchAtLogin}
          onOpenHooks={handleOpenHooksFromSettings}
          onOpenTokens={handleOpenTokensFromSettings}
          todayTokenTotal={dailyTokenTotal}
          hooksSummary={hooksSetupSummary}
          hooksNeedAttention={hooksNeedAttention}
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
        className={`island is-${phase} ${isExpanded ? "is-expanded" : ""} ${isIdleExpanded ? "is-idle" : ""} ${isMicro ? "is-micro" : ""} ${isDormant ? "is-dormant" : ""} ${snapshot.pendingCount > 0 ? "has-pending" : ""} ${isExpandedChrome && panelView.kind !== "home" ? "is-subview" : ""} ${panelView.kind === "session" || panelView.kind === "subagent" || panelView.kind === "subagentList" ? "is-session-subview" : ""}`}
        aria-label="Atoll"
        tabIndex={0}
        onClick={handleIslandClick}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onFocusCapture={handleIslandFocus}
        onBlurCapture={handleIslandBlur}
      >
        <header
          className="island-header"
          onMouseDown={startWindowDrag}
          title={isExpanded ? "Drag window" : "Hover to open"}
        >
          <div
            className={`header-main ${showPanelAgentTabs ? "has-agent-tabs" : ""}${isSubview ? " has-subview-nav" : ""}`}
          >
            <span className="atoll-indicator-wrap">
              <span
                className={`atoll-indicator is-app-${appLogoState} ${snapshot.online ? "is-online" : "is-offline"}${hooksNeedAttention ? " is-hook-attention" : ""}`}
                title={
                  updateAvailable
                    ? `Update available: v${updateVersion}`
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
                    display={headerLogo}
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
                  title={snapshot.online ? "Listening" : "Offline"}
                />
                <CompactSessionStack
                  sessions={compactLeftSessions}
                  overflowCount={compactLeftOverflow}
                  activeRequest={activeRequest}
                  justResolved={justResolved}
                />
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
            ) : panelView.kind === "settings" && panelView.page === "hooks" ? (
              <HooksSubviewNav
                onBack={navigateBackFromHooks}
                backLabel={hooksBackTarget === "settings-main" ? "Settings" : "Back"}
              />
            ) : panelView.kind === "settings" && panelView.page === "tokens" ? (
              <TokensSubviewNav
                onBack={navigateBackFromTokens}
                backLabel={tokensBackTarget === "settings-main" ? "Settings" : "Back"}
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
                  value={activeSessionTokenTotal}
                  usage={activeSessionTokens}
                  variant={isMicro ? "micro" : "compact"}
                  suppressAnimations={isPresentationTransition}
                  sessionCount={sessions.length}
                  maxCompactIcons={maxCompactIcons}
                  compactTokenLevel={compactHeaderLayout.tokenCompactLevel}
                />
              ) : null}
              {showCompactHeaderMetrics && snapshot.pendingCount > 0 ? (
                <span className="pending-badge-slot">
                  <span
                    className="pending-badge"
                    aria-label={`${snapshot.pendingCount} pending`}
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
                value={dailyTokenTotal}
                usage={dailyTokens}
                variant="expanded"
                onClick={handleOpenTokensFromCounter}
              />
            ) : null}
            <button
              className="icon-button"
              type="button"
              onClick={() => collapseIsland(true)}
              aria-label="Collapse Atoll"
              tabIndex={isExpandedChrome ? 0 : -1}
            >
              <ChevronUp size={16} />
            </button>
            <button
              className={`icon-button${updateAvailable ? " has-update" : ""}`}
              type="button"
              onClick={() => setMenuOpen((open) => !open)}
              aria-label="More options"
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
                  Agent hooks
                </button>
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleArchiveAll}
                >
                  <Archive size={14} />
                  Archive all
                </button>
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleOpenSettings}
                >
                  <Settings2 size={14} />
                  Settings
                </button>
                {updateDownloading ? (
                  <button type="button" role="menuitem" disabled>
                    <RefreshCw size={14} />
                    Downloading… {Math.round(updateDownloadProgress * 100)}%
                  </button>
                ) : updateAvailable ? (
                  <button
                    type="button"
                    role="menuitem"
                    className="accent"
                    onClick={handleInstallUpdate}
                  >
                    <ArrowUpCircle size={14} />
                    Update to v{updateVersion}
                  </button>
                ) : (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={handleCheckForUpdates}
                    disabled={updateChecking}
                  >
                    <RefreshCw size={14} />
                    {updateChecking ? "Checking…" : "Check for updates"}
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
                  Quit Atoll
                </button>
              </div>
            ) : null}
          </div>
          ) : null}

        </header>

        {!isPresentationTransition ? (
          <div className="island-panel">{renderPanel()}</div>
        ) : null}
        {updateNotice ? (
          <UpdateNotice version={updateNotice} onDismiss={dismissUpdateNotice} />
        ) : null}
      </section>
    </main>
  );
}

function UpdateNotice({
  version,
  onDismiss,
}: {
  version: string;
  onDismiss: () => void;
}) {
  return (
    <div
      className="update-notice-layer"
      data-no-drag
      onMouseDown={(event) => event.stopPropagation()}
      onClick={onDismiss}
    >
      <div
        className="update-notice-card"
        role="alertdialog"
        aria-labelledby="update-notice-title"
        aria-describedby="update-notice-desc"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="update-notice-icon-wrap" aria-hidden="true">
          <CircleCheck size={28} strokeWidth={1.75} />
        </div>
        <p id="update-notice-title" className="update-notice-title">
          You're up to date
        </p>
        <p id="update-notice-desc" className="update-notice-desc">
          Atoll <span className="update-notice-version">v{version}</span> is the latest version.
        </p>
        <button type="button" className="update-notice-button" onClick={onDismiss}>
          OK
        </button>
      </div>
    </div>
  );
}

/* ─── Compact Header ───────────────────────────────────────────── */

interface CompactSessionStackProps {
  sessions: SessionSummary[];
  overflowCount?: number;
  placement?: "left" | "right";
  activeRequest: PermissionRequest | null;
  justResolved: boolean;
}

function CompactSessionStack({
  sessions,
  overflowCount = 0,
  placement = "left",
  activeRequest,
  justResolved,
}: CompactSessionStackProps) {
  if (sessions.length === 0 && overflowCount === 0) {
    return null;
  }

  return (
    <span
      className={`compact-session-stack ${
        placement === "right" ? "compact-session-stack--right" : ""
      }`}
      aria-hidden="true"
    >
      {sessions.map((session) => {
        const sessionColor = getSessionColor(session.sessionId);
        return (
          <span
            key={session.sessionId}
            className={`compact-session-dot ${sessionColor.tone} ${
              session.pendingCount > 0 ? "has-pending" : ""
            }`}
            title={`${agentLabels[session.agent]} · ${sessionDisplayName(
              session.cwd,
            )}`}
          >
            <AgentMascot
              agent={session.agent}
              mood={deriveSessionMood(session, activeRequest, justResolved)}
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={session.agent === "cursor" ? 20 : 18}
            />
          </span>
        );
      })}
      {overflowCount > 0 ? (
        <span className="compact-session-overflow">+{overflowCount}</span>
      ) : null}
    </span>
  );
}

interface AgentTabBarProps {
  agents: AgentKind[];
  selectedAgent: AgentKind | null;
  pendingCountByAgent: Record<AgentKind, number>;
  showTabs: boolean;
  compact?: boolean;
  online: boolean;
  onSelectAgent: (agent: AgentKind) => void;
}

function AgentTabBar({
  agents,
  selectedAgent,
  pendingCountByAgent,
  showTabs,
  compact = false,
  online,
  onSelectAgent,
}: AgentTabBarProps) {
  if (agents.length === 0) {
    return (
      <span className="agent-tabs-empty">
        {online ? "Listening for agents" : "Offline"}
      </span>
    );
  }

  const active = selectedAgent ?? agents[0];
  if (!showTabs) {
    const pending = pendingCountByAgent[active] ?? 0;
    const mood: ClawdMood = pending > 0 ? "alert" : "calm";
    return (
      <span className={`agent-tab is-static ${agentTone[active]}${compact ? " is-compact" : ""}`} data-no-drag>
        <AgentMascot
          agent={active}
          mood={mood}
          accent={agentMascotAccent(active)}
          accentDark={agentMascotDark(active)}
          size={compact ? 14 : 16}
        />
        {!compact ? <span>{agentLabels[active]}</span> : null}
        {pending > 0 ? <span className="agent-tab-pending">{pending}</span> : null}
      </span>
    );
  }

  return (
    <div className={`agent-tabbar${compact ? " is-compact" : ""}`} data-no-drag>
      {agents.map((agent) => {
        const pending = pendingCountByAgent[agent] ?? 0;
        const isActive = agent === active;
        const mood: ClawdMood = pending > 0 ? "alert" : "calm";
        return (
          <button
            key={agent}
            type="button"
            className={`agent-tab ${isActive ? "is-active" : ""} ${agentTone[agent]}${compact ? " is-compact" : ""}`}
            onClick={() => onSelectAgent(agent)}
            aria-label={agentLabels[agent]}
            title={agentLabels[agent]}
          >
            <AgentMascot
              agent={agent}
              mood={mood}
              accent={agentMascotAccent(agent)}
              accentDark={agentMascotDark(agent)}
              size={compact ? 14 : 16}
            />
            {!compact ? <span>{agentLabels[agent]}</span> : null}
            {pending > 0 ? <span className="agent-tab-pending">{pending}</span> : null}
          </button>
        );
      })}
    </div>
  );
}

/* ─── Session List View ───────────────────────────────────────── */

function sessionIdAtClientPoint(
  x: number,
  y: number,
  listEl: HTMLElement | null,
): string | null {
  if (!listEl) return null;
  for (const item of listEl.querySelectorAll<HTMLElement>("[data-session-id]")) {
    const rect = item.getBoundingClientRect();
    if (x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom) {
      return item.dataset.sessionId ?? null;
    }
  }
  return null;
}

interface SessionListViewProps {
  sessions: SessionSummary[];
  activeRequest: PermissionRequest | null;
  justResolved: boolean;
  isExpanded: boolean;
  maxSubagentDisplay: number;
  onSelectSession: (sessionId: string) => void;
  onSelectSubagent: (sessionId: string, agentId: string) => void;
  onArchiveSession: (sessionId: string) => void;
  onArchiveCompletedSubagents: (sessionId: string) => void;
  onPinSession: (sessionId: string, pinned: boolean) => void;
  onViewSubagentList: (sessionId: string) => void;
}

function partitionSubagents(subagents: SubagentSummary[], limit: number) {
  const sorted = [...subagents].sort((a, b) => {
    const aDone = Boolean(a.completedAt);
    const bDone = Boolean(b.completedAt);
    if (aDone !== bDone) {
      return aDone ? 1 : -1;
    }
    return a.startedAt.localeCompare(b.startedAt);
  });
  const visible = sorted.slice(0, limit);
  const overflowCount = sorted.length - visible.length;
  return { visible, overflowCount, hidden: sorted.slice(limit) };
}

function SessionListView({
  sessions,
  activeRequest,
  justResolved,
  isExpanded,
  maxSubagentDisplay,
  onSelectSession,
  onSelectSubagent,
  onArchiveSession,
  onArchiveCompletedSubagents,
  onPinSession,
  onViewSubagentList,
}: SessionListViewProps) {
  const [hoveredSessionId, setHoveredSessionId] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isExpanded) {
      setHoveredSessionId(null);
      return;
    }

    const unsubscribe = manageAsyncUnlisten(
      onIslandHoverChanged(({ hovering, clientX, clientY }) => {
        if (!hovering || clientX == null || clientY == null) {
          if (!hovering) {
            setHoveredSessionId(null);
          }
          return;
        }
        setHoveredSessionId(sessionIdAtClientPoint(clientX, clientY, listRef.current));
      }),
    );

    return () => {
      unsubscribe();
    };
  }, [isExpanded, sessions.length]);

  function handleListPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const item = (event.target as HTMLElement).closest<HTMLElement>("[data-session-id]");
    setHoveredSessionId(item?.dataset.sessionId ?? null);
  }

  function handleSessionMainKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>,
    sessionId: string,
  ) {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onSelectSession(sessionId);
  }

  return (
    <div className="session-list-view">
      <div className="session-list-header">
        <Layers size={12} />
        <span>{sessions.length} session{sessions.length > 1 ? "s" : ""}</span>
      </div>
      <div
        ref={listRef}
        className="session-list"
        onPointerMove={handleListPointerMove}
        onPointerLeave={() => setHoveredSessionId(null)}
      >
        {sessions.map((session) => {
          const sessionColor = getSessionColor(session.sessionId);
          const isHovered = hoveredSessionId === session.sessionId;
          return (
            <div
              key={session.sessionId}
              data-session-id={session.sessionId}
              className={`session-item ${session.pinned ? "is-pinned" : ""} ${isHovered ? "is-hovered" : ""}`}
            >
              <div
                className="session-item-main"
                role="button"
                tabIndex={0}
                onClick={() => onSelectSession(session.sessionId)}
                onKeyDown={(event) => handleSessionMainKeyDown(event, session.sessionId)}
              >
                <div className="session-item-left">
                  <span className="session-clawd">
                    <AgentMascot
                      agent={session.agent}
                      mood={deriveSessionMood(session, activeRequest, justResolved)}
                      accent={sessionColor.accent}
                      accentDark={sessionColor.accentDark}
                    />
                  </span>
                  <div className="session-item-info">
                    <span className="session-item-name">
                      {session.pinned ? <Pin size={10} className="pin-indicator" /> : null}
                      {sessionDisplayName(session.cwd)}
                    </span>
                    <span className="session-item-meta">
                      {session.cwd}
                      <span className="meta-divider">·</span>
                      <span className={`session-agent-pill ${sessionColor.tone}`}>
                        {agentLabels[session.agent]}
                      </span>
                      <span className="meta-divider">·</span>
                      {timeAgo(session.lastActivity)}
                    </span>
                    {session.activeSubagents && session.activeSubagents.length > 0 ? (
                      <div className="session-subagents">
                        {(() => {
                          const { visible, overflowCount, hidden } = partitionSubagents(
                            session.activeSubagents,
                            maxSubagentDisplay,
                          );
                          const hasCompleted = session.activeSubagents.some((sub) => Boolean(sub.completedAt));
                          return (
                            <>
                              <div className="session-subagents-chips">
                                {visible.map((sub) => {
                                  const subagentColor = getSubagentColor(sub.agentId);
                                  const subagentMood = getSubagentMood(sub.agentId, Boolean(sub.completedAt));
                                  return (
                                    <button
                                      key={sub.agentId}
                                      className={`subagent-chip ${subagentColor.tone} ${sub.completedAt ? "is-completed" : ""}`}
                                      type="button"
                                      title={sub.completedAt ? `${sub.agentType} (done)` : sub.agentType}
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        onSelectSubagent(session.sessionId, sub.agentId);
                                      }}
                                    >
                                      <AgentMascot
                                        agent={session.agent}
                                        size={14}
                                        mood={subagentMood}
                                        accent={subagentColor.accent}
                                        accentDark={subagentColor.accentDark}
                                      />
                                      <span className="subagent-chip-label">{sub.agentType}</span>
                                      {sub.completedAt ? <Check size={10} /> : null}
                                    </button>
                                  );
                                })}
                                {overflowCount > 0 ? (
                                  <span
                                    className="subagent-chip-overflow"
                                    title={hidden.map((sub) => sub.agentType).join(", ")}
                                  >
                                    +{overflowCount}
                                  </span>
                                ) : null}
                              </div>
                              <div className="session-subagents-actions">
                                {session.activeSubagents.length >= 2 ? (
                                  <button
                                    type="button"
                                    className="subagent-view-all-btn"
                                    title="View all subagents"
                                    onClick={(e) => {
                                      e.stopPropagation();
                                      onViewSubagentList(session.sessionId);
                                    }}
                                  >
                                    <Layers size={12} />
                                  </button>
                                ) : null}
                                <button
                                  type="button"
                                  className="subagent-bulk-archive-btn"
                                  title="Archive completed subagents"
                                  disabled={!hasCompleted}
                                  onClick={(e) => {
                                    e.stopPropagation();
                                    onArchiveCompletedSubagents(session.sessionId);
                                  }}
                                >
                                  <Archive size={12} />
                                </button>
                              </div>
                            </>
                          );
                        })()}
                      </div>
                    ) : null}
                  </div>
                </div>
                <div className="session-item-trail">
                  {session.pendingCount > 0 ? (
                    <span className="session-pending-badge">{session.pendingCount}</span>
                  ) : null}
                  <ChevronRight size={14} />
                </div>
              </div>
              <div className="session-item-actions">
                <button
                  type="button"
                  className="session-action-btn"
                  title={session.pinned ? "Unpin" : "Pin"}
                  onClick={(e) => { e.stopPropagation(); onPinSession(session.sessionId, !session.pinned); }}
                >
                  {session.pinned ? <PinOff size={12} /> : <Pin size={12} />}
                </button>
                <button
                  type="button"
                  className="session-action-btn"
                  title="Archive"
                  onClick={(e) => { e.stopPropagation(); onArchiveSession(session.sessionId); }}
                >
                  <Archive size={12} />
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

/* ─── Settings View ────────────────────────────────────────────── */

interface SettingsViewProps {
  maxCompactIcons: number;
  maxCompactIconLimit: number;
  onChangeMaxCompactIcons: (value: number) => void;
  showFoldedIslandSizeSetting: boolean;
  foldedIslandSize: FoldedIslandSize;
  onChangeFoldedIslandSize: (small: boolean) => void;
  maxSubagentDisplay: number;
  onChangeMaxSubagentDisplay: (value: number) => void;
  retentionMinutes: number;
  onChangeRetentionMinutes: (value: number) => void;
  subagentRetentionMinutes: number;
  onChangeSubagentRetentionMinutes: (value: number) => void;
  idleIntervalSec: number;
  onChangeIdleInterval: (value: number) => void;
  idleDurationSec: number;
  onChangeIdleDuration: (value: number) => void;
  launchAtLogin: boolean;
  launchAtLoginBusy?: boolean;
  onChangeLaunchAtLogin: (enabled: boolean) => void;
  onOpenHooks: () => void;
  onOpenTokens: () => void;
  todayTokenTotal: number;
  hooksSummary: string;
  hooksNeedAttention: boolean;
}

function SettingsToggle({
  label,
  desc,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  desc: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{label}</span>
        <button
          type="button"
          role="switch"
          aria-checked={checked}
          aria-label={label}
          className={`settings-toggle${checked ? " is-on" : ""}`}
          disabled={disabled}
          onClick={() => onChange(!checked)}
          data-no-drag
        >
          <span className="settings-toggle-thumb" />
        </button>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

function SettingsSlider({
  label,
  value,
  min,
  max,
  step = 1,
  unit,
  desc,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  desc: string;
  onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{label}</span>
        <span className="settings-card-value">
          {value}
          {unit ? <span className="settings-card-unit">{unit}</span> : null}
        </span>
      </div>
      <div className="settings-slider-wrap">
        <input
          type="range"
          className="settings-slider"
          min={min}
          max={max}
          step={step}
          value={value}
          style={{ "--slider-pct": `${pct}%` } as React.CSSProperties}
          onChange={(e) => onChange(Number(e.target.value))}
        />
        <div className="settings-slider-labels">
          <span>{min}{unit ?? ""}</span>
          <span>{max}{unit ?? ""}</span>
        </div>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

function SettingsView({
  maxCompactIcons,
  maxCompactIconLimit,
  onChangeMaxCompactIcons,
  showFoldedIslandSizeSetting,
  foldedIslandSize,
  onChangeFoldedIslandSize,
  maxSubagentDisplay,
  onChangeMaxSubagentDisplay,
  retentionMinutes,
  onChangeRetentionMinutes,
  subagentRetentionMinutes,
  onChangeSubagentRetentionMinutes,
  idleIntervalSec,
  onChangeIdleInterval,
  idleDurationSec,
  onChangeIdleDuration,
  launchAtLogin,
  launchAtLoginBusy = false,
  onChangeLaunchAtLogin,
  onOpenHooks,
  onOpenTokens,
  todayTokenTotal,
  hooksSummary,
  hooksNeedAttention,
}: SettingsViewProps) {
  const todayLabel =
    todayTokenTotal > 0
      ? `${formatCompactTokenCount(todayTokenTotal, todayTokenTotal >= 1_000 ? 1 : 0, todayTokenTotal)} today`
      : "No usage yet";

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">General</span>
          <SettingsToggle
            label="Launch at login"
            desc="Start Atoll automatically when you log in. Requires the installed Atoll.app from Applications."
            checked={launchAtLogin}
            disabled={launchAtLoginBusy}
            onChange={onChangeLaunchAtLogin}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">Usage</span>
          <button
            type="button"
            className="settings-nav-card"
            onClick={onOpenTokens}
            data-no-drag
          >
            <div className="settings-nav-card-copy">
              <span className="settings-card-title">Token activity</span>
              <span className="settings-card-desc">
                Daily token heatmap and usage history.
              </span>
            </div>
            <div className="settings-nav-card-meta">
              <span className="settings-hook-badge is-summary is-installed">
                {todayLabel}
              </span>
              <ChevronRight size={14} className="settings-nav-chevron" />
            </div>
          </button>
        </div>

        <div className="settings-section">
          <span className="settings-section-label">Integrations</span>
          <button
            type="button"
            className="settings-nav-card"
            onClick={onOpenHooks}
            data-no-drag
          >
            <div className="settings-nav-card-copy">
              <span className="settings-card-title">Agent hooks</span>
              <span className="settings-card-desc">
                Connect Claude Code, Codex, and future local agents.
              </span>
            </div>
            <div className="settings-nav-card-meta">
              <span
                className={`settings-hook-badge is-summary${
                  hooksNeedAttention ? " is-missing" : hooksSummary === "All agents connected" ? " is-installed" : ""
                }`}
              >
                {hooksSummary}
              </span>
              <ChevronRight size={14} className="settings-nav-chevron" />
            </div>
          </button>
        </div>

        <div className="settings-section">
          <span className="settings-section-label">Display</span>
          {showFoldedIslandSizeSetting ? (
            <SettingsToggle
              label="Small folded island"
              desc="Use the smaller Windows folded island so pages under the top-center edge stay easier to click."
              checked={foldedIslandSize === "small"}
              onChange={onChangeFoldedIslandSize}
            />
          ) : null}
          <SettingsSlider
            label="Folded icon limit"
            value={maxCompactIcons}
            min={MIN_MAX_COMPACT_ICONS}
            max={maxCompactIconLimit}
            desc={
              maxCompactIconLimit < ABSOLUTE_MAX_COMPACT_ICONS
                ? `Up to ${maxCompactIconLimit} icons fit on this display; extras spill to the right beside tokens.`
                : "Max session icons in compact mode; extras spill to the right beside tokens."
            }
            onChange={onChangeMaxCompactIcons}
          />
          <SettingsSlider
            label="Subagent display limit"
            value={maxSubagentDisplay}
            min={MIN_MAX_SUBAGENT_DISPLAY}
            max={MAX_MAX_SUBAGENT_DISPLAY}
            desc="Max subagent chips shown per session; extras collapse to ×N."
            onChange={onChangeMaxSubagentDisplay}
          />
          <SettingsSlider
            label="Session auto-archive"
            value={retentionMinutes}
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            unit=" min"
            desc="Minutes before idle sessions are auto-archived. Pinned sessions are exempt."
            onChange={onChangeRetentionMinutes}
          />
          <SettingsSlider
            label="Subagent auto-archive"
            value={subagentRetentionMinutes}
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            unit=" min"
            desc="Minutes after completion before subagents are auto-archived."
            onChange={onChangeSubagentRetentionMinutes}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">Mascot</span>
          <SettingsSlider
            label="Activity interval"
            value={idleIntervalSec}
            min={MIN_IDLE_INTERVAL_MIN}
            max={MAX_IDLE_INTERVAL_MIN}
            unit=" min"
            desc="Minutes between random mascot activity switches."
            onChange={onChangeIdleInterval}
          />
          <SettingsSlider
            label="Activity duration"
            value={idleDurationSec}
            min={MIN_IDLE_DURATION_MIN}
            max={MAX_IDLE_DURATION_MIN}
            unit=" min"
            desc="How long each activity plays before switching."
            onChange={onChangeIdleDuration}
          />
        </div>
      </div>
    </div>
  );
}

/* ─── Approval Card ───────────────────────────────────────────── */

function getPlanModeType(request: PermissionRequest): "question" | "exitPlan" | null {
  if (request.command.startsWith("AskUserQuestion:") || request.command === "AskUserQuestion") {
    return "question";
  }
  if (request.command.startsWith("ExitPlanMode:") || request.command === "ExitPlanMode") {
    return "exitPlan";
  }
  return null;
}

interface PlanQuestionOption {
  label: string;
  description: string;
}

interface PlanQuestion {
  question: string;
  header: string;
  options: PlanQuestionOption[];
  multiSelect: boolean;
}

interface PlanQuestionCardProps {
  request: PermissionRequest;
  onResolve: (snapshot: IslandSnapshot) => void;
}

function parsePlanContent(toolInput: unknown): string | null {
  if (!toolInput || typeof toolInput !== "object") {
    return null;
  }
  const plan = (toolInput as { plan?: unknown }).plan;
  if (typeof plan !== "string") {
    return null;
  }
  const trimmed = plan.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function parsePlanQuestions(toolInput: unknown): PlanQuestion[] {
  if (!toolInput || typeof toolInput !== "object") {
    return [];
  }
  const questions = (toolInput as { questions?: unknown }).questions;
  if (!Array.isArray(questions)) {
    return [];
  }
  return questions.flatMap((entry) => {
    if (!entry || typeof entry !== "object") {
      return [];
    }
    const record = entry as Record<string, unknown>;
    const question = typeof record.question === "string" ? record.question : "";
    const header = typeof record.header === "string" ? record.header : "";
    const multiSelect = Boolean(record.multiSelect);
    const options = Array.isArray(record.options)
      ? record.options.flatMap((option) => {
          if (!option || typeof option !== "object") {
            return [];
          }
          const optionRecord = option as Record<string, unknown>;
          const label = typeof optionRecord.label === "string" ? optionRecord.label : "";
          const description =
            typeof optionRecord.description === "string" ? optionRecord.description : "";
          if (!label) {
            return [];
          }
          return [{ label, description }];
        })
      : [];
    if (!question || options.length === 0) {
      return [];
    }
    return [{ question, header, options, multiSelect }];
  });
}

function getOriginalQuestions(toolInput: unknown): unknown[] {
  if (!toolInput || typeof toolInput !== "object") return [];
  const questions = (toolInput as { questions?: unknown }).questions;
  return Array.isArray(questions) ? questions : [];
}

const OTHER_SENTINEL = "__atoll_other__";

function PlanQuestionCard({ request, onResolve }: PlanQuestionCardProps) {
  const questions = useMemo(() => parsePlanQuestions(request.toolInput), [request.toolInput]);
  const [answers, setAnswers] = useState<Record<string, string | string[]>>({});
  const [otherActive, setOtherActive] = useState<Record<string, boolean>>({});
  const [otherText, setOtherText] = useState<Record<string, string>>({});
  const [freeResponse, setFreeResponse] = useState("");
  const [useFreeResponse, setUseFreeResponse] = useState(false);
  const [busy, setBusy] = useState(false);

  function toggleOption(question: PlanQuestion, label: string) {
    const key = question.question;
    if (label === OTHER_SENTINEL) {
      setOtherActive((c) => ({ ...c, [key]: !c[key] }));
      if (otherActive[key]) {
        setOtherText((c) => ({ ...c, [key]: "" }));
        setAnswers((current) => {
          if (question.multiSelect) {
            const existing = current[key];
            const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
            return { ...current, [key]: selected.filter((item) => item !== OTHER_SENTINEL) };
          }
          const { [key]: _, ...rest } = current;
          return rest;
        });
      }
      return;
    }
    setAnswers((current) => {
      if (question.multiSelect) {
        const existing = current[key];
        const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
        const next = selected.includes(label)
          ? selected.filter((item) => item !== label)
          : [...selected, label];
        return { ...current, [key]: next };
      }
      setOtherActive((c) => ({ ...c, [key]: false }));
      setOtherText((c) => ({ ...c, [key]: "" }));
      return { ...current, [key]: label };
    });
  }

  function isOptionSelected(question: PlanQuestion, label: string) {
    if (label === OTHER_SENTINEL) return !!otherActive[question.question];
    const key = question.question;
    const value = answers[key];
    if (Array.isArray(value)) return value.includes(label);
    return value === label;
  }

  function buildUpdatedInput(): Record<string, unknown> {
    if (useFreeResponse && freeResponse.trim()) {
      return {
        questions: getOriginalQuestions(request.toolInput),
        response: freeResponse.trim(),
      };
    }
    const finalAnswers: Record<string, string | string[]> = {};
    for (const q of questions) {
      const key = q.question;
      if (otherActive[key] && otherText[key]?.trim()) {
        if (q.multiSelect) {
          const existing = answers[key];
          const selected = Array.isArray(existing) ? existing : existing ? [existing] : [];
          const filtered = selected.filter((s) => s !== OTHER_SENTINEL);
          finalAnswers[key] = [...filtered, otherText[key].trim()];
        } else {
          finalAnswers[key] = otherText[key].trim();
        }
      } else {
        const val = answers[key];
        if (val !== undefined) {
          finalAnswers[key] = val;
        }
      }
    }
    return {
      questions: getOriginalQuestions(request.toolInput),
      answers: finalAnswers,
    };
  }

  async function handleSubmit() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionWithInput(
        request.id,
        "approved",
        "",
        buildUpdatedInput(),
      );
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  async function handleDeny() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(request.id, "denied");
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="approval-view plan-question-view">
      <div className="request-main">
        <div className="request-kicker">
          <span className="kicker-label">
            <HelpCircle size={14} />
            Plan questions
          </span>
          <span className={`agent-label ${getSessionColor(request.session).tone}`}>
            {agentLabels[request.agent]}
          </span>
        </div>
        {useFreeResponse ? (
          <div className="plan-questions">
            <div className="plan-question-block">
              <p className="plan-question-text">Type your response:</p>
              <textarea
                className="plan-other-input plan-free-response"
                value={freeResponse}
                onChange={(e) => setFreeResponse((e.target as HTMLTextAreaElement).value)}
                placeholder="Type a freeform reply..."
                rows={3}
                disabled={busy}
              />
            </div>
          </div>
        ) : (
          <div className="plan-questions">
            {questions.map((question) => (
              <div className="plan-question-block" key={question.question}>
                {question.header ? <p className="plan-question-header">{question.header}</p> : null}
                <p className="plan-question-text">{question.question}</p>
                <div className="plan-options">
                  {question.options.map((option) => {
                    const selected = isOptionSelected(question, option.label);
                    return (
                      <button
                        key={option.label}
                        type="button"
                        className={`plan-option ${selected ? "selected" : ""}`}
                        onClick={() => toggleOption(question, option.label)}
                        disabled={busy}
                      >
                        <span className="plan-option-label">{option.label}</span>
                        {option.description ? (
                          <span className="plan-option-description">{option.description}</span>
                        ) : null}
                      </button>
                    );
                  })}
                  <button
                    type="button"
                    className={`plan-option plan-option-other ${isOptionSelected(question, OTHER_SENTINEL) ? "selected" : ""}`}
                    onClick={() => toggleOption(question, OTHER_SENTINEL)}
                    disabled={busy}
                  >
                    <span className="plan-option-label">Other...</span>
                  </button>
                </div>
                {otherActive[question.question] && (
                  <input
                    type="text"
                    className="plan-other-input"
                    placeholder="Type your answer..."
                    value={otherText[question.question] || ""}
                    onChange={(e) =>
                      setOtherText((c) => ({
                        ...c,
                        [question.question]: (e.target as HTMLInputElement).value,
                      }))
                    }
                    disabled={busy}
                  />
                )}
              </div>
            ))}
          </div>
        )}
      </div>
      <div className="approval-footer">
        <button
          type="button"
          className="plan-toggle-free"
          onClick={() => setUseFreeResponse((v) => !v)}
          disabled={busy}
        >
          {useFreeResponse ? "Back to options" : "Reply freely instead"}
        </button>
        <div className="decision-row">
          <button
            className="decision-button deny"
            type="button"
            onClick={handleDeny}
            disabled={busy}
          >
            <X size={16} />
            <span>{busy ? "Denying..." : "Deny"}</span>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={handleSubmit}
            disabled={busy}
          >
            <Check size={16} />
            <span>{busy ? "Submitting..." : "Submit"}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

interface PlanApprovalCardProps {
  request: PermissionRequest;
  onResolve: (snapshot: IslandSnapshot) => void;
}

function PlanApprovalCard({ request, onResolve }: PlanApprovalCardProps) {
  const sessionColor = getSessionColor(request.session);
  const [busy, setBusy] = useState(false);
  const planContent = useMemo(() => parsePlanContent(request.toolInput), [request.toolInput]);

  function handlePlanPreviewClick(event: MouseEvent<HTMLDivElement>) {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.href) {
      event.preventDefault();
      openUrl(anchor.href);
    }
  }

  async function handleApprove() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(request.id, "approved");
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  async function handleContinuePlanning() {
    setBusy(true);
    try {
      const snapshot = await resolvePermissionRequest(
        request.id,
        "denied",
        "Continue planning",
      );
      onResolve(snapshot);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="approval-view plan-approval-view">
      <div className="request-main">
        <div className="request-kicker">
          <span className="kicker-label">
            <AgentMascot
              agent={request.agent}
              mood="alert"
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={18}
            />
            Ready to build
          </span>
          <span className={`agent-label ${sessionColor.tone}`}>{agentLabels[request.agent]}</span>
        </div>
        <p className="plan-approval-message">Agent is ready to start building</p>
        {planContent ? (
          <div className="plan-preview" onClick={handlePlanPreviewClick}>
            <div className="plan-preview-md">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>{planContent}</ReactMarkdown>
            </div>
          </div>
        ) : null}
      </div>
      <div className="approval-footer">
        <div className="decision-row">
          <button
            className="decision-button deny"
            type="button"
            onClick={handleContinuePlanning}
            disabled={busy}
          >
            <HelpCircle size={16} />
            <span>{busy ? "Sending..." : "Continue Planning"}</span>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={handleApprove}
            disabled={busy}
          >
            <Hammer size={16} />
            <span>{busy ? "Approving..." : "Agree to Build"}</span>
          </button>
        </div>
      </div>
    </div>
  );
}

interface ApprovalCardProps {
  request: PermissionRequest;
  busyDecision: Decision | null;
  sessions: SessionSummary[];
  onApprove: () => void;
  onDeny: () => void;
  onAlwaysApprove: () => void;
  onViewSession: (sessionId: string) => void;
}

function ApprovalCard({ request, busyDecision, sessions, onApprove, onDeny, onAlwaysApprove, onViewSession }: ApprovalCardProps) {
  const session = sessions.find((s) => s.sessionId === request.session);
  const sessionColor = getSessionColor(request.session);
  const tone = sessionColor.tone;
  const risk = useMemo(() => assessRisk(request.command), [request.command]);
  const mascotMood: ClawdMood = risk === "danger" ? "worried" : "alert";

  return (
    <div className={`approval-view ${risk ? `is-${risk}` : ""}`}>
      <div className="request-main">
        <div className="request-kicker">
          <span className="kicker-label">
            <AgentMascot
              agent={request.agent}
              mood={mascotMood}
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={18}
            />
            Command request
          </span>
          <span className="kicker-tags">
            {risk ? (
              <span className={`risk-pill ${risk}`}>
                <TriangleAlert size={11} />
                {riskLabels[risk]}
              </span>
            ) : null}
            <span className={`agent-label ${tone}`}>{agentLabels[request.agent]}</span>
          </span>
        </div>
        <code className={`command-block ${risk ? `risk-${risk}` : ""}`}>{request.command}</code>
        {request.detail ? <p className="request-detail">{request.detail}</p> : null}
        <div className="cwd-line" title={request.cwd}>
          <FolderClosed size={11} />
          <span className="cwd-path">{request.cwd}</span>
        </div>
      </div>

      <div className="approval-footer">
        <div className={`decision-row ${request.supportsAlways ? "has-always" : ""}`}>
          <button
            className="decision-button deny"
            type="button"
            onClick={onDeny}
            disabled={busyDecision !== null}
          >
            <X size={16} />
            <span>{busyDecision === "denied" ? "Denying..." : "Deny"}</span>
            <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.deny}</kbd>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={onApprove}
            disabled={busyDecision !== null}
          >
            <Check size={16} />
            <span>{busyDecision === "approved" ? "Approving..." : "Approve"}</span>
            <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.approve}</kbd>
          </button>
          {request.supportsAlways ? (
            <button
              className="decision-button always-approve"
              type="button"
              onClick={onAlwaysApprove}
              disabled={busyDecision !== null}
              title="Approve this and all future requests for this session"
            >
              <CheckCheck size={16} />
              <span>Always</span>
              <kbd className="decision-kbd" aria-hidden="true">{DECISION_SHORTCUTS.always}</kbd>
            </button>
          ) : null}
        </div>
        {session ? (
          <button
            type="button"
            className="view-session-link"
            onClick={() => onViewSession(request.session)}
          >
            View session
            <ChevronRight size={12} />
          </button>
        ) : null}
      </div>
    </div>
  );
}

/* ─── Subview Header Nav (menu-bar row) ───────────────────────── */

interface SessionSubviewNavProps {
  cwd: string;
  agent?: AgentKind;
  sessionId?: string;
  sessionHost?: SessionHost;
  onBack: () => void;
  onOpenExternal: () => void;
}

function sessionJumpLabel(agent?: AgentKind, sessionHost?: SessionHost): string {
  if (agent === "claude") {
    if (sessionHost === "claudeCli") return "Terminal";
    return "Open Claude";
  }
  if (agent === "codex") {
    if (sessionHost === "codexCli") return "Terminal";
    return "Open Codex";
  }
  if (agent === "cursor") {
    return "Open Cursor";
  }
  return "Terminal";
}

function SessionSubviewNav({
  cwd,
  agent,
  sessionId,
  sessionHost,
  onBack,
  onOpenExternal,
}: SessionSubviewNavProps) {
  return (
    <div className="session-detail-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>Back</span>
      </button>
      <button
        type="button"
        className="open-terminal-button"
        onClick={onOpenExternal}
      >
        <ExternalLink size={13} />
        <span>{sessionJumpLabel(agent, sessionHost)}</span>
      </button>
    </div>
  );
}

interface SettingsSubviewNavProps {
  onBack: () => void;
}

function SettingsSubviewNav({ onBack }: SettingsSubviewNavProps) {
  return (
    <div className="settings-subview-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>Back</span>
      </button>
      <span className="settings-header-title">
        <Settings2 size={14} />
        <span>Settings</span>
      </span>
    </div>
  );
}

/* ─── Session Chat View ───────────────────────────────────────── */

interface SessionChatViewProps {
  sessionId: string;
  transcriptPath: string | null;
  requests: PermissionRequest[];
  agent: AgentKind;
}

function SessionChatView({ sessionId, transcriptPath, requests, agent }: SessionChatViewProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loadFailed, setLoadFailed] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const pollWhileActive = agent === "cursor";

  useEffect(() => {
    let active = true;
    let loading = false;
    setLoadFailed(false);

    function loadByPath(path: string) {
      return getSessionTranscript(path)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          setMessages(msgs);
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }

    function loadBySession() {
      return getSessionChat(sessionId)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          setMessages(msgs);
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }

    function load() {
      if (loading) {
        return Promise.resolve();
      }
      loading = true;
      const request = transcriptPath
        ? loadByPath(transcriptPath)
        : pollWhileActive
          ? loadBySession()
          : Promise.resolve();
      return request.finally(() => {
        loading = false;
      });
    }

    function loadAndIgnore() {
      void load();
    }

    loadAndIgnore();
    const interval = pollWhileActive ? window.setInterval(loadAndIgnore, 2000) : undefined;
    return () => {
      active = false;
      if (interval !== undefined) {
        window.clearInterval(interval);
      }
    };
  }, [sessionId, transcriptPath, pollWhileActive]);

  useEffect(() => {
    if (!scrollRef.current) {
      return;
    }
    scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
  }, [messages]);

  return (
    <div className="session-chat">
      <div className="chat-messages" ref={scrollRef}>
        {messages.length === 0 && requests.length === 0 ? (
          <div className="chat-empty">
            {loadFailed
              ? "Transcript unavailable."
              : pollWhileActive || transcriptPath
                ? "Loading conversation..."
                : "No conversation history."}
          </div>
        ) : null}
        {messages.map((msg, i) => (
          <ChatBubble key={i} message={msg} />
        ))}
      </div>
    </div>
  );
}

/* ─── Subagent List View ──────────────────────────────────────── */

interface SubagentListViewProps {
  subagents: SubagentSummary[];
  agent: AgentKind;
  onSelectSubagent: (agentId: string) => void;
  onArchiveCompletedSubagents: () => void | Promise<void>;
}

const SUBAGENT_LIST_VIRTUALIZE_THRESHOLD = 40;
const SUBAGENT_LIST_ROW_HEIGHT = 52;
const SUBAGENT_LIST_OVERSCAN = 6;
const SUBAGENT_LIST_FALLBACK_VIEWPORT_HEIGHT = 260;

function sortSubagents(subagents: SubagentSummary[]) {
  return [...subagents].sort((a, b) => {
    const aDone = Boolean(a.completedAt);
    const bDone = Boolean(b.completedAt);
    if (aDone !== bDone) return aDone ? 1 : -1;
    return b.startedAt.localeCompare(a.startedAt);
  });
}

interface SubagentListRowProps {
  subagent: SubagentSummary;
  agent: AgentKind;
  onSelectSubagent: (agentId: string) => void;
  style?: CSSProperties;
}

const SubagentListRow = memo(function SubagentListRow({
  subagent,
  agent,
  onSelectSubagent,
  style,
}: SubagentListRowProps) {
  const completed = Boolean(subagent.completedAt);
  const color = getSubagentColor(subagent.agentId);
  const mood = getSubagentMood(subagent.agentId, completed);
  const handleClick = useCallback(() => {
    onSelectSubagent(subagent.agentId);
  }, [onSelectSubagent, subagent.agentId]);

  return (
    <button
      type="button"
      className={`subagent-list-item ${color.tone} ${completed ? "is-completed" : ""}`}
      onClick={handleClick}
      style={style}
    >
      <AgentMascot
        agent={agent}
        size={18}
        mood={mood}
        accent={color.accent}
        accentDark={color.accentDark}
        animated={false}
      />
      <div className="subagent-list-item-info">
        <span className={`subagent-list-item-name ${color.tone}`}>
          {subagent.agentType}
        </span>
        <span className="subagent-list-item-meta">
          {timeAgo(subagent.startedAt)}
          {subagent.lastMessage ? (
            <>
              <span className="meta-divider">·</span>
              <span className="subagent-list-item-last-msg">
                {subagent.lastMessage}
              </span>
            </>
          ) : null}
        </span>
      </div>
      <div className="subagent-list-item-trail">
        {completed ? (
          <span className="subagent-status-badge done">
            <Check size={10} /> Done
          </span>
        ) : (
          <span className="subagent-status-badge running">Running</span>
        )}
        <ChevronRight size={14} />
      </div>
    </button>
  );
});

function SubagentListView({
  subagents,
  agent,
  onSelectSubagent,
  onArchiveCompletedSubagents,
}: SubagentListViewProps) {
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const listState = useMemo(() => {
    let runningCount = 0;
    let hasCompleted = false;
    for (const subagent of subagents) {
      if (subagent.completedAt) {
        hasCompleted = true;
      } else {
        runningCount += 1;
      }
    }
    return {
      sorted: sortSubagents(subagents),
      runningCount,
      hasCompleted,
    };
  }, [subagents]);
  const { sorted, runningCount, hasCompleted } = listState;
  const shouldVirtualize = sorted.length > SUBAGENT_LIST_VIRTUALIZE_THRESHOLD;
  const effectiveViewportHeight =
    viewportHeight || SUBAGENT_LIST_FALLBACK_VIEWPORT_HEIGHT;
  const maxScrollTop = Math.max(
    0,
    sorted.length * SUBAGENT_LIST_ROW_HEIGHT - effectiveViewportHeight,
  );
  const virtualScrollTop = shouldVirtualize
    ? Math.min(scrollTop, maxScrollTop)
    : 0;
  const visibleStart = shouldVirtualize
    ? Math.max(
        0,
        Math.floor(virtualScrollTop / SUBAGENT_LIST_ROW_HEIGHT) -
          SUBAGENT_LIST_OVERSCAN,
      )
    : 0;
  const visibleEnd = shouldVirtualize
    ? Math.min(
        sorted.length,
        Math.ceil(
          (virtualScrollTop + effectiveViewportHeight) /
            SUBAGENT_LIST_ROW_HEIGHT,
        ) + SUBAGENT_LIST_OVERSCAN,
      )
    : sorted.length;
  const visibleSubagents = shouldVirtualize
    ? sorted.slice(visibleStart, visibleEnd)
    : sorted;

  useEffect(() => {
    const listEl = listRef.current;
    if (!listEl) return;

    const updateViewport = () => {
      setViewportHeight(listEl.clientHeight);
    };
    updateViewport();

    if (typeof ResizeObserver !== "undefined") {
      const observer = new ResizeObserver(updateViewport);
      observer.observe(listEl);
      return () => observer.disconnect();
    }

    window.addEventListener("resize", updateViewport);
    return () => window.removeEventListener("resize", updateViewport);
  }, []);

  useEffect(() => {
    if (!shouldVirtualize && scrollTop !== 0) {
      setScrollTop(0);
      return;
    }
    if (shouldVirtualize && scrollTop > maxScrollTop) {
      setScrollTop(maxScrollTop);
    }
  }, [maxScrollTop, scrollTop, shouldVirtualize]);

  const handleScroll = useCallback((event: ReactUIEvent<HTMLDivElement>) => {
    setScrollTop(event.currentTarget.scrollTop);
  }, []);

  async function handleArchiveCompletedSubagents() {
    if (archiveBusy) return;
    setArchiveBusy(true);
    try {
      await onArchiveCompletedSubagents();
    } finally {
      setArchiveBusy(false);
    }
  }

  return (
    <div className="subagent-list-view">
      <div className="subagent-list-header">
        <div className="subagent-list-title-row">
          <Layers size={14} />
          <span className="subagent-list-title">
            Subagents ({subagents.length})
          </span>
          {runningCount > 0 ? (
            <span className="subagent-list-running-badge">{runningCount} running</span>
          ) : null}
        </div>
        {hasCompleted ? (
          <button
            type="button"
            className="subagent-list-archive-all-btn"
            onClick={handleArchiveCompletedSubagents}
            disabled={archiveBusy}
          >
            <Archive size={12} />
            <span>{archiveBusy ? "Archiving..." : "Archive completed"}</span>
          </button>
        ) : null}
      </div>
      <div
        ref={listRef}
        className={`subagent-list-body ${shouldVirtualize ? "is-virtualized" : ""}`}
        onScroll={handleScroll}
      >
        {shouldVirtualize ? (
          <div
            className="subagent-list-virtual-spacer"
            style={{ height: sorted.length * SUBAGENT_LIST_ROW_HEIGHT }}
          >
            <div
              className="subagent-list-virtual-window"
              style={{
                transform: `translateY(${
                  visibleStart * SUBAGENT_LIST_ROW_HEIGHT
                }px)`,
              }}
            >
              {visibleSubagents.map((subagent) => (
                <SubagentListRow
                  key={subagent.agentId}
                  subagent={subagent}
                  agent={agent}
                  onSelectSubagent={onSelectSubagent}
                  style={{ height: SUBAGENT_LIST_ROW_HEIGHT }}
                />
              ))}
            </div>
          </div>
        ) : (
          visibleSubagents.map((subagent) => (
            <SubagentListRow
              key={subagent.agentId}
              subagent={subagent}
              agent={agent}
              onSelectSubagent={onSelectSubagent}
            />
          ))
        )}
      </div>
    </div>
  );
}

/* ─── Subagent Detail View ───────────────────────────────────── */

interface SubagentDetailViewProps {
  agentId: string;
  agent: AgentKind;
  agentType: string;
  startedAt: string;
  completedAt: string | null;
  lastMessage: string | null;
  transcriptPath: string | null;
  onArchive: () => void | Promise<void>;
}

function SubagentDetailView({
  agentId,
  agent,
  agentType,
  startedAt,
  completedAt,
  lastMessage,
  transcriptPath,
  onArchive,
}: SubagentDetailViewProps) {
  const subagentColor = getSubagentColor(agentId);
  const subagentMood = getSubagentMood(agentId, Boolean(completedAt));
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [loadFailed, setLoadFailed] = useState(false);
  const [archiveBusy, setArchiveBusy] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const prevCountRef = useRef(0);

  useEffect(() => {
    if (!transcriptPath) return;
    let active = true;
    setLoadFailed(false);
    function load() {
      getSessionTranscript(transcriptPath!)
        .then((msgs) => {
          if (!active) return;
          setLoadFailed(false);
          if (msgs.length !== prevCountRef.current) {
            prevCountRef.current = msgs.length;
            setMessages(msgs);
          }
        })
        .catch(() => {
          if (!active) return;
          setLoadFailed(true);
        });
    }
    load();
    const pollMs = completedAt ? 0 : 2000;
    const interval = pollMs > 0 ? setInterval(load, pollMs) : undefined;
    return () => {
      active = false;
      if (interval) clearInterval(interval);
    };
  }, [transcriptPath, completedAt]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  async function handleArchive() {
    if (archiveBusy) return;
    setArchiveBusy(true);
    try {
      await onArchive();
    } finally {
      setArchiveBusy(false);
    }
  }

  return (
    <div className="subagent-detail-view">
      <div className="subagent-detail-header">
        <div className="subagent-detail-title-row">
          <AgentMascot
            agent={agent}
            size={20}
            mood={subagentMood}
            accent={subagentColor.accent}
            accentDark={subagentColor.accentDark}
          />
          <h2 className={`subagent-detail-title ${subagentColor.tone}`}>{agentType}</h2>
          {completedAt ? (
            <span className="subagent-status-badge done">
              <Check size={10} /> Done
            </span>
          ) : (
            <span className="subagent-status-badge running">Running</span>
          )}
        </div>
        <p className="subagent-detail-subtitle">
          Started {timeAgo(startedAt)}
          {completedAt ? ` · Finished ${timeAgo(completedAt)}` : ""}
        </p>
        {lastMessage && messages.length === 0 ? (
          <p className="subagent-last-message">{lastMessage}</p>
        ) : null}
      </div>
      <div className="session-chat">
        <div className="chat-messages" ref={scrollRef}>
          {messages.length === 0 && !lastMessage ? (
            <div className="chat-empty">
              {!transcriptPath
                ? "No transcript path available."
                : loadFailed && completedAt
                  ? "Transcript unavailable."
                  : "Loading..."}
            </div>
          ) : null}
          {messages.map((msg, i) => (
            <ChatBubble key={i} message={msg} />
          ))}
        </div>
      </div>
      {completedAt ? (
        <div className="subagent-detail-footer">
          <button
            type="button"
            className="subagent-archive-btn"
            onClick={handleArchive}
            disabled={archiveBusy}
          >
            <Archive size={14} />
            <span>{archiveBusy ? "Archiving..." : "Archive"}</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}

/* ─── Hooks View ──────────────────────────────────────────────── */

interface HookMenuAgent {
  key: string;
  label: string;
  status: HookStatus | null;
  note?: string;
  onInstall: () => void;
  onUninstall: () => void;
}

interface HooksViewProps {
  agents: HookMenuAgent[];
  hookBusy: boolean;
  hookInstallError: string | null;
  onInstallAll: () => void;
  onUninstallAll: () => void;
}

function formatHookInstallError(agentLabel: string, error: unknown): string {
  const message =
    typeof error === "string"
      ? error
      : error instanceof Error
        ? error.message
        : "Unknown error";
  return `${agentLabel} hook install failed: ${message}`;
}

function TokensSubviewNav({
  onBack,
  backLabel,
}: {
  onBack: () => void;
  backLabel: string;
}) {
  return (
    <div className="settings-subview-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>{backLabel}</span>
      </button>
      <span className="settings-header-title">
        <Activity size={14} />
        <span>Token activity</span>
      </span>
    </div>
  );
}

function HooksSubviewNav({
  onBack,
  backLabel,
}: {
  onBack: () => void;
  backLabel: string;
}) {
  return (
    <div className="settings-subview-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>{backLabel}</span>
      </button>
      <span className="settings-header-title">
        <Download size={14} />
        <span>Agent hooks</span>
      </span>
    </div>
  );
}

function HooksView({
  agents,
  hookBusy,
  hookInstallError,
  onInstallAll,
  onUninstallAll,
}: HooksViewProps) {
  const installedCount = agents.filter((agent) => agent.status?.installed).length;
  const missingCount = agents.filter(
    (agent) => agent.status && !agent.status.installed,
  ).length;

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">Agents</span>
          {hookInstallError ? (
            <span className="settings-card-desc settings-hook-warning">{hookInstallError}</span>
          ) : null}
          {missingCount > 0 || installedCount > 1 ? (
            <div className="settings-hook-bulk">
              {missingCount > 0 ? (
                <button
                  type="button"
                  className="settings-hook-button"
                  onClick={onInstallAll}
                  disabled={hookBusy}
                  data-no-drag
                >
                  <Download size={13} />
                  Install all
                </button>
              ) : null}
              {installedCount > 1 ? (
                <button
                  type="button"
                  className="settings-hook-button is-muted"
                  onClick={onUninstallAll}
                  disabled={hookBusy}
                  data-no-drag
                >
                  <Trash2 size={13} />
                  Uninstall all
                </button>
              ) : null}
            </div>
          ) : null}
          {agents.map((agent) => {
            const installed = Boolean(agent.status?.installed);
            const scriptMissing = agent.status && !agent.status.scriptFound;
            const ready = isHookReady(agent.status);
            const needsRetrust = ready && Boolean(agent.status?.needsRetrust);
            const statusIssue = hookStatusIssue(agent.status);
            return (
              <div key={agent.key} className="settings-card settings-hook-card">
                <div className="settings-card-head">
                  <span className="settings-card-title">{agent.label}</span>
                  <span
                    className={`settings-hook-badge${
                      ready
                        ? needsRetrust
                          ? " is-warning"
                          : " is-installed"
                        : installed
                          ? " is-warning"
                          : " is-missing"
                    }`}
                  >
                    {ready
                      ? needsRetrust
                        ? "Needs re-trust"
                        : "Connected"
                      : installed
                        ? "Shim missing"
                        : "Not installed"}
                  </span>
                </div>
                {agent.status?.settingsPath ? (
                  <span className="settings-card-desc settings-hook-path">
                    {agent.status.settingsPath}
                  </span>
                ) : null}
                {agent.note && (!installed || agent.key === "codex" || agent.key === "claude" || agent.key === "cursor") ? (
                  <span className="settings-card-desc">{agent.note}</span>
                ) : null}
                {agent.key === "claude" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>Claude Desktop checklist</summary>
                    <ul>
                      <li>Install Node.js on this machine before installing hooks.</li>
                      <li>In Claude Desktop, set permissions to Ask permissions.</li>
                      <li>Quit and reopen Claude Desktop after installing hooks.</li>
                      <li>Trigger one Bash permission in a local Code session to verify.</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "codex" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>Codex Desktop checklist</summary>
                    <ul>
                      <li>Install Node.js on this machine before installing hooks.</li>
                      <li>In Codex Desktop or CLI, open /hooks and trust the Atoll hook.</li>
                      <li>Quit and reopen Codex Desktop after installing hooks.</li>
                      <li>Trigger one shell permission in a local session to verify.</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "cursor" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>Cursor checklist</summary>
                    <ul>
                      <li>Install Node.js on this machine before installing hooks.</li>
                      <li>In Cursor, open Settings → Hooks and confirm Atoll hooks are loaded.</li>
                      <li>Restart Cursor after installing hooks.</li>
                      <li>Send a message in Agent or Ask mode to verify.</li>
                    </ul>
                  </details>
                ) : null}
                {needsRetrust ? (
                  <span className="settings-card-desc settings-hook-warning">
                    {hookRetrustNote(agent.key as HookAgentKey)}
                  </span>
                ) : null}
                {statusIssue ? (
                  <span className="settings-card-desc settings-hook-warning">
                    {statusIssue}
                  </span>
                ) : null}
                {scriptMissing ? (
                  <span className="settings-card-desc settings-hook-warning">
                    Hook script missing from the app bundle. Reinstall Atoll if install fails.
                  </span>
                ) : null}
                <div className="settings-hook-actions">
                  {installed ? (
                    <button
                      type="button"
                      className="settings-hook-button is-muted"
                      onClick={agent.onUninstall}
                      disabled={hookBusy}
                      data-no-drag
                    >
                      <Trash2 size={13} />
                      Uninstall
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="settings-hook-button"
                      onClick={agent.onInstall}
                      disabled={hookBusy}
                      data-no-drag
                    >
                      <Download size={13} />
                      {hookBusy ? "Installing…" : "Install"}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/* ─── Idle View ───────────────────────────────────────────────── */

interface HeaderLogoProps {
  display: HeaderLogoDisplay;
  size: number;
  idleIntervalSec: number;
  idleDurationSec: number;
  motionPaused: boolean;
}

function HeaderLogo({
  display,
  size,
  idleIntervalSec,
  idleDurationSec,
  motionPaused,
}: HeaderLogoProps) {
  if (display.kind === "agent") {
    return (
      <AgentMascot
        agent={display.agent}
        mood={display.mood}
        size={size}
        className="header-agent-logo"
        accent={agentMascotAccent(display.agent)}
        accentDark={agentMascotDark(display.agent)}
      />
    );
  }

  return (
    <AtollLogo
      size={size}
      activity={display.activity}
      idleIntervalSec={idleIntervalSec}
      idleDurationSec={idleDurationSec}
      motionPaused={motionPaused}
    />
  );
}

interface IdleViewProps {
  needsHookSetup: boolean;
  needsReconnect: boolean;
  disconnectedAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
  retrustAgents: Array<{ key: HookAgentKey; label: string; status: HookStatus }>;
  onOpenHooks: () => void;
}

function IdleView({
  needsHookSetup,
  needsReconnect,
  disconnectedAgents,
  retrustAgents,
  onOpenHooks,
}: IdleViewProps) {
  if (!needsHookSetup) {
    const hasDisconnected = disconnectedAgents.length > 0;
    const hasRetrust = retrustAgents.length > 0;
    const reconnectTitle =
      hasDisconnected && hasRetrust
        ? `${[...disconnectedAgents, ...retrustAgents].map((agent) => agent.label).join(", ")} need attention`
        : hasDisconnected
          ? `${disconnectedAgents.map((agent) => agent.label).join(", ")} disconnected`
          : `${retrustAgents.map((agent) => agent.label).join(", ")} needs re-trust`;
    const reconnectDetail = hasDisconnected
      ? "Hooks were removed or changed outside Atoll. Reconnect to restore approvals."
      : hasRetrust
        ? "Atoll updated the hook script. Re-confirm trust in the agent app to restore approvals."
        : "";
    return (
      <div className={`idle-view${needsReconnect ? " idle-view--alert" : ""}`}>
        <div className="idle-stack">
          {needsReconnect ? (
            <div className="idle-reconnect-banner">
              <div className="idle-reconnect-icon" aria-hidden="true">
                <TriangleAlert size={14} />
              </div>
              <div className="idle-reconnect-copy">
                <strong>{reconnectTitle}</strong>
                <span>{reconnectDetail}</span>
              </div>
              <button
                type="button"
                className="install-button is-compact"
                onClick={onOpenHooks}
                data-no-drag
              >
                Reconnect
              </button>
            </div>
          ) : null}
          <div className="idle-content">
            <span className="idle-dot" />
            <span className="idle-text">Waiting for requests…</span>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="idle-view setup-view">
      <div className="setup-card">
        <div className="setup-head">
          <div className="idle-icon setup-icon">
            <Download size={16} />
          </div>
          <div className="setup-copy">
            <h2>Connect your agents</h2>
            <p>
              Install hooks for Claude Code, Codex, and other local agents from one place.
            </p>
          </div>
        </div>
        <button
          type="button"
          className="install-button"
          onClick={onOpenHooks}
          data-no-drag
        >
          <Download size={14} />
          <span>Open agent hooks</span>
        </button>
      </div>
    </div>
  );
}

/* ─── Helpers ─────────────────────────────────────────────────── */

// True only while a real text field is focused (the reply input). Used to keep
// the island open while typing, without letting a stray button focus pin it.
function isTextEntryActive() {
  if (typeof document === "undefined") return false;
  const element = document.activeElement as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName;
  return tag === "INPUT" || tag === "TEXTAREA" || element.isContentEditable;
}

function sessionDisplayName(cwd: string) {
  if (!cwd || cwd === ".") {
    return "Cursor session";
  }
  const parts = cwd.split(/[/\\]/).filter(Boolean);
  return parts[parts.length - 1] || cwd;
}

function ChatBubbleQuestionReadonly({ toolInput }: { toolInput: unknown }) {
  const input = toolInput as { questions?: Array<{ question: string; header?: string; options?: Array<{ label: string; description?: string }>; multiSelect?: boolean }> } | null;
  const questions = input?.questions;
  if (!questions?.length) return null;

  return (
    <div className="chat-question-readonly">
      {questions.map((q, qi) => (
        <div key={qi} className="chat-question-item">
          {q.header && <span className="chat-question-header">{q.header}</span>}
          <span className="chat-question-text">{q.question}</span>
          {q.options && (
            <div className="chat-question-options">
              {q.options.map((opt, oi) => (
                <span key={oi} className="chat-question-option">{opt.label}</span>
              ))}
            </div>
          )}
        </div>
      ))}
    </div>
  );
}

function ChatBubble({ message }: { message: ChatMessage }) {
  const text = message.content || (message.toolName ? `Using ${message.toolName}...` : "");
  const hasMarkdown = useMemo(() => /[*_`#\[\]!\n>|]/.test(text), [text]);
  const isQuestion = message.toolName === "AskUserQuestion" && message.toolInput;

  function handleClick(event: MouseEvent<HTMLDivElement>) {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.href) {
      event.preventDefault();
      event.stopPropagation();
      openUrl(anchor.href);
    }
  }

  return (
    <div className={`chat-bubble ${message.role}`} onClick={handleClick}>
      {message.toolName ? (
        <span className="chat-tool-badge">{message.toolName}</span>
      ) : null}
      {isQuestion ? (
        <ChatBubbleQuestionReadonly toolInput={message.toolInput} />
      ) : hasMarkdown ? (
        <div className="chat-bubble-md">
          <ReactMarkdown remarkPlugins={[remarkGfm]}>{text}</ReactMarkdown>
        </div>
      ) : (
        <span className="chat-bubble-text">{text}</span>
      )}
    </div>
  );
}

function timeAgo(isoDate: string) {
  const elapsedSeconds = Math.max(1, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));

  if (elapsedSeconds < 60) return `${elapsedSeconds}s ago`;

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) return `${elapsedMinutes}m ago`;

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  return `${elapsedHours}h ago`;
}
