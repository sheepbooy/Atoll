import {
  AGENT_ACCENT,
} from "./AgentMascot";
import {
  type AgentKind,
} from "./appTypes";

export const agentLabels: Record<AgentKind, string> = {
  claude: "Claude",
  codex: "Codex",
  cursor: "Cursor",
  zcode: "ZCode",
  gemini: "Gemini",
  other: "Agent",
};

export const agentTone: Record<AgentKind, string> = {
  claude: "coral",
  codex: "cyan",
  cursor: "violet",
  zcode: "sky",
  gemini: "lime",
  other: "neutral",
};

export const agentSortRank: Record<AgentKind, number> = {
  claude: 0,
  codex: 1,
  cursor: 2,
  zcode: 2,
  gemini: 2,
  other: 3,
};
export const agentMascotAccent = (agent: AgentKind) => AGENT_ACCENT[agent]?.accent;
export const agentMascotDark = (agent: AgentKind) => AGENT_ACCENT[agent]?.accentDark;

export const PANEL_GLOW: Record<AgentKind, string> = {
  claude: "rgba(255, 129, 117, 0.18)",
  codex: "rgba(97, 216, 247, 0.18)",
  cursor: "rgba(167, 139, 250, 0.18)",
  zcode: "rgba(56, 189, 248, 0.18)",
  gemini: "rgba(178, 229, 120, 0.18)",
  other: "rgba(201, 188, 255, 0.16)",
};
