import { FocusEvent, MouseEvent, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Check,
  ChevronUp,
  Circle,
  ClipboardCheck,
  Code2,
  Ellipsis,
  Power,
  ShieldAlert,
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
  IslandSnapshot,
  onIslandHoverChanged,
  onIslandOpenRequested,
  onSnapshotChanged,
  PermissionRequest,
  quitAtoll,
  resolvePermissionRequest,
  setIslandPresentation,
} from "./tauri";

type Decision = "approved" | "denied";

const initialSnapshot: IslandSnapshot = {
  online: false,
  pendingCount: 0,
  activeRequest: null,
  recent: [],
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

  const activeRequest = snapshot.activeRequest;

  useEffect(() => {
    let unsubscribe: () => void = () => undefined;
    let unsubscribeHover: () => void = () => undefined;
    let unsubscribeOpen: () => void = () => undefined;

    getSnapshot()
      .then(applySnapshot)
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
        suppressHoverExpandRef.current = false;
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

  async function resolveActive(decision: Decision) {
    if (!activeRequest) return;

    setBusyDecision(decision);
    try {
      const nextSnapshot = await resolvePermissionRequest(activeRequest.id, decision);
      applySnapshot(nextSnapshot);
      if (nextSnapshot.pendingCount === 0) {
        collapseIsland(true);
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
    suppressHoverExpandRef.current = false;
    scheduleIdleCollapse();
  }

  function handleIslandClick(event: MouseEvent<HTMLElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
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

  async function handleQuit() {
    setMenuOpen(false);
    await quitAtoll().catch(() => undefined);
  }

  const isExpanded = phase === "opening" || phase === "expanded";
  const showExpandedActions = phase !== "compact";
  const agent = activeRequest?.agent;

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
          <span className={`agent-dot ${agent ? agentTone[agent] : "idle"}`}>
            {activeRequest ? <ShieldAlert size={15} /> : <Circle size={11} />}
          </span>

          <span className="header-copy">
            <span className="header-title">
              {activeRequest ? `${agentLabels[activeRequest.agent]} approval` : "Atoll"}
            </span>
            <span className="header-meta">
              <span className={`listener-dot ${snapshot.online ? "online" : ""}`} />
              {snapshot.online ? "Listening" : "Offline"}
              {activeRequest ? (
                <>
                  <span className="meta-divider">·</span>
                  {timeAgo(activeRequest.requestedAt)}
                </>
              ) : null}
            </span>
          </span>

          {snapshot.pendingCount > 0 ? (
            <span className="pending-badge" aria-label={`${snapshot.pendingCount} pending`}>
              {snapshot.pendingCount}
            </span>
          ) : null}

          {showExpandedActions ? (
            <div className="header-actions" data-no-drag ref={menuRef}>
              <button
                className="icon-button"
                type="button"
                onClick={() => collapseIsland(true)}
                aria-label="Collapse Atoll"
              >
                <ChevronUp size={16} />
              </button>
              <button
                className="icon-button"
                type="button"
                onClick={() => setMenuOpen((open) => !open)}
                aria-label="More options"
                aria-expanded={menuOpen}
              >
                <Ellipsis size={17} />
              </button>
              {menuOpen ? (
                <div className="more-menu" role="menu">
                  <button type="button" role="menuitem" onClick={handleQuit}>
                    <Power size={15} />
                    Quit Atoll
                  </button>
                </div>
              ) : null}
            </div>
          ) : null}
        </header>

        <div className="island-panel">
          {activeRequest ? (
            <ApprovalView
              request={activeRequest}
              busyDecision={busyDecision}
              onApprove={() => resolveActive("approved")}
              onDeny={() => resolveActive("denied")}
            />
          ) : (
            <IdleView />
          )}
        </div>
      </section>
    </main>
  );
}

interface ApprovalViewProps {
  request: PermissionRequest;
  busyDecision: Decision | null;
  onApprove: () => void;
  onDeny: () => void;
}

function ApprovalView({ request, busyDecision, onApprove, onDeny }: ApprovalViewProps) {
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

      <div className="decision-row">
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
      </div>
    </div>
  );
}

function IdleView() {
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

function timeAgo(isoDate: string) {
  const elapsedSeconds = Math.max(1, Math.floor((Date.now() - new Date(isoDate).getTime()) / 1000));

  if (elapsedSeconds < 60) return `${elapsedSeconds}s ago`;

  const elapsedMinutes = Math.floor(elapsedSeconds / 60);
  if (elapsedMinutes < 60) return `${elapsedMinutes}m ago`;

  const elapsedHours = Math.floor(elapsedMinutes / 60);
  return `${elapsedHours}h ago`;
}
