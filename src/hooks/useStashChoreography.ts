// 文件中转站"岛即生物"编舞：吉祥物只演眼睛（CSS 时间线驱动），这里负责
// 岛形拍点、原生窗口脉冲、吞咽涟漪、文件飞行层与结果 toast。
import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { TFunction } from "i18next";
import type { AtollReaction } from "../AtollLogo";
import { ATOLL_REACTION_MS, ISLAND_SHAPE_MS } from "../atollTransitions";
import { motionDelay, useReducedMotion } from "../animationTiming";
import {
  type CancelMotion,
  flySpitFiles,
  flyStashFiles,
  playIslandShape,
  playSwallowRipple,
} from "../islandShape";
import { pulseIslandShape } from "../tauri";

interface UseStashChoreographyOptions {
  stashReaction: AtollReaction | null;
  stashReactionKey: number;
  motionPaused: boolean;
  lastStageResult: { added: number; evicted: number; skipped: number } | null;
  islandRef: RefObject<HTMLElement | null>;
  atollIndicatorRef: RefObject<HTMLSpanElement | null>;
  /** Island-local cursor position of the drop (useFileStation). */
  dropPointRef: RefObject<{ x: number; y: number } | null>;
  /** Drag-out vector angle in CSS deg (atan2(dy,dx)); set by the panel row
   *  gesture so spit files fly along the real drag direction. */
  spitAngleRef: RefObject<number | null>;
  t: TFunction;
}

/** 各档吸入的视觉飞行数量：大批量才拉出连飞感。 */
const EAT_FLY_COUNT: Record<"eat1" | "eat2" | "eat3" | "eat4", number> = {
  eat1: 1,
  eat2: 2,
  eat3: 3,
  eat4: 6,
};

export function useStashChoreography({
  stashReaction,
  stashReactionKey,
  motionPaused,  lastStageResult,
  islandRef,
  atollIndicatorRef,
  dropPointRef,
  spitAngleRef,
  t,
}: UseStashChoreographyOptions) {
  const reducedMotion = useReducedMotion();
  const eventRef = useRef<{ reaction: AtollReaction; key: number; expiresAt: number; played: boolean } | null>(null);
  useEffect(() => {
    if (!stashReaction) { eventRef.current = null; return; }
    let event = eventRef.current;
    if (!event || event.key !== stashReactionKey || event.reaction !== stashReaction) {
      event = { reaction: stashReaction, key: stashReactionKey, expiresAt: Date.now() + ATOLL_REACTION_MS[stashReaction], played: false };
      eventRef.current = event;
    }
    if (reducedMotion) { event.played = true; return; }
    if (motionPaused || event.played || event.expiresAt <= Date.now()) return;
    const islandEl = islandRef.current;
    if (!islandEl) return;
    event.played = true;
    let cancelled = false;
    const timers: number[] = [];
    const motions: CancelMotion[] = [];
    const keep = (cancel: CancelMotion) => motions.push(cancel);
    const later = (fn: () => void, ms: number) => {
      timers.push(window.setTimeout(() => { if (!cancelled) fn(); }, motionDelay(ms)));
    };
    const cancel = () => {
      cancelled = true;
      timers.forEach((timer) => window.clearTimeout(timer));
      motions.forEach((stop) => stop());
    };
    if (stashReaction === "spit") {
      const angle = spitAngleRef.current ?? -31;
      const radians = (angle * Math.PI) / 180;
      const direction = { dx: Math.cos(radians) * 4, dy: Math.sin(radians) * 4, skew: Math.sin(radians) * 0.5 };
      keep(playIslandShape(islandEl, "coil", { direction }));
      later(() => {
        keep(playIslandShape(islandEl, "launch", { direction }));
        void pulseIslandShape({ heightDelta: -8, outMs: 100, backMs: 190 });
        keep(flySpitFiles(islandEl, atollIndicatorRef.current, angle, 3));
      }, ISLAND_SHAPE_MS.coil);
      later(() => keep(playIslandShape(islandEl, "wobble", { direction })), ISLAND_SHAPE_MS.coil + 180);
      return cancel;
    }
    if (!Object.prototype.hasOwnProperty.call(EAT_FLY_COUNT, stashReaction)) return cancel;
    const flyCount = EAT_FLY_COUNT[stashReaction as keyof typeof EAT_FLY_COUNT];
    const drop = dropPointRef.current;
    const islandWidth = islandEl.clientWidth || 1;
    const skew = drop !== null ? Math.max(-1, Math.min(1, (drop.x - islandWidth / 2) / (islandWidth / 2))) * 0.6 : 0;
    keep(playIslandShape(islandEl, "squash", { direction: { skew } }));
    keep(flyStashFiles(islandEl, atollIndicatorRef.current, drop, flyCount, {
      onFirstHit: () => {
        if (cancelled) return;
        void pulseIslandShape({ heightDelta: 12, outMs: 130, backMs: 200 });
        keep(playSwallowRipple(islandEl, drop !== null ? drop.x : islandWidth / 2));
      },
    }));
    return cancel;
  }, [stashReaction, stashReactionKey, motionPaused, reducedMotion]);

  const [stashToast, setStashToast] = useState<{ text: string; key: number } | null>(null);
  const [stashToastLeaving, setStashToastLeaving] = useState(false);
  const stashToastTimerRef = useRef(0);
  const stashToastLeaveTimerRef = useRef(0);
  useEffect(() => {
    if (!lastStageResult) {
      return;
    }
    const result = lastStageResult;
    const parts: string[] = [];
    if (result.added > 0) {
      parts.push(t("fileStation.stagedToast", { count: result.added }));
    }
    if (result.evicted > 0) {
      parts.push(t("fileStation.evicted", { count: result.evicted }));
    }
    if (result.skipped > 0) {
      parts.push(t("fileStation.skipped", { count: result.skipped }));
    }
    if (parts.length === 0) {
      return;
    }
    setStashToast({ text: parts.join(" · "), key: Date.now() });
    setStashToastLeaving(false);
    window.clearTimeout(stashToastTimerRef.current);
    window.clearTimeout(stashToastLeaveTimerRef.current);
    // 先播 260ms 退场（is-leaving），再卸载——不再硬消失。
    stashToastLeaveTimerRef.current = window.setTimeout(() => {
      setStashToastLeaving(true);
    }, 2540);
    stashToastTimerRef.current = window.setTimeout(() => {
      setStashToast(null);
      setStashToastLeaving(false);
    }, 2800);
    return () => {
      window.clearTimeout(stashToastTimerRef.current);
      window.clearTimeout(stashToastLeaveTimerRef.current);
    };
  }, [lastStageResult]);
  useEffect(
    () => () => {
      window.clearTimeout(stashToastTimerRef.current);
      window.clearTimeout(stashToastLeaveTimerRef.current);
    },
    [],
  );

  return { stashToast, stashToastLeaving };
}
