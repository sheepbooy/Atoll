import { ClawdMascot, type ClawdMood } from "./ClawdMascot";
import { CodexMascot } from "./CodexMascot";
import { CursorMascot } from "./CursorMascot";
import { GeminiMascot } from "./GeminiMascot";
import { OpencodeMascot } from "./OpencodeMascot";
import { ZcodeMascot } from "./ZcodeMascot";
import type { AgentKind } from "./tauri";
import { useMascotEnvironment, type MascotMotionProps } from "./mascotMotion";

export type AgentMascotMood = ClawdMood;

export const AGENT_ACCENT: Record<AgentKind, { accent?: string; accentDark?: string }> = {
  claude: { accent: undefined, accentDark: undefined },
  codex: { accent: "#61d8f7", accentDark: "#3d9fb8" },
  cursor: { accent: "#a78bfa", accentDark: "#7c5fd4" },
  zcode: { accent: "#7cc8f0", accentDark: "#2f88b8" },
  gemini: { accent: "#4796E3", accentDark: "#CA6673" },
  opencode: { accent: "#70d8c8", accentDark: "#2aa896" },
  other: { accent: "#c9bcff", accentDark: "#9182d1" },
};

export type { MascotAction, MascotMotionProps } from "./mascotMotion";

export interface AgentMascotProps extends MascotMotionProps {
  agent: AgentKind;
  mood: AgentMascotMood;
  size?: number;
  className?: string;
  accent?: string;
  accentDark?: string;
  animated?: boolean;
  /** Brand tabs summarize sessions; individual avatars keep session-local feedback. */
  feedbackScope?: "session" | "agent";
}

const CHARACTERS = { claude: ClawdMascot, codex: CodexMascot, cursor: CursorMascot,
  zcode: ZcodeMascot, gemini: GeminiMascot, opencode: OpencodeMascot, other: ClawdMascot };

export function AgentMascot({ agent, feedbackScope = "session", ...props }: AgentMascotProps) {
  const { feedback, workingSessions } = useMascotEnvironment();
  const Character = CHARACTERS[agent];
  const working = workingSessions.has(`${agent}:${props.instanceKey}`) ||
    (props.instanceKey === agent && [...workingSessions].some(key => key.startsWith(`${agent}:`)));
  const characterProps = props.activity === undefined && working ? { ...props, activity: "working" as const } : props;
  if (props.action === undefined && feedback && feedback.agent === agent &&
      (feedbackScope === "agent" || feedback.sessionId === props.instanceKey) &&
      props.mood !== "alert" && props.mood !== "worried") {
    return <Character {...characterProps} action={feedback.decision} actionKey={feedback.eventId} />;
  }
  return <Character {...characterProps} />;
}
