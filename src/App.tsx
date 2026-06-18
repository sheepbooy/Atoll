import { FocusEvent, MouseEvent, PointerEvent as ReactPointerEvent, useEffect, useRef, useState, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Archive,
  ArrowLeft,
  Check,
  CheckCheck,
  ChevronRight,
  ChevronUp,
  Download,
  Ellipsis,
  ExternalLink,
  FolderClosed,
  Layers,
  Pin,
  PinOff,
  Power,
  Settings2,
  TriangleAlert,
  Trash2,
  X,
} from "lucide-react";
import {
  beginCollapse,
  beginExpand,
  COLLAPSE_ANIMATION_MS,
  finishCollapse,
  finishExpand,
  IDLE_COLLAPSE_DELAY_MS,
  PresentationPhase,
} from "./islandPresentation";
import { ClawdMascot, type ClawdMood } from "./ClawdMascot";
import { AtollLogo, type AtollActivity } from "./AtollLogo";
import { deriveAppLogoState, deriveAtollActivity } from "./logoStates";
import {
  ABSOLUTE_MAX_COMPACT_ICONS,
  COMPACT_NOTCH_INNER_GAP,
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  computeMaxCompactIconLimit,
  MIN_MAX_COMPACT_ICONS,
} from "./compactLayout";
import { TokenCounter } from "./TokenCounter";

import {
  getSnapshot,
  getSessionRequests,
  getSessionTranscript,
  IslandSnapshot,
  TokenUsage,
  onIslandHoverChanged,
  onIslandOpenRequested,
  onSnapshotChanged,
  PermissionRequest,
  SessionSummary,
  ChatMessage,
  HookStatus,
  archiveAllResolved,
  archiveSession,
  pinSession,
  deactivateAtoll,
  quitAtoll,
  resolvePermissionRequest,
  setIslandPresentation,
  setSessionAutoApprove,
  getNotchMetrics,
  NotchMetrics,
  getClaudeHookStatus,
  installClaudeHooks,
  uninstallClaudeHooks,
  getSessionRetention,
  setSessionRetention,
  openInTerminal,
  openUrl,
} from "./tauri";

type Decision = "approved" | "denied";
type AgentKind = PermissionRequest["agent"];
type PanelView =
  | { kind: "home" }
  | { kind: "session"; sessionId: string }
  | { kind: "settings" };

const COMPACT_ICON_SETTING_KEY = "atoll.maxCompactIcons";
const RETENTION_SETTING_KEY = "atoll.sessionRetentionMinutes";
const DEFAULT_MAX_COMPACT_ICONS = 3;
const DEFAULT_RETENTION_MINUTES = 15;
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
};

const agentLabels: Record<AgentKind, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  other: "Agent",
};

const agentTone: Record<AgentKind, string> = {
  claude: "coral",
  codex: "cyan",
  gemini: "lime",
  other: "neutral",
};

const agentSortRank: Record<AgentKind, number> = {
  claude: 0,
  codex: 1,
  gemini: 2,
  other: 3,
};
const agentMascotAccent: Record<AgentKind, string | undefined> = {
  claude: undefined,
  codex: "#61d8f7",
  gemini: "#b2e578",
  other: "#c9bcff",
};
const agentMascotDark: Record<AgentKind, string | undefined> = {
  claude: undefined,
  codex: "#3d9fb8",
  gemini: "#7aa44d",
  other: "#9182d1",
};

type SessionTone =
  | "coral"
  | "cyan"
  | "lime"
  | "neutral"
  | "amber"
  | "pink"
  | "teal"
  | "blue";

interface SessionColor {
  tone: SessionTone;
  accent: string;
  accentDark: string;
}

