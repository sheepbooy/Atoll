export type PresentationPhase =
  | "micro"
  | "compact"
  | "opening"
  | "expanded"
  | "closing";

export const COLLAPSE_ANIMATION_MS = 420;
export const IDLE_COLLAPSE_DELAY_MS = 500;
/** Fade panel content before native window shrink begins. */
export const PANEL_EXIT_MS = 120;
/** Hold resolve UI feedback before applying the next snapshot. */
export const RESOLVE_FEEDBACK_MS = 240;
export const MICRO_SHRINK_DELAY_MS = 500;

export function beginExpand(phase: PresentationPhase): PresentationPhase {
  return phase === "micro" || phase === "compact" || phase === "closing"
    ? "opening"
    : phase;
}

export function finishExpand(phase: PresentationPhase): PresentationPhase {
  return phase === "opening" ? "expanded" : phase;
}

export function beginCollapse(phase: PresentationPhase): PresentationPhase {
  return phase === "opening" || phase === "expanded" ? "closing" : phase;
}

export function finishCollapse(
  phase: PresentationPhase,
  toMicro = false,
): PresentationPhase {
  if (phase !== "closing") return phase;
  return toMicro ? "micro" : "compact";
}
