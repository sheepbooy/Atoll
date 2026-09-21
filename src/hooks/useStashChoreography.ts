// 文件中转站"岛即生物"编舞：吉祥物只演眼睛（CSS 时间线驱动），这里负责
// 岛形拍点、原生窗口脉冲、吞咽涟漪、文件飞行层与结果 toast。
import { useEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { TFunction } from "i18next";
import type { AtollReaction } from "../AtollLogo";
import { ATOLL_REACTION_MS, ISLAND_SHAPE_MS } from "../atollTransitions";
import { motionDelay } from "../animationTiming";
import {
  flySpitFiles,
  flyStashFiles,
  playIslandShape,
  playSwallowRipple,
} from "../islandShape";
import { pulseIslandShape } from "../tauri";

interface UseStashChoreographyOptions {
  stashReaction: AtollReaction | null;
  stashReactionKey: number;
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
  lastStageResult,
  islandRef,
  atollIndicatorRef,
  dropPointRef,
  spitAngleRef,
  t,
}: UseStashChoreographyOptions) {
  const beatTimersRef = useRef<number[]>([]);
  // reduced-motion 下 CSS/JS 全部降级，这里直接不编排（toast 仍显示）。
  useEffect(() => {
    if (!stashReaction) {
      return;
    }
    if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      return;
    }
    const islandEl = islandRef.current;
    if (!islandEl) {
      return;
    }
    const later = (fn: () => void, ms: number) => {
      beatTimersRef.current.push(window.setTimeout(fn, motionDelay(ms)));
    };

    if (stashReaction === "spit") {
      const angle = spitAngleRef.current ?? -31;
      const radians = (angle * Math.PI) / 180;
      const direction = {
        dx: Math.cos(radians) * 4,
        dy: Math.sin(radians) * 4,
        skew: Math.sin(radians) * 0.5,
      };
      playIslandShape(islandEl, "coil", { direction });
      later(() => {
        playIslandShape(islandEl, "launch", { direction });
        void pulseIslandShape({ heightDelta: -8, outMs: 100, backMs: 190 });
        flySpitFiles(islandEl, atollIndicatorRef.current, angle, 3);
      }, ISLAND_SHAPE_MS.coil);
      later(() => {
        playIslandShape(islandEl, "wobble", { direction });
      }, ISLAND_SHAPE_MS.coil + 180);
      return () => {
        beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
        beatTimersRef.current = [];
      };
    }

    // 吃：命中下压 → 文件从真实 drop 点错峰飞向吉祥物；第一只命中时
    // 原生窗口"咕咚"脉冲 + 岛底吞咽涟漪。满足眯眼由 CSS 时间线自行发生。
    if (!Object.prototype.hasOwnProperty.call(EAT_FLY_COUNT, stashReaction)) {
      return;
    }
    const flyCount = EAT_FLY_COUNT[stashReaction as keyof typeof EAT_FLY_COUNT];
    const drop = dropPointRef.current;
    const islandWidth = islandEl.clientWidth || 1;
    const skew =
      drop !== null
        ? Math.max(-1, Math.min(1, (drop.x - islandWidth / 2) / (islandWidth / 2))) * 0.6
        : 0;
    playIslandShape(islandEl, "squash", { direction: { skew } });
    flyStashFiles(
      islandEl,
      atollIndicatorRef.current,
      drop,
      flyCount,
      {
        onFirstHit: () => {
          void pulseIslandShape({ heightDelta: 12, outMs: 130, backMs: 200 });
          playSwallowRipple(islandEl, drop !== null ? drop.x : islandWidth / 2);
        },
      },
    );
    return () => {
      beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      beatTimersRef.current = [];
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [stashReaction, stashReactionKey]);
  useEffect(
    () => () => {
      beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      beatTimersRef.current = [];
    },
    [],
  );

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
