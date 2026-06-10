import { FocusEvent, MouseEvent, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Bell,
  Check,
  Circle,
  ClipboardCheck,
  Code2,
  Play,
  ShieldAlert,
  TerminalSquare,
  X,
} from "lucide-react";
import {
  getSnapshot,
  IslandSnapshot,
  onIslandHoverChanged,
  onIslandOpenRequested,
  PermissionRequest,
  resolvePermissionRequest,
  setIslandPresentation,
  simulatePermissionRequest,
  onSnapshotChanged,
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

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

export function App() {
  const [snapshot, setSnapshot] = useState<IslandSnapshot>(initialSnapshot);
  const snapshotRef = useRef(initialSnapshot);
  const [expanded, setExpanded] = useState(false);
  const expandedRef = useRef(false);
  const hoveringRef = useRef(false);
  const collapseTimerRef = useRef<number | null>(null);
  const openTimerRef = useRef<number | null>(null);
  const [busyDecision, setBusyDecision] = useState<Decision | null>(null);

  const activeRequest = snapshot.activeRequest;
  const pendingRequests = useMemo(
    () => snapshot.recent.filter((request) => request.status === "pending"),
    [snapshot.recent],
  );

  useEffect(() => {
    let unsubscribe: () => void = () => undefined;
    let unsubscribeHover: () => void = () => undefined;
    let unsubscribeOpen: () => void = () => undefined;

    getSnapshot()
      .then((nextSnapshot) => {
        applySnapshot(nextSnapshot);
      })
      .catch(() => undefined);
    onSnapshotChanged(applySnapshot).then((cleanup) => {
      unsubscribe = cleanup;
    });
    onIslandHoverChanged(({ hovering }) => {
      hoveringRef.current = hovering;
      if (hovering) {
        expandIsland();
      } else {
        collapseIdleIsland();
      }
    }).then((cleanup) => {
      unsubscribeHover = cleanup;
    });
    onIslandOpenRequested(() => {
      expandIsland();
      scheduleIdleCollapse();
    }).then((cleanup) => {
      unsubscribeOpen = cleanup;
    });

    return () => {
      unsubscribe();
      unsubscribeHover();
      unsubscribeOpen();
      clearCollapseTimer();
      clearOpenTimer();
    };
  }, []);

  async function resolveActive(decision: Decision) {
    if (!activeRequest) return;

    setBusyDecision(decision);
    try {
      applySnapshot(await resolvePermissionRequest(activeRequest.id, decision));
    } finally {
      setBusyDecision(null);
    }
  }

  async function createDemoRequest() {
    applySnapshot(await simulatePermissionRequest());
    expandIsland();
  }

  function applySnapshot(nextSnapshot: IslandSnapshot) {
    snapshotRef.current = nextSnapshot;
    setSnapshot(nextSnapshot);

    if (nextSnapshot.pendingCount > 0) {
      expandIsland();
      return;
    }

    if (hoveringRef.current) {
      return;
    }

    collapseIsland();
  }

  function expandIsland() {
    clearCollapseTimer();
    if (expandedRef.current) return;
    expandedRef.current = true;
    setIslandPresentation("expanded")
      .catch(() => undefined)
      .finally(() => {
        if (expandedRef.current) {
          setExpanded(true);
        }
      });
  }

  function collapseIsland() {
    clearCollapseTimer();
    if (!expandedRef.current) {
      setExpanded(false);
      setIslandPresentation("compact").catch(() => undefined);
      return;
    }
    expandedRef.current = false;
    setExpanded(false);
    collapseTimerRef.current = window.setTimeout(() => {
      collapseTimerRef.current = null;
      if (!expandedRef.current) {
        setIslandPresentation("compact").catch(() => undefined);
      }
    }, 360);
  }

  function collapseIdleIsland() {
    if (snapshotRef.current.pendingCount === 0) {
      collapseIsland();
    }
  }

  function clearCollapseTimer() {
    if (collapseTimerRef.current === null) return;
    window.clearTimeout(collapseTimerRef.current);
    collapseTimerRef.current = null;
  }

  function clearOpenTimer() {
    if (openTimerRef.current === null) return;
    window.clearTimeout(openTimerRef.current);
    openTimerRef.current = null;
  }

  function scheduleIdleCollapse() {
    clearOpenTimer();
    openTimerRef.current = window.setTimeout(() => {
      openTimerRef.current = null;
      if (!hoveringRef.current && snapshotRef.current.pendingCount === 0) {
        collapseIsland();
      }
    }, 1600);
  }

  function handleIslandBlur(event: FocusEvent<HTMLElement>) {
    if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
      collapseIdleIsland();
    }
  }

  function compactIslandNow() {
    clearCollapseTimer();
    clearOpenTimer();
    expandedRef.current = false;
    setExpanded(false);
    return setIslandPresentation("compact").catch(() => undefined);
  }

  async function startWindowDrag(event: MouseEvent<HTMLElement>) {
    if (!isTauriRuntime || event.button !== 0) return;

    const target = event.target as HTMLElement;
    if (target.closest("[data-no-drag]")) return;

    await getCurrentWindow().startDragging().catch(() => undefined);
  }

  async function hideWindow(event: MouseEvent<HTMLButtonElement>) {
    event.stopPropagation();
    if (!isTauriRuntime) return;

    await compactIslandNow();
    await getCurrentWindow().hide().catch(() => undefined);
  }

  return (
    <main className="stage">
      <section
        className={`island ${expanded ? "is-expanded" : ""}`}
        aria-label="Atoll"
        onMouseEnter={expandIsland}
        onPointerEnter={expandIsland}
        onPointerMove={expandIsland}
        onMouseLeave={collapseIdleIsland}
        onFocusCapture={expandIsland}
        onBlurCapture={handleIslandBlur}
      >
        <header
          className="island-compact"
          onMouseDown={startWindowDrag}
          title="Drag Atoll"
        >
          <span className={`agent-dot ${activeRequest ? agentTone[activeRequest.agent] : "idle"}`}>
            {activeRequest ? <ShieldAlert size={16} /> : <Circle size={12} />}
          </span>
          <span className="compact-copy">
            <span className="compact-title">
              {activeRequest ? `${agentLabels[activeRequest.agent]} waiting` : "Atoll"}
            </span>
            <span className="compact-meta">
              {activeRequest ? activeRequest.command : "No pending approvals"}
            </span>
          </span>
          {snapshot.pendingCount > 0 ? (
            <span className="pending-badge" aria-label={`${snapshot.pendingCount} pending`}>
              {snapshot.pendingCount}
            </span>
          ) : null}
          <button
            className="compact-action close-action"
            type="button"
            onClick={hideWindow}
            aria-label="Hide Atoll"
            data-no-drag
          >
            <X size={16} />
          </button>
        </header>

        <div className="island-panel">
          <header className="panel-header" onMouseDown={startWindowDrag}>
            <div className="session-line">
              <span className="status-pill">
                <Bell size={14} />
                {snapshot.online ? "Listening" : "Offline"}
              </span>
              <span>{pendingRequests.length} pending</span>
            </div>
            <div className="window-actions" data-no-drag>
              <button className="icon-button" type="button" onClick={hideWindow} aria-label="Hide Atoll">
                <X size={16} />
              </button>
            </div>
          </header>

          {activeRequest ? (
            <ApprovalView
              request={activeRequest}
              busyDecision={busyDecision}
              onApprove={() => resolveActive("approved")}
              onDeny={() => resolveActive("denied")}
            />
          ) : (
            <IdleView onCreateDemo={createDemoRequest} />
          )}

          <footer className="queue-strip" aria-label="Pending request queue">
            {pendingRequests.slice(0, 4).map((request) => (
              <button
                key={request.id}
                className={`queue-chip ${agentTone[request.agent]}`}
                type="button"
                title={`${agentLabels[request.agent]}: ${request.command}`}
              >
                <TerminalSquare size={14} />
                <span>{agentLabels[request.agent]}</span>
              </button>
            ))}
            <button className="queue-chip ghost" type="button" onClick={createDemoRequest}>
              <Play size={14} />
              <span>Demo</span>
            </button>
          </footer>
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
        <div className={`request-icon ${agentTone[request.agent]}`}>
          <Code2 size={20} />
        </div>
        <div className="request-copy">
          <div className="request-kicker">
            <span>{agentLabels[request.agent]}</span>
            <span>{timeAgo(request.requestedAt)}</span>
          </div>
          <h1>{request.command}</h1>
          <p>{request.detail}</p>
          <div className="cwd-line" title={request.cwd}>
            {request.cwd}
          </div>
        </div>
      </div>

      <div className="decision-row">
        <button
          className="decision-button deny"
          type="button"
          onClick={onDeny}
          disabled={busyDecision !== null}
        >
          <X size={18} />
          <span>{busyDecision === "denied" ? "Denying" : "Deny"}</span>
        </button>
        <button
          className="decision-button approve"
          type="button"
          onClick={onApprove}
          disabled={busyDecision !== null}
        >
          <Check size={18} />
          <span>{busyDecision === "approved" ? "Approving" : "Approve"}</span>
        </button>
      </div>
    </div>
  );
}

function IdleView({ onCreateDemo }: { onCreateDemo: () => void }) {
  return (
    <div className="idle-view">
      <div className="idle-icon">
        <ClipboardCheck size={22} />
      </div>
      <div>
        <h1>All clear</h1>
        <p>Agent approvals will surface here.</p>
      </div>
      <button className="icon-button prominent" type="button" onClick={onCreateDemo} aria-label="Create demo request">
        <Play size={16} />
      </button>
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
