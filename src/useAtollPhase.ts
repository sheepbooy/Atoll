import { useEffect, useRef, useState } from "react";
import type { AtollActivity } from "./AtollLogo";
import { isAppStatePose } from "./logoStates";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS, type AtollPhase } from "./atollTransitions";
import { motionDelay, useReducedMotion } from "./animationTiming";

function initialPhase(targetAct: AtollActivity): AtollPhase {
  if (targetAct === "idle" || isAppStatePose(targetAct)) return "loop";
  return "enter";
}

export function useAtollPhase(targetAct: AtollActivity, motionPaused = false) {
  const reducedMotion = useReducedMotion();
  const [renderAct, setRenderAct] = useState<AtollActivity>(targetAct);
  const [phase, setPhase] = useState<AtollPhase>(() => initialPhase(targetAct));
  const enterTimerRef = useRef<number | null>(null);
  const transitionTimerRef = useRef<number | null>(null);
  const prevTargetRef = useRef(targetAct);
  const renderActRef = useRef(renderAct);
  renderActRef.current = renderAct;

  const clearEnterTimer = () => {
    if (enterTimerRef.current !== null) {
      window.clearTimeout(enterTimerRef.current);
      enterTimerRef.current = null;
    }
  };

  const clearTransitionTimer = () => {
    if (transitionTimerRef.current !== null) {
      window.clearTimeout(transitionTimerRef.current);
      transitionTimerRef.current = null;
    }
  };

  useEffect(() => {
    if (phase !== "enter" || motionPaused || reducedMotion) return;
    clearEnterTimer();
    enterTimerRef.current = window.setTimeout(() => {
      enterTimerRef.current = null;
      setPhase("loop");
    }, motionDelay(ATOLL_ENTER_MS));
    return clearEnterTimer;
  }, [phase, motionPaused, reducedMotion]);

  useEffect(() => {
    const prev = prevTargetRef.current;
    prevTargetRef.current = targetAct;
    if (reducedMotion) {
      clearEnterTimer();
      clearTransitionTimer();
      setRenderAct(targetAct);
      setPhase("loop");
      return;
    }
    if (motionPaused) {
      prevTargetRef.current = prev;
      clearTransitionTimer();
      return;
    }
    if (prev === targetAct && phase !== "exit") return;

    clearTransitionTimer();
    clearEnterTimer();

    if (targetAct === "idle") {
      if (renderActRef.current === "idle") {
        setPhase("loop");
        return;
      }
      setPhase("exit");
      transitionTimerRef.current = window.setTimeout(() => {
        transitionTimerRef.current = null;
        setRenderAct("idle");
        setPhase("loop");
      }, motionDelay(ATOLL_EXIT_MS));
      return;
    }

    if (renderActRef.current !== "idle" && renderActRef.current !== targetAct) {
      setPhase("exit");
      transitionTimerRef.current = window.setTimeout(() => {
        transitionTimerRef.current = null;
        setRenderAct(targetAct);
        setPhase("enter");
      }, motionDelay(ATOLL_EXIT_MS));
      return;
    }

    setRenderAct(targetAct);
    setPhase("enter");
  }, [targetAct, motionPaused, reducedMotion]);

  useEffect(
    () => () => {
      clearEnterTimer();
      clearTransitionTimer();
    },
    [],
  );

  return { renderAct, phase };
}
