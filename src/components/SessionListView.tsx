import {
  useEffect,
  useRef,
  useState,
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import {
  Archive,
  Check,
  ChevronRight,
  Layers,
  Pin,
  PinOff,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  onIslandHoverChanged,
  archiveCompletedSubagents,
  type PermissionRequest,
  type SessionSummary,
  type SubagentSummary,
} from "../tauri";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  getSessionColor,
  getSubagentColor,
  getSubagentMood,
} from "../subagentIdentity";
import {
  manageAsyncUnlisten,
} from "../asyncUnlisten";
import {
  agentLabels,
} from "../agents";
import {
  deriveSessionMood,
} from "../riskAssess";
import {
  sessionDisplayName,
  timeAgo,
} from "../sessionDisplay";

export function sessionIdAtClientPoint(
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

export interface SessionListViewProps {
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

export function partitionSubagents(subagents: SubagentSummary[], limit: number) {
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

export function SessionListView({
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
  const { t } = useTranslation();
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
        <span>{t("session.count", { count: sessions.length })}</span>
      </div>
      <div
        ref={listRef}
        className="session-list"
        onPointerMove={handleListPointerMove}
        onPointerLeave={() => setHoveredSessionId(null)}
      >
        {sessions.map((session, sessionIndex) => {
          const sessionColor = getSessionColor(session.sessionId);
          const isHovered = hoveredSessionId === session.sessionId;
          return (
            <div
              key={session.sessionId}
              data-session-id={session.sessionId}
              className={`session-item ${session.pinned ? "is-pinned" : ""} ${isHovered ? "is-hovered" : ""}`}
              style={{ "--stagger-i": Math.min(sessionIndex, 8) } as CSSProperties}
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
                                      title={
                                        sub.completedAt
                                          ? t("session.subagentDone", {
                                              agentType: sub.agentType,
                                            })
                                          : sub.agentType
                                      }
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
                                    title={t("session.viewAllSubagents")}
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
                                  title={t("session.archiveCompletedSubagents")}
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
                  title={session.pinned ? t("session.unpin") : t("session.pin")}
                  onClick={(e) => { e.stopPropagation(); onPinSession(session.sessionId, !session.pinned); }}
                >
                  {session.pinned ? <PinOff size={12} /> : <Pin size={12} />}
                </button>
                <button
                  type="button"
                  className="session-action-btn"
                  title={t("session.archive")}
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
