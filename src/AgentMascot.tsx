import { ClawdMascot, type ClawdMood } from "./ClawdMascot";
import { CodexMascot } from "./CodexMascot";
import { CursorMascot } from "./CursorMascot";
import type { AgentKind } from "./tauri";

export type AgentMascotMood = ClawdMood;

export const AGENT_ACCENT: Record<AgentKind, { accent?: string; accentDark?: string }> = {
  claude: { accent: undefined, accentDark: undefined },
  codex: { accent: "#61d8f7", accentDark: "#3d9fb8" },
  cursor: { accent: "#a78bfa", accentDark: "#7c5fd4" },
  gemini: { accent: "#b2e578", accentDark: "#7aa44d" },
  other: { accent: "#c9bcff", accentDark: "#9182d1" },
};

interface AgentMascotProps {
  agent: AgentKind;
  mood: AgentMascotMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
}

export function AgentMascot({
  agent,
  mood,
  size,
  className,
  accent,
  accentDark,
}: AgentMascotProps) {
  if (agent === "codex") {
    return (
      <CodexMascot
        mood={mood}
        size={size}
        className={className}
        accent={accent}
        accentDark={accentDark}
      />
    );
  }

  if (agent === "cursor") {
    return (
      <CursorMascot
        mood={mood}
        size={size}
        className={className}
        accent={accent}
        accentDark={accentDark}
      />
    );
  }

  return (
    <ClawdMascot
      mood={mood}
      size={size}
      className={className}
      accent={accent}
      accentDark={accentDark}
    />
  );
}
