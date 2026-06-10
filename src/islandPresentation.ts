export type PresentationPhase =
  | "compact"
  | "opening"
  | "expanded"
  | "closing";

export const COLLAPSE_ANIMATION_MS = 420;
export const IDLE_COLLAPSE_DELAY_MS = 500;

export function beginExpand(phase: PresentationPhase): PresentationPhase {
  return phase === "compact" || phase === "closing" ? "opening" : phase;
}

export function finishExpand(phase: PresentationPhase): PresentationPhase {
  return phase === "opening" ? "expanded" : phase;
}

export function beginCollapse(phase: PresentationPhase): PresentationPhase {
  return phase === "opening" || phase === "expanded" ? "closing" : phase;
}

export function finishCollapse(phase: PresentationPhase): PresentationPhase {
  return phase === "closing" ? "compact" : phase;
}
