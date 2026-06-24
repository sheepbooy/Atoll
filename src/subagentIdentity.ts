import type { ClawdMood } from "./ClawdMascot";

export type SessionTone =
  | "coral"
  | "cyan"
  | "lime"
  | "neutral"
  | "amber"
  | "pink"
  | "teal"
  | "blue";

export interface SessionColor {
  tone: SessionTone;
  accent: string;
  accentDark: string;
}

export const SESSION_PALETTE: SessionColor[] = [
  { tone: "coral", accent: "#e8765a", accentDark: "#b85a42" },
  { tone: "cyan", accent: "#61d8f7", accentDark: "#3d9fb8" },
  { tone: "lime", accent: "#b2e578", accentDark: "#7aa44d" },
  { tone: "neutral", accent: "#c9bcff", accentDark: "#9182d1" },
  { tone: "amber", accent: "#f0c060", accentDark: "#b89040" },
  { tone: "pink", accent: "#f7a0c8", accentDark: "#c07098" },
  { tone: "teal", accent: "#70d8c8", accentDark: "#48a898" },
  { tone: "blue", accent: "#80b0f8", accentDark: "#5888d0" },
];

const SUBAGENT_RUNNING_MOODS: ClawdMood[] = ["alert", "happy", "calm"];
const SUBAGENT_COMPLETED_MOODS: ClawdMood[] = ["calm", "happy", "sleeping"];

export function stringHash(value: string): number {
  let hash = 0;
  for (let i = 0; i < value.length; i += 1) {
    hash = ((hash << 5) - hash + value.charCodeAt(i)) | 0;
  }
  return hash;
}

export function paletteIndex(key: string, size: number): number {
  if (!key || size <= 0) return 0;
  return ((stringHash(key) % size) + size) % size;
}

export function getSessionColor(sessionId: string): SessionColor {
  return SESSION_PALETTE[paletteIndex(sessionId, SESSION_PALETTE.length)];
}

export function getSubagentColor(agentId: string): SessionColor {
  return SESSION_PALETTE[paletteIndex(agentId, SESSION_PALETTE.length)];
}

export function getSubagentMood(agentId: string, completed: boolean): ClawdMood {
  const moods = completed ? SUBAGENT_COMPLETED_MOODS : SUBAGENT_RUNNING_MOODS;
  return moods[paletteIndex(`${agentId}:mood`, moods.length)];
}
