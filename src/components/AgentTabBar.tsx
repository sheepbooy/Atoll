import {
  useTranslation,
} from "react-i18next";
import {
  AgentMascot,
} from "../AgentMascot";
import {
  type ClawdMood,
} from "../ClawdMascot";
import {
  type AgentKind,
} from "../appTypes";
import {
  agentLabels,
  agentTone,
  agentMascotAccent,
  agentMascotDark,
} from "../agents";

export interface AgentTabBarProps {
  agents: AgentKind[];
  selectedAgent: AgentKind | null;
  pendingCountByAgent: Record<AgentKind, number>;
  showTabs: boolean;
  compact?: boolean;
  online: boolean;
  onSelectAgent: (agent: AgentKind) => void;
}

export function AgentTabBar({
  agents,
  selectedAgent,
  pendingCountByAgent,
  showTabs,
  compact = false,
  online,
  onSelectAgent,
}: AgentTabBarProps) {
  const { t } = useTranslation();

  if (agents.length === 0) {
    return (
      <span className="agent-tabs-empty">
        {online ? t("header.listeningForAgents") : t("header.offline")}
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
