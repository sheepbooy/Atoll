import {
  type PermissionRequest,
  type SessionSummary,
} from "../tauri";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  getSessionColor,
} from "../subagentIdentity";
import {
  agentLabels,
} from "../agents";
import {
  deriveSessionMood,
} from "../riskAssess";
import {
  sessionDisplayName,
} from "../sessionDisplay";

export interface CompactSessionStackProps {
  sessions: SessionSummary[];
  overflowCount?: number;
  placement?: "left" | "right";
  activeRequest: PermissionRequest | null;
  justResolved: boolean;
}

export function CompactSessionStack({
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
