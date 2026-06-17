import { FocusEvent, MouseEvent, useEffect, useRef, useState, useMemo } from "react";
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
  Minus,
  Power,
  Settings2,
  Plus,
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
import { AtollLogo } from "./AtollLogo";
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
const DEFAULT_MAX_COMPACT_ICONS = 4;
const MIN_MAX_COMPACT_ICONS = 1;
const MAX_MAX_COMPACT_ICONS = 8;
const DEFAULT_RETENTION_MINUTES = 5;
const MIN_RETENTION_MINUTES = 1;
const MAX_RETENTION_MINUTES = 30;
const COMPACT_WIDTH_MIN = 96;
const COMPACT_WIDTH_BASE = 52;
const COMPACT_WIDTH_ICON_SIZE = 20;
const COMPACT_WIDTH_ICON_GAP = 4;
const COMPACT_WIDTH_OVERFLOW = 32;
const COMPACT_WIDTH_PENDING = 28;
const COMPACT_WIDTH_TOKEN_COUNTER = 72;
const COMPACT_WIDTH_EMPTY = 72;
const ZERO_TOKEN_USAGE: TokenUsage = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
};

function clampCompactIconLimit(value: number) {
  return Math.min(
    MAX_MAX_COMPACT_ICONS,
    Math.max(MIN_MAX_COMPACT_ICONS, Math.round(value)),
  );
}

function readCompactIconLimit() {
  if (typeof window === "undefined") return DEFAULT_MAX_COMPACT_ICONS;
  try {
    const raw = Number(window.localStorage.getItem(COMPACT_ICON_SETTING_KEY));
    if (!Number.isFinite(raw)) return DEFAULT_MAX_COMPACT_ICONS;
    return clampCompactIconLimit(raw);
  } catch {
    return DEFAULT_MAX_COMPACT_ICONS;
  }
}

function clampRetentionMinutes(value: number) {
  return Math.min(
    MAX_RETENTION_MINUTES,
    Math.max(MIN_RETENTION_MINUTES, Math.round(value)),
  );
}

function readRetentionMinutes() {
  if (typeof window === "undefined") return DEFAULT_RETENTION_MINUTES;
  try {
    const raw = Number(window.localStorage.getItem(RETENTION_SETTING_KEY));
    if (!Number.isFinite(raw)) return DEFAULT_RETENTION_MINUTES;
    return clampRetentionMinutes(raw);
  } catch {
    return DEFAULT_RETENTION_MINUTES;
  }
}

