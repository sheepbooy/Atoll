import { useEffect, useRef, useState } from "react";
import type { AtollActivity } from "./AtollLogo";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS, type AtollPhase } from "./atollTransitions";

export function useAtollPhase(targetAct: AtollActivity) {
  const [renderAct, setRenderAct] = useState<AtollActivity>(targetAct);
  const [phase, setPhase] = useState<AtollPhase>(() =>
    targetAct === "idle" ? "loop" : "enter",
  );
  const timersRef = useRef<number[]>([]);
  const prevTargetRef = useRef(targetAct);
  const renderActRef = useRef(renderAct);
  renderActRef.current = renderAct;

  const clearTimers = () => {
    timersRef.current.forEach((id) => window.clearTimeout(id));
    timersRef.current = [];
  };

  const schedule = (fn: () => void, ms: number) => {
    const id = window.setTimeout(fn, ms);
    timersRef.current.push(id);
  };

  useEffect(() => {
    if (phase !== "enter") return;
    schedule(() => setPhase("loop"), ATOLL_ENTER_MS);
    return clearTimers;
  }, [phase, renderAct]);

  useEffect(() => {
    const prev = prevTargetRef.current;
    prevTargetRef.current = targetAct;
    if (prev === targetAct) return;

    clearTimers();

    if (targetAct === "idle") {
      if (renderActRef.current === "idle") {
        setPhase("loop");
        return;
      }
      setPhase("exit");
      schedule(() => {
        setRenderAct("idle");
        setPhase("loop");
      }, ATOLL_EXIT_MS);
      return;
    }

    if (renderActRef.current !== "idle" && renderActRef.current !== targetAct) {
      setPhase("exit");
      schedule(() => {
        setRenderAct(targetAct);
        setPhase("enter");
      }, ATOLL_EXIT_MS);
      return;
    }

    setRenderAct(targetAct);
    setPhase("enter");
  }, [targetAct]);

  useEffect(() => clearTimers, []);

  return { renderAct, phase };
}
