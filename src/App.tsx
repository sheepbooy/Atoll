import { FocusEvent, MouseEvent, useEffect, useRef, useState, useMemo } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import ReactMarkdown from "react-markdown";
import {
  Archive,
  ArrowLeft,
  Check,
  CheckCheck,
  ChevronRight,
  ChevronUp,
  Circle,
  ClipboardCheck,
  Code2,
  Download,
  Ellipsis,
  Layers,
  Power,
  SendHorizonal,
  ShieldAlert,
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
import {
  getSnapshot,
  getSessionRequests,
  getSessionTranscript,
  IslandSnapshot,
  onIslandHoverChanged,
  onIslandOpenRequested,
  onSnapshotChanged,
  PermissionRequest,
  SessionSummary,
  ChatMessage,
  HookStatus,
  archiveAllResolved,
  quitAtoll,
  resolvePermissionRequest,
  setIslandPresentation,
  setSessionAutoApprove,
  getClaudeHookStatus,
  installClaudeHooks,
  uninstallClaudeHooks,
} from "./tauri";

type Decision = "approved" | "denied";
type PanelView = { kind: "home" } | { kind: "session"; sessionId: string };

const initialSnapshot: IslandSnapshot = {
  online: false,
  pendingCount: 0,
  archivedCount: 0,
  activeRequest: null,
  recent: [],
  sessions: [],
};

const agentLabels: Record<PermissionRequest["agent"], string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  other: "Agent",
};