const SESSION_PALETTE: SessionColor[] = [
  { tone: "coral", accent: "#e8765a", accentDark: "#b85a42" },
  { tone: "cyan", accent: "#61d8f7", accentDark: "#3d9fb8" },
  { tone: "lime", accent: "#b2e578", accentDark: "#7aa44d" },
  { tone: "neutral", accent: "#c9bcff", accentDark: "#9182d1" },
  { tone: "amber", accent: "#f0c060", accentDark: "#b89040" },
  { tone: "pink", accent: "#f7a0c8", accentDark: "#c07098" },
  { tone: "teal", accent: "#70d8c8", accentDark: "#48a898" },
  { tone: "blue", accent: "#80b0f8", accentDark: "#5888d0" },
];

function sessionColorIndex(sessionId: string): number {
  if (!sessionId) return 0;
  let hash = 0;
  for (let i = 0; i < sessionId.length; i += 1) {
    hash = ((hash << 5) - hash + sessionId.charCodeAt(i)) | 0;
  }
  return ((hash % SESSION_PALETTE.length) + SESSION_PALETTE.length) % SESSION_PALETTE.length;
}

function getSessionColor(sessionId: string): SessionColor {
  return SESSION_PALETTE[sessionColorIndex(sessionId)];
}

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
];

const CAUTION_PATTERNS: RegExp[] = [
  /\brm\s+-/i,
  /\bgit\s+clean\b/i,
  /\bgit\s+checkout\s+--\s/i,
  /\b(npm|pnpm|yarn|bun)\s+(install|i|ci|add|remove)\b/i,
  /\b(mv|chmod|chown|ln)\b/i,
  /\bdocker\b[^\n]*\b(rm|rmi|prune|down|stop)\b/i,
  /\b(brew|apt|apt-get|yum|dnf|pacman)\s+(install|remove|uninstall)\b/i,
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
// Keep in sync with NOTCH_COVER_PADDING in src-tauri/src/lib.rs.
const NOTCH_COVER_PADDING = 16;

// Keep in sync with EXPANDED_IDLE_WINDOW_HEIGHT in src-tauri/src/lib.rs.
const EXPANDED_IDLE_WINDOW_HEIGHT = 240;

function applyWindowMetrics(notch: NotchMetrics) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--compact-height", `${COMPACT_WINDOW_HEIGHT}px`);
  root.style.setProperty(
    "--expanded-idle-height",
    `${EXPANDED_IDLE_WINDOW_HEIGHT}px`,
  );
  const coverHeight = notch.hasNotch
    ? Math.max(0, notch.height + NOTCH_COVER_PADDING)
    : 0;
  root.style.setProperty("--notch-height", `${coverHeight}px`);
  root.style.setProperty("--notch-width", `${Math.max(0, notch.width)}px`);
  root.style.setProperty("--compact-notch-inner-gap", `${COMPACT_NOTCH_INNER_GAP}px`);
  root.classList.toggle("has-notch", notch.hasNotch);
}

function compactPresentationKey(
  mode: "compact" | "dormant",
  width: number,
  leftWidth: number,
): string {
  return mode === "dormant" ? "dormant" : `compact:${width}:${leftWidth}`;
}

function expandedPresentationKey(idle: boolean): string {
  return `expanded:${idle}`;
}

