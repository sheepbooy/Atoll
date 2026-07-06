import { useEffect, useRef, useState } from "react";
import type { AtollActivity } from "./AtollLogo";
import { isAppStatePose } from "./logoStates";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS, type AtollPhase } from "./atollTransitions";

function initialPhase(targetAct: AtollActivity): AtollPhase {
  if (targetAct === "idle" || isAppStatePose(targetAct)) return "loop";
  return "enter";
}

export function useAtollPhase(targetAct: AtollActivity) {
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
    if (phase !== "enter") return;
    clearEnterTimer();
    enterTimerRef.current = window.setTimeout(() => {
      enterTimerRef.current = null;
      setPhase("loop");
    }, ATOLL_ENTER_MS);
    return clearEnterTimer;
  }, [phase]);

  useEffect(() => {
    const prev = prevTargetRef.current;
    prevTargetRef.current = targetAct;
    if (prev === targetAct) return;

    clearTransitionTimer();

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
      }, ATOLL_EXIT_MS);
      return;
    }

    if (renderActRef.current !== "idle" && renderActRef.current !== targetAct) {
      setPhase("exit");
      transitionTimerRef.current = window.setTimeout(() => {
        transitionTimerRef.current = null;
        setRenderAct(targetAct);
        setPhase("enter");
      }, ATOLL_EXIT_MS);
      return;
    }

    setRenderAct(targetAct);
    setPhase("enter");
  }, [targetAct]);

  useEffect(
    () => () => {
      clearEnterTimer();
      clearTransitionTimer();
    },
    [],
  );

  return { renderAct, phase };
}
