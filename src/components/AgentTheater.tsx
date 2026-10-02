import { useTranslation } from "react-i18next";
import { AgentMascot } from "../AgentMascot";
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
  return <span className="agent-theater compact-session-stack" title={label} role="status" aria-label={label} key={event.eventId}>
    {actors.map(id => <span key={id === null ? "main" : `subagent:${id}`} className={`compact-session-dot agent-theater-actor${id === event.subagentId ? ` is-${event.kind}` : ""}`}>
      <AgentMascot agent={event.agent} mood={event.kind === "turnEnded" ? "calm" : "alert"} size={18} animated={false} />
      {id === null && event.kind === "started" ? <svg className="theater-keyboard" viewBox="0 0 18 6" aria-hidden="true">
        <rect x="1" y="1" width="16" height="4" rx="1" fill="#33434f" />
        <path d="M3 2h12M4 4h10" stroke="#84d1e8" strokeWidth=".6" />
      </svg> : null}
    </span>)}
    {overflow > 0 ? <span className="compact-session-overflow">+{overflow}</span> : null}
  </span>;
}