const agentTone: Record<PermissionRequest["agent"], string> = {
  claude: "coral",
  codex: "cyan",
  gemini: "lime",
  other: "neutral",
};

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

  const activeRequest = snapshot.activeRequest;
  const sessions = snapshot.sessions;

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
      if (!snapshotRef.current.activeRequest || busyRef.current) return;
      if (menuOpenRef.current) return;
      if ((event.target as HTMLElement).tagName === "INPUT" || (event.target as HTMLElement).tagName === "TEXTAREA") return;

      if (event.key === "Enter" && event.shiftKey) {
        event.preventDefault();
        resolveActive("approved", true);
      } else if (event.key === "Enter") {
        event.preventDefault();
        resolveActive("approved");
      } else if (event.key === "Backspace" || event.key === "Delete") {
        event.preventDefault();
        resolveActive("denied");
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  async function resolveActive(decision: Decision, alwaysApprove = false, note = "") {
    if (!activeRequest) return;

    setBusyDecision(decision);
    try {
      if (alwaysApprove) {
        await setSessionAutoApprove(activeRequest.session, true);
      }
      const nextSnapshot = await resolvePermissionRequest(activeRequest.id, decision, note);
      applySnapshot(nextSnapshot);
      if (nextSnapshot.pendingCount === 0) {
        collapseIsland(true);
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

    const nativeTransition = setIslandPresentation("compact");
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
    if (
      hoveringRef.current ||
      focusedRef.current ||
      snapshotRef.current.pendingCount > 0
    ) {
      return;
    }

    idleTimerRef.current = window.setTimeout(() => {
      idleTimerRef.current = null;
      if (
        !hoveringRef.current &&
        !focusedRef.current &&
        snapshotRef.current.pendingCount === 0
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

  const isExpanded = phase === "opening" || phase === "expanded";
  const agent = activeRequest?.agent;

  function renderPanel() {
    if (panelView.kind === "session") {
      const session = sessions.find((s) => s.sessionId === panelView.sessionId);
      return (
        <SessionChatView
          sessionId={panelView.sessionId}
          transcriptPath={session?.transcriptPath ?? null}
          requests={sessionRequests}
          busyDecision={busyDecision}
          onReply={(note) => {
            const pending = sessionRequests.find((r) => r.status === "pending");
            if (pending) {
              resolveRequest(pending.id, "denied", note);
            }
          }}
          onBack={navigateBack}
        />
      );
    }

    if (activeRequest) {
      return (
        <ApprovalCard
          request={activeRequest}
          busyDecision={busyDecision}
          sessions={sessions}
          onApprove={() => resolveActive("approved")}
          onDeny={() => resolveActive("denied")}
          onAlwaysApprove={() => resolveActive("approved", true)}
          onViewSession={navigateToSession}
        />
      );
    }

    if (sessions.length > 0) {
      return (
        <SessionListView
          sessions={sessions}
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
        className={`island is-${phase} ${isExpanded ? "is-expanded" : ""}`}
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
          title={isExpanded ? "Drag Atoll" : "Hover to open Atoll"}
        >
          <span className="agent-slot">
            <span className={`agent-dot ${agent ? agentTone[agent] : "idle"}`}>
              {activeRequest ? <ShieldAlert size={15} /> : <Circle size={11} />}
            </span>
          </span>

          <span className="header-copy">
            <span className="header-title">
              {activeRequest
                ? `${agentLabels[activeRequest.agent]} approval`
                : "Atoll"}
            </span>
            <span className="header-meta">
              <span className={`listener-dot ${snapshot.online ? "online" : ""}`} />
              {snapshot.online ? "Listening" : "Offline"}
              {sessions.length > 0 ? (
                <>
                  <span className="meta-divider">·</span>
                  {sessions.length} session{sessions.length > 1 ? "s" : ""}
                </>
              ) : activeRequest ? (
                <>
                  <span className="meta-divider">·</span>
                  {timeAgo(activeRequest.requestedAt)}
                </>
              ) : null}
            </span>
          </span>

          {snapshot.pendingCount > 0 ? (
            <span className="pending-badge-slot">
              <span className="pending-badge" aria-label={`${snapshot.pendingCount} pending`}>
                {snapshot.pendingCount}
              </span>
            </span>
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
                    <Trash2 size={15} />
                    Uninstall hooks
                  </button>
                ) : (
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => { setMenuOpen(false); handleInstallHooks(); }}
                    disabled={hookBusy}
                  >
                    <Download size={15} />
                    Install hooks
                  </button>
                )}
                <button
                  type="button"
                  role="menuitem"
                  onClick={handleArchiveAll}
                >
                  <Archive size={15} />
                  Archive all
                </button>
                <button
                  type="button"
                  role="menuitem"
                  className="danger"
                  onClick={handleQuit}
                >
                  <Power size={15} />
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

interface SessionListViewProps {
  sessions: SessionSummary[];
  onSelectSession: (sessionId: string) => void;
}

function SessionListView({ sessions, onSelectSession }: SessionListViewProps) {
  return (
    <div className="session-list-view">
      <div className="session-list-header">
        <Layers size={13} />
        <span>{sessions.length} session{sessions.length > 1 ? "s" : ""}</span>
      </div>
      <div className="session-list">
        {sessions.map((session) => (
          <button
            key={session.sessionId}
            className="session-item"
            type="button"
            onClick={() => onSelectSession(session.sessionId)}
          >
            <div className="session-item-info">
              <span className="session-item-name">
                {sessionDisplayName(session.cwd)}
              </span>
              <span className="session-item-meta">
                {session.cwd}
                <span className="meta-divider">·</span>
                {timeAgo(session.lastActivity)}
              </span>
            </div>
            <div className="session-item-trail">
              {session.pendingCount > 0 ? (
                <span className="session-pending-badge">{session.pendingCount}</span>
              ) : null}
              <ChevronRight size={14} />
            </div>
          </button>
        ))}
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

  return (
    <div className="approval-view">
      <div className="request-main">
        <div className="request-kicker">
          <span>
            <Code2 size={13} />
            Command request
          </span>
          <span>{agentLabels[request.agent]}</span>
        </div>
        <code className="command-block">{request.command}</code>
        {request.detail ? <p className="request-detail">{request.detail}</p> : null}
        <div className="cwd-line" title={request.cwd}>
          {request.cwd}
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
            <X size={17} />
            <span>{busyDecision === "denied" ? "Denying" : "Deny"}</span>
          </button>
          <button
            className="decision-button approve"
            type="button"
            onClick={onApprove}
            disabled={busyDecision !== null}
          >
            <Check size={17} />
            <span>{busyDecision === "approved" ? "Approving" : "Approve"}</span>
          </button>
          {request.supportsAlways ? (
            <button
              className="decision-button always-approve"
              type="button"
              onClick={onAlwaysApprove}
              disabled={busyDecision !== null}
              title="Approve this and all future requests for this session"
            >
              <CheckCheck size={17} />
              <span>Always</span>
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

interface SessionChatViewProps {
  sessionId: string;
  transcriptPath: string | null;
  requests: PermissionRequest[];
  busyDecision: Decision | null;
  onReply: (note: string) => void;
  onBack: () => void;
}

function SessionChatView({ sessionId, transcriptPath, requests, busyDecision, onReply, onBack }: SessionChatViewProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [replyText, setReplyText] = useState("");
  const scrollRef = useRef<HTMLDivElement>(null);
  const pendingRequest = requests.find((r) => r.status === "pending") ?? null;

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

  function handleSendReply() {
    const text = replyText.trim();
    if (!text || !pendingRequest) return;
    setReplyText("");
    onReply(text);
  }

  return (
    <div className="session-chat">
      <div className="session-detail-nav">
        <button type="button" className="back-button" onClick={onBack}>
          <ArrowLeft size={14} />
          <span>Back</span>
        </button>
        <span className="session-chat-title">{sessionDisplayName(requests[0]?.cwd || sessionId)}</span>
      </div>

      <div className="chat-messages" ref={scrollRef}>
        {messages.length === 0 && requests.length === 0 ? (
          <div className="chat-empty">No conversation history.</div>
        ) : null}
        {messages.map((msg, i) => (
          <ChatBubble key={i} message={msg} />
        ))}
      </div>

      <div className="conversation-reply" data-no-drag>
        <input
          type="text"
          className="reply-input"
          placeholder={pendingRequest ? "Reply to agent..." : "No pending request"}
          value={replyText}
          disabled={!pendingRequest || busyDecision !== null}
          onChange={(e) => setReplyText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey && replyText.trim()) {
              e.preventDefault();
              e.stopPropagation();
              handleSendReply();
            }
          }}
        />
        <button
          type="button"
          className="reply-send-button"
          disabled={!replyText.trim() || !pendingRequest || busyDecision !== null}
          onClick={handleSendReply}
          title="Send reply (deny with message)"
        >
          <SendHorizonal size={14} />
        </button>
      </div>
    </div>
  );
}

interface IdleViewProps {
  hookStatus: HookStatus | null;
  hookBusy: boolean;
  onInstall: () => void;
}

function IdleView({ hookStatus, hookBusy, onInstall }: IdleViewProps) {
  if (hookStatus && !hookStatus.installed) {
    return (
      <div className="idle-view setup-view">
        <div className="idle-icon setup-icon">
          <Download size={21} />
        </div>
        <div>
          <h1>Setup required</h1>
          <p>Install Claude Code hooks to forward approval requests to Atoll.</p>
        </div>
        <button
          type="button"
          className="install-button"
          onClick={onInstall}
          disabled={hookBusy}
          data-no-drag
        >
          <Download size={15} />
          <span>{hookBusy ? "Installing..." : "Install hooks"}</span>
        </button>
      </div>
    );
  }

  return (
    <div className="idle-view">
      <div className="idle-icon">
        <ClipboardCheck size={21} />
      </div>
      <div>
        <h1>All clear</h1>
        <p>Agent approvals will appear here when they need your attention.</p>
      </div>
    </div>
  );
}

function sessionDisplayName(cwd: string) {
  const parts = cwd.split("/").filter(Boolean);
  return parts[parts.length - 1] || cwd;
}

function ChatBubble({ message }: { message: ChatMessage }) {
  const text = message.content || (message.toolName ? `Using ${message.toolName}...` : "");
  const hasMarkdown = useMemo(() => /[*_`#\[\]!\n>-]/.test(text), [text]);

  return (
    <div className={`chat-bubble ${message.role}`}>
      {message.toolName ? (
        <span className="chat-tool-badge">{message.toolName}</span>
      ) : null}
      {hasMarkdown ? (
        <div className="chat-bubble-md">
          <ReactMarkdown>{text}</ReactMarkdown>
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