function computeCompactWindowWidth(
  sessionCount: number,
  maxCompactIcons: number,
  pendingCount: number,
  tokenTotal: number,
) {
  const pendingWidth = pendingCount > 0 ? COMPACT_WIDTH_PENDING : 0;
  const hasTokenCounter = sessionCount > 0 || pendingCount > 0 || tokenTotal > 0;
  const tokenWidth = hasTokenCounter ? COMPACT_WIDTH_TOKEN_COUNTER : 0;
  if (sessionCount === 0) {
    return Math.max(
      COMPACT_WIDTH_EMPTY,
      Math.ceil(COMPACT_WIDTH_BASE + tokenWidth + pendingWidth),
    );
  }
  const shown = Math.min(sessionCount, maxCompactIcons);
  const iconWidth =
    shown > 0
      ? shown * COMPACT_WIDTH_ICON_SIZE +
        Math.max(0, shown - 1) * COMPACT_WIDTH_ICON_GAP
      : COMPACT_WIDTH_ICON_SIZE;
  const overflowWidth = sessionCount > shown ? COMPACT_WIDTH_OVERFLOW : 0;
  return Math.max(
    COMPACT_WIDTH_MIN,
    Math.ceil(COMPACT_WIDTH_BASE + iconWidth + overflowWidth + pendingWidth + tokenWidth),
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

function applyNotchMetrics(notch: NotchMetrics) {
  if (typeof document === "undefined") return;
  const root = document.documentElement;
  root.style.setProperty("--notch-height", `${Math.max(0, notch.height)}px`);
  root.style.setProperty("--notch-width", `${Math.max(0, notch.width)}px`);
  root.classList.toggle("has-notch", notch.hasNotch);
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
  const [maxCompactIcons, setMaxCompactIcons] = useState<number>(() => readCompactIconLimit());
  const [retentionMinutes, setRetentionMinutes] = useState<number>(() => readRetentionMinutes());
  const [justResolved, setJustResolved] = useState(false);
  const prevPendingRef = useRef(0);
  const selectedAgentRef = useRef<AgentKind | null>(null);
  selectedAgentRef.current = selectedAgent;

  const activeRequest = snapshot.activeRequest;
  const sessions = snapshot.sessions;
  const dailyTokens = snapshot.dailyTokens ?? ZERO_TOKEN_USAGE;
  const dailyTokenTotal = dailyTokens.inputTokens + dailyTokens.outputTokens;
  const compactWindowWidth = useMemo(
    () =>
      computeCompactWindowWidth(
        sessions.length,
        maxCompactIcons,
        snapshot.pendingCount,
        dailyTokenTotal,
      ),
    [sessions.length, maxCompactIcons, snapshot.pendingCount, dailyTokenTotal],
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
      .then(applyNotchMetrics)
      .catch(() => undefined);
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
    if (panelView.kind !== "session") return;
    if (sessions.some((session) => session.sessionId === panelView.sessionId)) return;
    setPanelView({ kind: "home" });
    setSessionRequests([]);
  }, [panelView, sessions]);

  useEffect(() => {
    if (phase !== "compact") return;
    if (collapsedMode === "dormant") {
      setIslandPresentation("dormant").catch(() => undefined);
    } else {
      setIslandPresentation("compact", compactWindowWidth).catch(() => undefined);
    }
  }, [phase, compactWindowWidth, collapsedMode]);

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

    const nativeTransition = setIslandPresentation("expanded");
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
    if (releaseFocus) {
      suppressHoverExpandRef.current = true;
      focusedRef.current = false;
      if (document.activeElement instanceof HTMLElement) {
        document.activeElement.blur();
      }
    }

    const next = beginCollapse(phaseRef.current);
    if (next === phaseRef.current) return;
    clearTransitionWork();
    setPresentationPhase(next);
    setPanelView({ kind: "home" });

    const nativeTransition =
      collapsedModeRef.current === "dormant"
        ? setIslandPresentation("dormant")
        : setIslandPresentation("compact", compactWindowWidth);
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

  function handleOpenSettings() {
    setMenuOpen(false);
    setPanelView({ kind: "settings" });
  }

  const isExpanded = phase === "opening" || phase === "expanded";
  const showAgentTabs = isExpanded && tabAgents.length > 1;
  const isDormant = !isExpanded && collapsedMode === "dormant";

  function renderPanel() {
    if (panelView.kind === "session") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      return (
        <SessionChatView
          sessionId={panelView.sessionId}
          cwd={session?.cwd ?? ""}
          transcriptPath={session?.transcriptPath ?? null}
          requests={sessionRequests}
          onBack={navigateBack}
        />
      );
    }

    if (panelView.kind === "settings") {
      return (
        <SettingsView
          maxCompactIcons={maxCompactIcons}
          onChangeMaxCompactIcons={(nextValue) =>
            setMaxCompactIcons(clampCompactIconLimit(nextValue))
          }
          retentionMinutes={retentionMinutes}
          onChangeRetentionMinutes={(nextValue) =>
            setRetentionMinutes(clampRetentionMinutes(nextValue))
          }
          onBack={navigateBack}
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
          onSelectSession={navigateToSession}
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
        className={`island is-${phase} ${isExpanded ? "is-expanded" : ""} ${isDormant ? "is-dormant" : ""} ${snapshot.pendingCount > 0 ? "has-pending" : ""}`}
        aria-label="Atoll"
        tabIndex={0}
        onClick={handleIslandClick}
        onPointerEnter={handlePointerEnter}
        onPointerLeave={handlePointerLeave}
        onFocusCapture={handleIslandFocus}
        onBlurCapture={handleIslandBlur}
      >
        {collapsedMode === "dormant" && (
          <span
            className={`atoll-indicator ${snapshot.online ? "is-online" : "is-offline"}`}
            title={snapshot.online ? "Listening" : "Offline"}
          >
            <AtollLogo size={16} />
          </span>
        )}

        <header
          className="island-header"
          onMouseDown={startWindowDrag}
          title={isExpanded ? "Drag window" : "Hover to open"}
        >
          <div className="header-main">
            {collapsedMode !== "dormant" && (
              <>
                <span
                  className={`listener-dot ${snapshot.online ? "online" : ""}`}
                  title={snapshot.online ? "Listening" : "Offline"}
                />
                {isExpanded ? (
                  <AgentTabBar
                    agents={tabAgents}
                    selectedAgent={selectedAgent}
                    pendingCountByAgent={pendingCountByAgent}
                    showTabs={showAgentTabs}
                    online={snapshot.online}
                    onSelectAgent={(agent) => {
                      setSelectedAgent(agent);
                      if (panelView.kind !== "home") {
                        setPanelView({ kind: "home" });
                      }
                    }}
                  />
                ) : (
                  <CompactSessionStack
                    sessions={sessions}
                    maxCompactIcons={maxCompactIcons}
                    online={snapshot.online}
                    activeRequest={activeRequest}
                    justResolved={justResolved}
                  />
                )}
              </>
            )}
          </div>

          {(!isDormant && !isExpanded) || snapshot.pendingCount > 0 ? (
            <div className="header-metrics">
              {!isDormant && !isExpanded ? (
                <TokenCounter value={dailyTokenTotal} usage={dailyTokens} />
              ) : null}
              {snapshot.pendingCount > 0 ? (
                <span className="pending-badge-slot">
                  <span className="pending-badge" aria-label={`${snapshot.pendingCount} pending`}>
                    {snapshot.pendingCount}
                  </span>
                </span>
              ) : null}
            </div>
          ) : null}

          <div
            className="header-actions"
            data-no-drag
            ref={menuRef}
            onMouseDown={handleControlMouseDown}
          >
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
        </header>

        <div className="island-panel">
          {renderPanel()}
        </div>
      </section>
    </main>
  );
}

/* ─── Compact Header ───────────────────────────────────────────── */

function formatCompactTokenCount(value: number): string {
  const abs = Math.abs(value);
  const sign = value < 0 ? "-" : "";
  if (abs >= 1_000_000) {
    const fractionDigits = abs >= 100_000_000 ? 0 : abs >= 10_000_000 ? 1 : 2;
    return `${sign}${(abs / 1_000_000).toFixed(fractionDigits)}M`;
  }
  if (abs >= 1_000) {
    const fractionDigits = abs >= 100_000 ? 1 : 2;
    return `${sign}${(abs / 1_000).toFixed(fractionDigits)}K`;
  }
  return value.toLocaleString();
}

function tokenCounterTitle(value: number, usage: TokenUsage): string {
  return [
    `Today tokens ${value.toLocaleString()}`,
    `input ${usage.inputTokens.toLocaleString()}`,
    `output ${usage.outputTokens.toLocaleString()}`,
    `cache-read ${usage.cacheReadTokens.toLocaleString()}`,
    `cache-write ${usage.cacheCreationTokens.toLocaleString()}`,
  ].join(" · ");
}

interface TokenCounterProps {
  value: number;
  usage: TokenUsage;
}

function TokenCounter({ value, usage }: TokenCounterProps) {
  const [displayValue, setDisplayValue] = useState(value);
  const [isUpdating, setIsUpdating] = useState(false);
  const [deltaText, setDeltaText] = useState<string | null>(null);
  const displayRef = useRef(value);
  const animatedValueRef = useRef(value);
  const targetRef = useRef(value);
  const lastFrameAtRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);
  const pulseTimerRef = useRef<number | null>(null);
  const deltaTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (deltaTimerRef.current !== null) {
      window.clearTimeout(deltaTimerRef.current);
      deltaTimerRef.current = null;
    }

    const incomingDelta = value - targetRef.current;
    targetRef.current = value;
    if (incomingDelta > 0) {
      setDeltaText(`+${formatCompactTokenCount(incomingDelta)}`);
      deltaTimerRef.current = window.setTimeout(() => {
        setDeltaText(null);
        deltaTimerRef.current = null;
      }, 920);
    } else {
      setDeltaText(null);
    }

    if (pulseTimerRef.current !== null) {
      window.clearTimeout(pulseTimerRef.current);
      pulseTimerRef.current = null;
    }
    setIsUpdating(true);

    const animate = (now: number) => {
      const previousFrameAt = lastFrameAtRef.current ?? now;
      lastFrameAtRef.current = now;
      const dt = Math.min(0.06, Math.max(0.001, (now - previousFrameAt) / 1000));
      const current = animatedValueRef.current;
      const target = targetRef.current;
      const diff = target - current;
      const distance = Math.abs(diff);

      if (distance < 0.5) {
        const settled = target;
        animatedValueRef.current = settled;
        const settledInt = Math.round(settled);
        if (settledInt !== displayRef.current) {
          displayRef.current = settledInt;
          setDisplayValue(settledInt);
        }
        frameRef.current = null;
        lastFrameAtRef.current = null;
        pulseTimerRef.current = window.setTimeout(() => {
          setIsUpdating(false);
          pulseTimerRef.current = null;
        }, 220);
        return;
      }

      // Keep the counter feeling "alive": larger gaps roll faster but still
      // increment smoothly instead of jumping straight to the final value.
      const speedPerSecond = Math.min(22_000, Math.max(240, distance * 3.8));
      const step = Math.max(1, speedPerSecond * dt);
      const next = current + Math.sign(diff) * Math.min(distance, step);

      animatedValueRef.current = next;
      const nextInt = Math.round(next);
      if (nextInt !== displayRef.current) {
        displayRef.current = nextInt;
        setDisplayValue(nextInt);
      }

      frameRef.current = window.requestAnimationFrame(animate);
    };

    if (frameRef.current === null) {
      frameRef.current = window.requestAnimationFrame(animate);
    }

    return () => {
      if (frameRef.current !== null) {
        window.cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
        lastFrameAtRef.current = null;
      }
      if (pulseTimerRef.current !== null) {
        window.clearTimeout(pulseTimerRef.current);
        pulseTimerRef.current = null;
      }
      if (deltaTimerRef.current !== null) {
        window.clearTimeout(deltaTimerRef.current);
        deltaTimerRef.current = null;
      }
    };
  }, [value]);

  return (
    <span className="token-counter-wrap">
      <span
        className={`token-counter ${isUpdating ? "is-updating" : ""}`}
        aria-label={tokenCounterTitle(displayValue, usage)}
        title={tokenCounterTitle(displayValue, usage)}
      >
        {formatCompactTokenCount(displayValue)}
      </span>
      {deltaText ? (
        <span className="token-counter-delta" aria-hidden="true">
          {deltaText}
        </span>
      ) : null}
    </span>
  );
}

