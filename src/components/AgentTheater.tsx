import { useTranslation } from "react-i18next";
import { AgentMascot, type MascotAction } from "../AgentMascot";
import { getSessionColor, getSubagentColor } from "../subagentIdentity";
import type { TheaterScene } from "../hooks/useAgentTheater";

export function AgentTheater({ scene, capacity = 3 }: { scene: TheaterScene; capacity?: number }) {
  const { t } = useTranslation("common");
  const { event } = scene;
  const limit = Math.max(1, Math.min(3, capacity));
  // Always keep the actor performing this interaction visible.
  const ordered = [...scene.actors];
  if (event.subagentId && ordered.includes(event.subagentId)) {
    ordered.splice(ordered.indexOf(event.subagentId), 1);
    ordered.splice(limit === 1 ? 0 : Math.min(1, ordered.length), 0, event.subagentId);
  }
  const actors = ordered.slice(0, limit);
  const overflow = ordered.length - actors.length;
  const label = t(`theater.${event.kind}`);
  const action: MascotAction = event.kind === "started" ? "started" : event.kind === "subagentStarted" ? "join" : event.kind === "subagentEnded" ? "leave" : "turnEnded";
  return <span className="agent-theater compact-session-stack" title={label} role="status" aria-label={label}>
    {actors.map(id => {
      const performing = id === event.subagentId;
      const color = id === null ? getSessionColor(event.sessionId) : getSubagentColor(id);
      return <span key={`${event.agent}:${event.sessionId}:${id ?? "main"}`} className={`compact-session-dot agent-theater-actor${performing ? ` is-${event.kind}` : ""}`}>
        <AgentMascot agent={event.agent} mood="calm" size={18}
          instanceKey={id ?? event.sessionId} accent={color.accent} accentDark={color.accentDark}
          activity="rest"
          action={performing ? action : null} actionKey={event.eventId} />
      </span>;
    })}
    {overflow > 0 ? <span className="compact-session-overflow">+{overflow}</span> : null}
  </span>;
}