export function App() {
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(initialSnapshot);
  const snapshotRef = useRef(initialSnapshot);
  const [phase, setPhase] = useState<PresentationPhase>("compact");
  const phaseRef = useRef<PresentationPhase>("compact");
  const hoveringRef = useRef(false);
  const focusedRef = useRef(false);
  const suppressHoverExpandRef = useRef(false);
  const transitionTimerRef = useRef<number | null>(null);
  const idleTimerRef = useRef<number | null>(null);
  const lastNativePresentationKeyRef = useRef<string | null>(null);
  const [busyDecision, setBusyDecision] = useState<Decision | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const busyRef = useRef<Decision | null>(null);
  busyRef.current = busyDecision;
  const menuOpenRef = useRef(false);
  menuOpenRef.current = menuOpen;

  const [panelView, setPanelView] = useState<PanelView>({ kind: "home" });
  const [sessionRequests, setSessionRequests] = useState<PermissionRequest[]>([]);
  const [hookStatus, setHookStatus] = useState<HookStatus | null>(null);
  const [hookBusy, setHookBusy] = useState(false);
  const [selectedAgent, setSelectedAgent] = useState<AgentKind | null>(null);
  const [notchMetrics, setNotchMetrics] = useState<NotchMetrics>(EMPTY_NOTCH_METRICS);
  const [maxCompactIcons, setMaxCompactIcons] = useState<number>(() => readCompactIconLimit());
  const [retentionMinutes, setRetentionMinutes] = useState<number>(() => readRetentionMinutes());
  const [idleIntervalSec, setIdleIntervalSec] = useState<number>(() => readIdleInterval());
  const [idleDurationSec, setIdleDurationSec] = useState<number>(() => readIdleDuration());
  const [justResolved, setJustResolved] = useState(false);
  const prevPendingRef = useRef(0);
  const selectedAgentRef = useRef<AgentKind | null>(null);
  selectedAgentRef.current = selectedAgent;

  const activeRequest = snapshot.activeRequest;
  const sessions = snapshot.sessions;
  const atollActivity = useMemo(
    () =>
      deriveAtollActivity({
        online: snapshot.online,
        pendingCount: snapshot.pendingCount,
        sessionCount: sessions.length,
      }),
    [snapshot.online, snapshot.pendingCount, sessions.length],
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
  const dailyTokens = snapshot.dailyTokens ?? ZERO_TOKEN_USAGE;
  const dailyTokenTotal = dailyTokens.inputTokens + dailyTokens.outputTokens;
  const maxCompactIconLimit = useMemo(
    () => computeMaxCompactIconLimit(notchMetrics),
    [notchMetrics],
  );
  const compactHeaderLayout = useMemo(
    () =>
      computeCompactHeaderLayout(
        notchMetrics,
        sessions.length,
        maxCompactIcons,
        dailyTokenTotal,
        snapshot.pendingCount,
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      dailyTokenTotal,
      snapshot.pendingCount,
    ],
  );
  const collapsedWindowWidth = useMemo(
    () =>
      computeCollapsedWindowWidth(
        notchMetrics,
        sessions.length,
        maxCompactIcons,
        dailyTokenTotal,
        snapshot.pendingCount,
      ),
    [
      notchMetrics,
      sessions.length,
      maxCompactIcons,
      dailyTokenTotal,
      snapshot.pendingCount,
    ],
  );
  const compactLeftPaneWidth = useMemo(
    () => computeCompactLeftPaneWidth(compactHeaderLayout),
    [compactHeaderLayout],
  );
  // With no active sessions the island super-collapses into a tiny top-edge
  // drawer handle instead of showing a capsule.
  const collapsedMode: "compact" | "dormant" =
    sessions.length === 0 && snapshot.pendingCount === 0 ? "dormant" : "compact";
  const collapsedModeRef = useRef<"compact" | "dormant">("compact");
  collapsedModeRef.current = collapsedMode;
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
      gemini: 0,
      other: 0,
    };
    for (const session of sessions) {
      counts[session.agent] += session.pendingCount;
    }
    return counts;
  }, [sessions]);

  useEffect(() => {
    let unsubscribe: () => void = () => undefined;
    let unsubscribeHover: () => void = () => undefined;
    let unsubscribeOpen: () => void = () => undefined;

    getSnapshot()
      .then(applySnapshot)
      .catch(() => undefined);
    getClaudeHookStatus()
      .then(setHookStatus)
      .catch(() => undefined);
    getNotchMetrics()
      .then((notch) => {
        setNotchMetrics(notch);
        applyWindowMetrics(notch);
      })
      .catch(() => {
        setNotchMetrics(EMPTY_NOTCH_METRICS);
        applyWindowMetrics(EMPTY_NOTCH_METRICS);
      });
    setSessionRetention(readRetentionMinutes()).catch(() => undefined);
    onSnapshotChanged(applySnapshot).then((cleanup) => {
      unsubscribe = cleanup;
    });
    onIslandHoverChanged(({ hovering }) => {
      hoveringRef.current = hovering;
      if (hovering) {
        if (!suppressHoverExpandRef.current) {
          expandIsland();
        }
      } else {
        if (phaseRef.current !== "closing") {
          suppressHoverExpandRef.current = false;
        }
        scheduleIdleCollapse();
      }
    }).then((cleanup) => {
      unsubscribeHover = cleanup;
    });
    onIslandOpenRequested(() => {
      suppressHoverExpandRef.current = false;
      expandIsland();
      scheduleIdleCollapse();
    }).then((cleanup) => {
      unsubscribeOpen = cleanup;
    });

    return () => {
      unsubscribe();
      unsubscribeHover();
      unsubscribeOpen();
      clearTransitionWork();
      clearIdleTimer();
    };
  }, []);

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
    try { window.localStorage.setItem(IDLE_INTERVAL_SETTING_KEY, String(idleIntervalSec)); } catch {}
  }, [idleIntervalSec]);

  useEffect(() => {
    try { window.localStorage.setItem(IDLE_DURATION_SETTING_KEY, String(idleDurationSec)); } catch {}
  }, [idleDurationSec]);

  useEffect(() => {
    markSettingsInitialized();
  }, []);

  useEffect(() => {
    if (panelView.kind !== "session") return;
    if (sessions.some((session) => session.sessionId === panelView.sessionId)) return;
    setPanelView({ kind: "home" });
    setSessionRequests([]);
  }, [panelView, sessions]);

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
        deactivateAtoll().catch(() => undefined);
      }
    } finally {
      setBusyDecision(null);
    }
  }

  async function resolveRequest(id: string, decision: Decision, note = "") {
    setBusyDecision(decision);
    try {
      const nextSnapshot = await resolvePermissionRequest(id, decision, note);
      applySnapshot(nextSnapshot);
      if (panelView.kind === "session") {
        const requests = await getSessionRequests(panelView.sessionId);
        setSessionRequests(requests);
      }
      if (nextSnapshot.pendingCount === 0) {
        scheduleIdleCollapse();
        deactivateAtoll().catch(() => undefined);
      }
    } finally {
      setBusyDecision(null);
    }
  }

  function applySnapshot(nextSnapshot: IslandSnapshot) {
    snapshotRef.current = nextSnapshot;
    setSnapshot(nextSnapshot);

    if (nextSnapshot.pendingCount > 0) {
      expandIsland();
    } else {
      scheduleIdleCollapse();
    }
  }

  async function navigateToSession(sessionId: string) {
    setPanelView({ kind: "session", sessionId });
    const requests = await getSessionRequests(sessionId);
    setSessionRequests(requests);
  }

  function navigateBack() {
    setPanelView({ kind: "home" });
  }

  function setPresentationPhase(next: PresentationPhase) {
    phaseRef.current = next;
    setPhase(next);
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

  async function expandIsland() {
    clearIdleTimer();

    const next = beginExpand(phaseRef.current);
    if (next === phaseRef.current) return;
    clearTransitionWork();
    setPresentationPhase(next);

    const idleExpanded =
      snapshotRef.current.pendingCount === 0 &&
      snapshotRef.current.sessions.length === 0;
    lastNativePresentationKeyRef.current = expandedPresentationKey(idleExpanded);

    const nativeTransition = setIslandPresentation(
      "expanded",
      undefined,
      idleExpanded,
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
        setPresentationPhase("compact");
      }
    }, COLLAPSE_ANIMATION_MS);
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
    clearTransitionWork();
    setPresentationPhase(next);
    setPanelView({ kind: "home" });

    lastNativePresentationKeyRef.current = compactPresentationKey(
      collapsedModeRef.current,
      collapsedWindowWidth,
      compactLeftPaneWidth,
    );

    const nativeTransition =
      collapsedModeRef.current === "dormant"
        ? setIslandPresentation("dormant")
        : setIslandPresentation(
            "compact",
            collapsedWindowWidth,
            undefined,
            compactLeftPaneWidth,
          );
    transitionTimerRef.current = window.setTimeout(async () => {
      transitionTimerRef.current = null;
      if (phaseRef.current !== "closing") return;

      try {
        await nativeTransition;
        if (phaseRef.current === "closing") {
          setPresentationPhase(finishCollapse("closing"));
        }
      } catch {
        setPresentationPhase("expanded");
      } finally {
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
    if (!suppressHoverExpandRef.current) {
      expandIsland();
    }
  }

  function handlePointerLeave() {
    hoveringRef.current = false;
    if (phaseRef.current !== "closing") {
      suppressHoverExpandRef.current = false;
    }
    scheduleIdleCollapse();
  }

  function handleIslandClick(event: MouseEvent<HTMLElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
    if ((event.target as HTMLElement).closest("input")) return;
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

  async function handleInstallHooks() {
    setHookBusy(true);
    try {
      const status = await installClaudeHooks();
      setHookStatus(status);
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) applySnapshot(nextSnapshot);
      if (status.installed) {
        collapseIsland(true);
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
      const status = await uninstallClaudeHooks();
      setHookStatus(status);
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) applySnapshot(nextSnapshot);
    } catch {
      // keep previous status
    } finally {
      setHookBusy(false);
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

  function handleSelectAgent(agent: AgentKind) {
    setSelectedAgent(agent);
    if (panelView.kind !== "home") {
      setPanelView({ kind: "home" });
    }
  }

  function handleOpenSettings() {
    setMenuOpen(false);
    setPanelView({ kind: "settings" });
  }

  const isExpanded = phase === "opening" || phase === "expanded";
  const showAgentTabs = isExpanded && tabAgents.length > 1;
  const showPanelAgentTabs =
    isExpanded && panelView.kind === "home" && tabAgents.length > 1;
  const isDormant = !isExpanded && collapsedMode === "dormant";
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
    isExpanded &&
    panelView.kind === "home" &&
    sessions.length === 0 &&
    snapshot.pendingCount === 0;
  const isSubview = isExpanded && panelView.kind !== "home";
  const menuBarLogoSize = isExpanded ? 36 : 34;
  const subviewSession =
    panelView.kind === "session"
      ? sessions.find((session) => session.sessionId === panelView.sessionId)
      : undefined;

  // Keep the native window in sync when compact/expanded layout inputs change.
  // collapseIsland / expandIsland pre-mark the matching key so we do not replay
  // the same native animation right after a user-driven transition finishes.
  useEffect(() => {
    if (phase === "opening" || phase === "closing") {
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
        setIslandPresentation("dormant").catch(() => undefined);
      } else {
        setIslandPresentation(
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
      setIslandPresentation("expanded", undefined, isIdleExpanded).catch(
        () => undefined,
      );
    }
  }, [
    phase,
    collapsedWindowWidth,
    compactLeftPaneWidth,
    collapsedMode,
    isIdleExpanded,
  ]);

  function renderPanel() {
    if (panelView.kind === "session") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      return (
        <SessionChatView
          transcriptPath={session?.transcriptPath ?? null}
          requests={sessionRequests}
        />
      );
    }

    if (panelView.kind === "settings") {
      return (
        <SettingsView
          maxCompactIcons={maxCompactIcons}
          maxCompactIconLimit={maxCompactIconLimit}
          onChangeMaxCompactIcons={(nextValue) =>
            setMaxCompactIcons(clampCompactIconLimit(nextValue, maxCompactIconLimit))
          }
          retentionMinutes={retentionMinutes}
          onChangeRetentionMinutes={(nextValue) =>
            setRetentionMinutes(clampRetentionMinutes(nextValue))
          }
          idleIntervalSec={idleIntervalSec}
          onChangeIdleInterval={(v) => setIdleIntervalSec(clampIdleInterval(v))}
          idleDurationSec={idleDurationSec}
          onChangeIdleDuration={(v) => setIdleDurationSec(clampIdleDuration(v))}
        />
      );
    }

    if (selectedAgentRequest) {
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
          isExpanded={isExpanded}
          onSelectSession={navigateToSession}
          onArchiveSession={handleArchiveSession}
          onPinSession={handlePinSession}
        />
      );
    }

    return (
      <IdleView
        hookStatus={hookStatus}
        hookBusy={hookBusy}
        onInstall={handleInstallHooks}
      />
    );
  }

  return (
    <main className="stage">
      <section
        className={`island is-${phase} ${isExpanded ? "is-expanded" : ""} ${isIdleExpanded ? "is-idle" : ""} ${isDormant ? "is-dormant" : ""} ${snapshot.pendingCount > 0 ? "has-pending" : ""} ${isExpanded && panelView.kind !== "home" ? "is-subview" : ""} ${panelView.kind === "session" ? "is-session-subview" : ""}`}
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
            <span
              className={`atoll-indicator is-app-${appLogoState} ${snapshot.online ? "is-online" : "is-offline"}`}
              title={snapshot.online ? "Listening" : "Offline"}
            >
              <span className="atoll-indicator-inner">
                <AtollLogo
                  size={menuBarLogoSize}
                  activity={atollActivity}
                  idleIntervalSec={idleIntervalSec * 60}
                  idleDurationSec={idleDurationSec * 60}
                />
              </span>
            </span>
            {collapsedMode !== "dormant" && !isExpanded ? (
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
            ) : panelView.kind === "session" ? (
              <SessionSubviewNav
                cwd={subviewSession?.cwd ?? ""}
                onBack={navigateBack}
              />
            ) : panelView.kind === "settings" ? (
              <SettingsSubviewNav onBack={navigateBack} />
            ) : showPanelAgentTabs ? (
              <div className="header-agent-tabs" data-no-drag>
                <AgentTabBar
                  agents={tabAgents}
                  selectedAgent={selectedAgent}
                  pendingCountByAgent={pendingCountByAgent}
                  showTabs={showAgentTabs}
                  online={snapshot.online}
                  onSelectAgent={handleSelectAgent}
                />
              </div>
            ) : null}
          </div>

          {showCompactNotchSpacer ? (
            <span className="header-notch-spacer" aria-hidden="true" />
          ) : null}

          {!isDormant && !isExpanded ? (
            <div className="header-metrics">
              {compactRightSessions.length > 0 ? (
                <CompactSessionStack
                  placement="right"
                  sessions={compactRightSessions}
                  overflowCount={compactRightOverflow}
                  activeRequest={activeRequest}
                  justResolved={justResolved}
                />
              ) : null}
              <TokenCounter
                value={dailyTokenTotal}
                usage={dailyTokens}
                variant="compact"
                sessionCount={sessions.length}
                maxCompactIcons={maxCompactIcons}
                compactTokenLevel={compactHeaderLayout.tokenCompactLevel}
              />
              {snapshot.pendingCount > 0 ? (
                <span className="pending-badge-slot">
                  <span className="pending-badge" aria-label={`${snapshot.pendingCount} pending`}>
                    {snapshot.pendingCount}
                  </span>
                </span>
              ) : null}
            </div>
          ) : null}

          {panelView.kind !== "session" ? (
          <div
            className="header-actions"
            data-no-drag
            ref={menuRef}
            onMouseDown={handleControlMouseDown}
          >
            {!isDormant && isExpanded ? (
              <TokenCounter
                value={dailyTokenTotal}
                usage={dailyTokens}
                variant="expanded"
                sessionCount={sessions.length}
                maxCompactIcons={maxCompactIcons}
              />
            ) : null}
            <button
              className="icon-button"
              type="button"
              onClick={() => collapseIsland(true)}
              aria-label="Collapse Atoll"
              tabIndex={isExpanded ? 0 : -1}
            >
              <ChevronUp size={16} />
            </button>
            <button
              className="icon-button"
              type="button"
              onClick={() => setMenuOpen((open) => !open)}
              aria-label="More options"
              aria-expanded={menuOpen}
              tabIndex={isExpanded ? 0 : -1}
            >
              <Ellipsis size={17} />
            </button>
            {menuOpen ? (
              <div className="more-menu" role="menu">
                {hookStatus?.installed ? (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={handleUninstallHooks}
                    disabled={hookBusy}
                  >
                    <Trash2 size={14} />
                    Uninstall hooks
                  </button>
                ) : (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => { setMenuOpen(false); handleInstallHooks(); }}
                    disabled={hookBusy}
                  >
                    <Download size={14} />
                    Install hooks
                  </button>
                )}
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

        <div className="island-panel">
          {renderPanel()}
        </div>
      </section>
    </main>
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
            <ClawdMascot
              mood={deriveSessionMood(session, activeRequest, justResolved)}
              accent={sessionColor.accent}
              accentDark={sessionColor.accentDark}
              size={18}
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
  online: boolean;
  onSelectAgent: (agent: AgentKind) => void;
}

function AgentTabBar({
  agents,
  selectedAgent,
  pendingCountByAgent,
  showTabs,
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
      <span className={`agent-tab is-static ${agentTone[active]}`} data-no-drag>
        <ClawdMascot
          mood={mood}
          accent={agentMascotAccent[active]}
          accentDark={agentMascotDark[active]}
          size={16}
        />
        <span>{agentLabels[active]}</span>
        {pending > 0 ? <span className="agent-tab-pending">{pending}</span> : null}
      </span>
    );
  }

  return (
    <div className="agent-tabbar" data-no-drag>
      {agents.map((agent) => {
        const pending = pendingCountByAgent[agent] ?? 0;
        const isActive = agent === active;
        const mood: ClawdMood = pending > 0 ? "alert" : "calm";
        return (
          <button
            key={agent}
            type="button"
            className={`agent-tab ${isActive ? "is-active" : ""} ${agentTone[agent]}`}
            onClick={() => onSelectAgent(agent)}
          >
            <ClawdMascot
              mood={mood}
              accent={agentMascotAccent[agent]}
              accentDark={agentMascotDark[agent]}
              size={16}
            />
            <span>{agentLabels[agent]}</span>
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
  onSelectSession: (sessionId: string) => void;
  onArchiveSession: (sessionId: string) => void;
  onPinSession: (sessionId: string, pinned: boolean) => void;
}

function SessionListView({ sessions, activeRequest, justResolved, isExpanded, onSelectSession, onArchiveSession, onPinSession }: SessionListViewProps) {
  const [hoveredSessionId, setHoveredSessionId] = useState<string | null>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isExpanded) {
      setHoveredSessionId(null);
      return;
    }

    let unsubscribe = () => {};

    onIslandHoverChanged(({ hovering, clientX, clientY }) => {
      if (!hovering || clientX == null || clientY == null) {
        if (!hovering) {
          setHoveredSessionId(null);
        }
        return;
      }
      setHoveredSessionId(sessionIdAtClientPoint(clientX, clientY, listRef.current));
    }).then((cleanup) => {
      unsubscribe = cleanup;
    });

    return () => {
      unsubscribe();
    };
  }, [isExpanded, sessions.length]);

  function handleListPointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const item = (event.target as HTMLElement).closest<HTMLElement>("[data-session-id]");
    setHoveredSessionId(item?.dataset.sessionId ?? null);
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
              <button
                className="session-item-main"
                type="button"
                onClick={() => onSelectSession(session.sessionId)}
              >
                <div className="session-item-left">
                  <span className="session-clawd">
                    <ClawdMascot
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
                  </div>
                </div>
                <div className="session-item-trail">
                  {session.pendingCount > 0 ? (
                    <span className="session-pending-badge">{session.pendingCount}</span>
                  ) : null}
                  <ChevronRight size={14} />
                </div>
              </button>
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
  retentionMinutes: number;
  onChangeRetentionMinutes: (value: number) => void;
  idleIntervalSec: number;
  onChangeIdleInterval: (value: number) => void;
  idleDurationSec: number;
  onChangeIdleDuration: (value: number) => void;
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
  retentionMinutes,
  onChangeRetentionMinutes,
  idleIntervalSec,
  onChangeIdleInterval,
  idleDurationSec,
  onChangeIdleDuration,
}: SettingsViewProps) {
  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">Display</span>
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
            label="Auto-archive timeout"
            value={retentionMinutes}
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            unit=" min"
            desc="Minutes before idle sessions are auto-archived. Pinned sessions are exempt."
            onChange={onChangeRetentionMinutes}
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
            <ClawdMascot
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
            <kbd className="decision-kbd" aria-hidden="true">⌫</kbd>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={onApprove}
            disabled={busyDecision !== null}
          >
            <Check size={16} />
            <span>{busyDecision === "approved" ? "Approving..." : "Approve"}</span>
            <kbd className="decision-kbd" aria-hidden="true">↵</kbd>
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
              <kbd className="decision-kbd" aria-hidden="true">⇧↵</kbd>
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
  onBack: () => void;
}

function SessionSubviewNav({ cwd, onBack }: SessionSubviewNavProps) {
  return (
    <div className="session-detail-nav" data-no-drag>
      <button type="button" className="back-button" onClick={onBack}>
        <ArrowLeft size={13} />
        <span>Back</span>
      </button>
      <button
        type="button"
        className="open-terminal-button"
        onClick={() => openInTerminal(cwd)}
      >
        <ExternalLink size={13} />
        <span>Terminal</span>
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
  transcriptPath: string | null;
  requests: PermissionRequest[];
}

function SessionChatView({ transcriptPath, requests }: SessionChatViewProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (transcriptPath) {
      getSessionTranscript(transcriptPath)
        .then(setMessages)
        .catch(() => setMessages([]));
    }
  }, [transcriptPath]);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [messages]);

  return (
    <div className="session-chat">
      <div className="chat-messages" ref={scrollRef}>
        {messages.length === 0 && requests.length === 0 ? (
          <div className="chat-empty">No conversation history.</div>
        ) : null}
        {messages.map((msg, i) => (
          <ChatBubble key={i} message={msg} />
        ))}
      </div>
    </div>
  );
}

/* ─── Idle View ───────────────────────────────────────────────── */

interface IdleViewProps {
  hookStatus: HookStatus | null;
  hookBusy: boolean;
  onInstall: () => void;
}

function IdleView({ hookStatus, hookBusy, onInstall }: IdleViewProps) {
  if (hookStatus && !hookStatus.installed) {
    const scriptMissing = !hookStatus.scriptFound;
    return (
      <div className="idle-view setup-view">
        <div className="setup-card">
          <div className="setup-head">
            <div className="idle-icon setup-icon">
              <Download size={16} />
            </div>
            <div className="setup-copy">
              <h2>Install Claude hooks</h2>
              <p>Forward approval requests and token usage updates to Atoll.</p>
              {scriptMissing ? (
                <p className="setup-warning">
                  Hook script not found. If install fails, reinstall Atoll and try again.
                </p>
              ) : null}
            </div>
          </div>
          <p className="setup-footnote">Once installed, this panel will auto-switch to waiting mode.</p>
          <button
            type="button"
            className="install-button"
            onClick={onInstall}
            disabled={hookBusy}
            data-no-drag
          >
            <Download size={14} />
            <span>{hookBusy ? "Installing..." : "Install hooks"}</span>
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="idle-view">
      <div className="idle-content">
        <span className="idle-dot" />
        <span className="idle-text">Waiting for requests…</span>
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
  const parts = cwd.split("/").filter(Boolean);
  return parts[parts.length - 1] || cwd;
}

function ChatBubble({ message }: { message: ChatMessage }) {
  const text = message.content || (message.toolName ? `Using ${message.toolName}...` : "");
  const hasMarkdown = useMemo(() => /[*_`#\[\]!\n>|]/.test(text), [text]);

  function handleClick(event: MouseEvent<HTMLDivElement>) {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.href) {
      event.preventDefault();
      openUrl(anchor.href);
    }
  }

  return (
    <div className={`chat-bubble ${message.role}`} onClick={handleClick}>
      {message.toolName ? (
        <span className="chat-tool-badge">{message.toolName}</span>
      ) : null}
      {hasMarkdown ? (
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
