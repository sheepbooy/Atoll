import { ClawdMascot, type ClawdMood } from "./ClawdMascot";
import { CodexMascot } from "./CodexMascot";
import type { AgentKind } from "./tauri";

export type AgentMascotMood = ClawdMood;

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
