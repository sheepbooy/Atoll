import { createContext, createElement, useContext, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useReducedMotion } from "./animationTiming";
import type { ClawdMood } from "./mascotShared";
import type { AgentKind } from "./tauri";

export type MascotAction = "sleeping" | "concerned" | "downcast" | "started" | "attention" | "approved" | "denied" | "turnEnded" | "join" | "leave";
export interface MascotMotionProps {
  activity?: "idle" | "working" | "rest";
  action?: MascotAction | null;
  actionKey?: string | number;
  instanceKey?: string;
  motionPaused?: boolean;
}
export const MASCOT_ACTION_MS = 900;
export interface MascotTimeline {
  state: MascotAction | "idle" | "working";
  key: string;
  durationMs: number;
  phaseMs: number;
  repeat: boolean;
}
export interface ApprovalMascotFeedback {
  eventId: string;
  requestId: string;
  agent: AgentKind;
  sessionId: string;
  decision: "approved" | "denied";
}
const NO_WORK: ReadonlySet<string> = new Set();
const Environment = createContext<{ ambientPaused: boolean; feedback: ApprovalMascotFeedback | null; rate: number; workingSessions: ReadonlySet<string> }>({ ambientPaused: false, feedback: null, rate: 1, workingSessions: NO_WORK });
export function MascotEnvironmentProvider({ ambientPaused, feedback, children, rate = 1, workingSessions = NO_WORK }: {
  ambientPaused: boolean; feedback: ApprovalMascotFeedback | null; children: ReactNode; rate?: number; workingSessions?: ReadonlySet<string>;
}) {
  const value = useMemo(() => ({ ambientPaused, feedback, rate, workingSessions }), [ambientPaused, feedback, rate, workingSessions]);
  return createElement(Environment.Provider, { value }, children);
}
export const useMascotEnvironment = () => useContext(Environment);

/** Only schedules finite feedback. The six characters own their CSS timelines. */
export function useMascotMotion({ mood, animated = true, size, activity = "idle", action,
  actionKey, instanceKey = "", motionPaused = false, character = "claude",
}: MascotMotionProps & { mood: ClawdMood; animated?: boolean; size?: number; character?: AgentKind }) {
  const ref = useRef<HTMLSpanElement>(null);
  const { ambientPaused, rate } = useMascotEnvironment();
  const reduced = useReducedMotion();
  const [shot, setShot] = useState<{ kind: MascotAction; key: string } | null>(null);
  const event = useRef<{ key: string; expiresAt: number } | null>(null);
  const settled = useRef(false);
  const enabled = animated && !motionPaused && !reduced;
  const implied = mood === "alert" ? "attention" : mood === "happy" ? "approved" : mood === "worried" ? "concerned" : mood === "sleeping" ? "sleeping" : mood === "sad" && character !== "claude" ? "downcast" : null;
  const requested = action === undefined ? implied : action;
  const actionMs = requested === "attention" && character !== "claude" ? 1600 : MASCOT_ACTION_MS;
  const key = requested ? `${action === undefined ? "mood" : "event"}:${requested}:${actionKey ?? mood}` : "";

  useEffect(() => {
    if (!requested) { event.current = null; setShot(null); return; }
    if (event.current?.key !== key) event.current = { key, expiresAt: Date.now() + actionMs / rate };
    if (!enabled) { event.current.expiresAt = 0; setShot(null); return; }
    const remaining = event.current.expiresAt - Date.now();
    if (remaining <= 0) { setShot(null); return; }
    settled.current = true;
    setShot(previous => previous?.key === key ? previous : { kind: requested, key });
    const timer = window.setTimeout(() => setShot(null), remaining);
    return () => window.clearTimeout(timer);
  }, [requested, key, enabled, mood, rate, actionMs]);

  useLayoutEffect(() => {
    if (!shot || !enabled) return;
    for (const animation of ref.current?.getAnimations?.({ subtree: true }) ?? []) {
      if ((animation as CSSAnimation).animationName?.startsWith("agent-shot-")) animation.currentTime = 0;
    }
  }, [shot, enabled]);

  const loop = !enabled || ambientPaused || activity === "rest" || ["sleeping", "dead", "sad", "worried"].includes(mood)
    ? null : mood === "alert" ? "attention" : activity === "working" ? "working" : "idle";
  let hash = 0;
  for (const char of instanceKey) hash = (hash * 31 + char.charCodeAt(0)) | 0;
  // Resume at a neutral frame after feedback; asking again starts at the next 4s beat.
  const phase = settled.current ? 0 : Math.abs(hash % 2400);
  const reminderDelay = event.current?.key === key ? (4000 - actionMs) / rate : -phase / rate;
  const activeShot = shot && enabled && shot.key === key ? shot : null;
  const timeline: MascotTimeline | null = activeShot
    ? { state: activeShot.kind, key: activeShot.key, durationMs: actionMs / rate, phaseMs: 0, repeat: false }
    : loop
      ? { state: loop, key: `loop:${loop}`, durationMs: (loop === "working" ? 3200 : loop === "attention" ? 4000 : 12000) / rate, phaseMs: loop === "attention" && character !== "claude" ? -reminderDelay : phase / rate, repeat: true }
      : null;
  return {
    ref,
    timeline,
    ambientEnabled: enabled && !ambientPaused && activity !== "rest",
    motionClassName: `agent-mascot is-activity-${activity}${loop ? ` is-loop-${loop}` : ""}${size !== undefined && size <= 20 ? " is-small" : ""}${!enabled ? " is-motion-disabled" : ""}${shot && enabled && shot.key === key ? ` has-action is-action-${shot.kind}` : ""}`,
    motionStyle: { "--agent-phase": `-${phase / rate}ms`, "--agent-remind-phase": `${reminderDelay}ms`, "--agent-shot-ms": `${actionMs / rate}ms`, "--agent-idle-ms": `${12000 / rate}ms`, "--agent-work-ms": `${3200 / rate}ms`, "--agent-remind-ms": `${4000 / rate}ms` } as CSSProperties,
  };
}