interface CompactSessionStackProps {
  sessions: SessionSummary[];
  maxCompactIcons: number;
  online: boolean;
  activeRequest: PermissionRequest | null;
  justResolved: boolean;
}

function CompactSessionStack({
  sessions,
  maxCompactIcons,
  online,
  activeRequest,
  justResolved,
}: CompactSessionStackProps) {
  const shown = sessions.slice(0, maxCompactIcons);
  const overflow = sessions.length - shown.length;
  if (shown.length === 0) {
    return (
      <span className="compact-session-stack is-empty" aria-hidden="true">
        <span
          className={`compact-empty-logo ${online ? "is-online" : "is-offline"}`}
        >
          <AtollLogo size={22} />
        </span>
      </span>
    );
  }

  return (
    <span className="compact-session-stack" aria-hidden="true">
      {shown.map((session) => {
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
              size={16}
            />
          </span>
        );
      })}
      {overflow > 0 ? (
        <span className="compact-session-overflow">✖️{overflow}</span>
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
          size={14}
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
              size={14}
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

interface SessionListViewProps {
  sessions: SessionSummary[];
  activeRequest: PermissionRequest | null;
  justResolved: boolean;
  onSelectSession: (sessionId: string) => void;
}

function SessionListView({ sessions, activeRequest, justResolved, onSelectSession }: SessionListViewProps) {
  return (
    <div className="session-list-view">
      <div className="session-list-header">
        <Layers size={12} />
        <span>{sessions.length} session{sessions.length > 1 ? "s" : ""}</span>
      </div>
      <div className="session-list">
        {sessions.map((session) => {
          const sessionColor = getSessionColor(session.sessionId);
          return (
            <button
              key={session.sessionId}
              className="session-item"
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
          );
        })}
      </div>
    </div>
  );
}

/* ─── Settings View ────────────────────────────────────────────── */

interface SettingsViewProps {
  maxCompactIcons: number;
  onChangeMaxCompactIcons: (value: number) => void;
  retentionMinutes: number;
  onChangeRetentionMinutes: (value: number) => void;
  onBack: () => void;
}

function SettingsView({
  maxCompactIcons,
  onChangeMaxCompactIcons,
  retentionMinutes,
  onChangeRetentionMinutes,
  onBack,
}: SettingsViewProps) {
  const canDecrease = maxCompactIcons > MIN_MAX_COMPACT_ICONS;
  const canIncrease = maxCompactIcons < MAX_MAX_COMPACT_ICONS;
  const canDecreaseRetention = retentionMinutes > MIN_RETENTION_MINUTES;
  const canIncreaseRetention = retentionMinutes < MAX_RETENTION_MINUTES;

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-header">
        <button type="button" className="back-button" onClick={onBack}>
          <ArrowLeft size={13} />
          <span>Back</span>
        </button>
        <span className="settings-header-title">
          <Settings2 size={14} />
          <span>Display settings</span>
        </span>
      </div>
      <div className="settings-item">
        <div className="settings-item-text">
          <span className="settings-item-title">Folded icon limit</span>
          <span className="settings-item-desc">
            Max session icons in compact mode, overflow shows as ✖️N.
          </span>
        </div>
        <div className="settings-stepper">
          <button
            type="button"
            className="settings-stepper-btn"
            onClick={() => onChangeMaxCompactIcons(maxCompactIcons - 1)}
            disabled={!canDecrease}
            aria-label="Decrease compact icon limit"
          >
            <Minus size={12} />
          </button>
          <input
            type="number"
            min={MIN_MAX_COMPACT_ICONS}
            max={MAX_MAX_COMPACT_ICONS}
            value={maxCompactIcons}
            className="settings-stepper-input"
            onChange={(event) => {
              const parsed = Number(event.target.value);
              if (!Number.isFinite(parsed)) return;
              onChangeMaxCompactIcons(parsed);
            }}
          />
          <button
            type="button"
            className="settings-stepper-btn"
            onClick={() => onChangeMaxCompactIcons(maxCompactIcons + 1)}
            disabled={!canIncrease}
            aria-label="Increase compact icon limit"
          >
            <Plus size={12} />
          </button>
        </div>
      </div>
      <div className="settings-item">
        <div className="settings-item-text">
          <span className="settings-item-title">Session retention</span>
          <span className="settings-item-desc">
            Minutes a session stays visible after all requests are resolved.
          </span>
        </div>
        <div className="settings-stepper">
          <button
            type="button"
            className="settings-stepper-btn"
            onClick={() => onChangeRetentionMinutes(retentionMinutes - 1)}
            disabled={!canDecreaseRetention}
            aria-label="Decrease session retention"
          >
            <Minus size={12} />
          </button>
          <input
            type="number"
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            value={retentionMinutes}
            className="settings-stepper-input"
            onChange={(event) => {
              const parsed = Number(event.target.value);
              if (!Number.isFinite(parsed)) return;
              onChangeRetentionMinutes(parsed);
            }}
          />
          <button
            type="button"
            className="settings-stepper-btn"
            onClick={() => onChangeRetentionMinutes(retentionMinutes + 1)}
            disabled={!canIncreaseRetention}
            aria-label="Increase session retention"
          >
            <Plus size={12} />
          </button>
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
              size={16}
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

/* ─── Session Chat View ───────────────────────────────────────── */

interface SessionChatViewProps {
  sessionId: string;
  cwd: string;
  transcriptPath: string | null;
  requests: PermissionRequest[];
  onBack: () => void;
}

function SessionChatView({ sessionId, cwd, transcriptPath, requests, onBack }: SessionChatViewProps) {
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
      <div className="session-detail-nav">
        <button type="button" className="back-button" onClick={onBack}>
          <ArrowLeft size={13} />
          <span>Back</span>
        </button>
        <span className="session-chat-title">{sessionDisplayName(cwd || sessionId)}</span>
        <button
          type="button"
          className="open-terminal-button"
          onClick={() => openInTerminal(cwd)}
          data-no-drag
        >
          <ExternalLink size={13} />
          <span>Terminal</span>
        </button>
      </div>

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
