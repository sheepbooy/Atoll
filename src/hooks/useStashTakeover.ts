// File-station takeover animation (logo grows to fill the island) plus the
// island-shape choreography: native window gulp pulse, jelly squash/chew
// beats, and drop-point measurement for the fly-in origin. Extracted from
// App.tsx; behavior otherwise unchanged.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { TFunction } from "i18next";
import type { AtollReaction } from "../AtollLogo";
import { ATOLL_REACTION_MS, ISLAND_SHAPE_MS, TAKEOVER_EXIT_MS } from "../atollTransitions";
import { motionDelay } from "../animationTiming";
import { playIslandShape } from "../islandShape";
import { pulseIslandShape } from "../tauri";

interface UseStashTakeoverOptions {
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

export function useStashTakeover({
  stashReaction,
  stashReactionKey,
  lastStageResult,
  islandRef,
  atollIndicatorRef,
  dropPointRef,
  spitAngleRef,
  t,
}: UseStashTakeoverOptions) {
  const [takeover, setTakeover] = useState<{ reaction: AtollReaction; key: number } | null>(null);
  const [takeoverExiting, setTakeoverExiting] = useState(false);
  const takeoverTimerRef = useRef(0);
  const takeoverMountedRef = useRef(false);
  const takeoverElRef = useRef<HTMLDivElement | null>(null);
  const beatTimersRef = useRef<number[]>([]);
  useEffect(() => {
    if (stashReaction) {
      // 减少动态效果：不做整岛接管（logo 的反应动画同样被关闭）。
      if (window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
        return;
      }
      window.clearTimeout(takeoverTimerRef.current);
      takeoverMountedRef.current = true;
      setTakeoverExiting(false);
      setTakeover({ reaction: stashReaction, key: stashReactionKey });
      return;
    }
    if (!takeoverMountedRef.current) {
      return;
    }
    takeoverMountedRef.current = false;
    setTakeoverExiting(true);
    // 退场动画时长来自 --takeover-exit-ms（spit 在元素上覆写为更快的 240ms），
    // JS 卸载定时器多留 20ms 余量，避免两处硬编码人肉同步。
    takeoverTimerRef.current = window.setTimeout(() => {
      setTakeoverExiting(false);
      setTakeover(null);
    }, motionDelay(TAKEOVER_EXIT_MS) + 20);
  }, [stashReaction, stashReactionKey]);
  useEffect(
    () => () => {
      window.clearTimeout(takeoverTimerRef.current);
      beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      beatTimersRef.current = [];
    },
    [],
  );

  // 每次接管开始时实测 header logo 相对整岛的位置，写入 CSS 变量：任何岛
  // 尺寸（会话 560x320 / 设置页 680x680）下都能精确“从角落长出/缩回角落”。
  // 同时把真实 drop 坐标换算成 takeover logo 的 SVG 用户单位（viewBox
  // -16,-36 宽 96 高 108；文件图元挂在 translate(32,36) 组上），吃入文件
  // 因此从用户松手的位置飞进嘴里。布局测量用 offset*（不受进场动画影响），
  // 且必须在挂上形变 class 之前完成。
  useLayoutEffect(() => {
    if (!takeover) {
      return;
    }
    const takeoverEl = takeoverElRef.current;
    const islandEl = islandRef.current;
    const logoEl = atollIndicatorRef.current;
    if (!takeoverEl || !islandEl || !logoEl) {
      return;
    }
    const islandRect = islandEl.getBoundingClientRect();
    const logoRect = logoEl.getBoundingClientRect();
    // 布局尺寸（offsetHeight）不受进场 scale 动画影响；rect 会。
    const takeoverLogo = takeoverEl.querySelector<HTMLElement>(".atoll-takeover-logo");
    const bigHeight = takeoverLogo ? takeoverLogo.offsetHeight : 0;
    const dx = logoRect.left + logoRect.width / 2 - (islandRect.left + islandRect.width / 2);
    const dy = logoRect.top + logoRect.height / 2 - (islandRect.top + islandRect.height / 2);
    const scale = bigHeight > 0 ? Math.max(0.08, logoRect.height / bigHeight) : 0.18;
    takeoverEl.style.setProperty("--takeover-dx", `${dx.toFixed(1)}px`);
    takeoverEl.style.setProperty("--takeover-dy", `${dy.toFixed(1)}px`);
    takeoverEl.style.setProperty("--takeover-scale", scale.toFixed(3));

    if (takeover.reaction === "spit") {
      // 吐出：轻量遮罩（is-spit 由 JSX className 提供）+ 快速退场；
      // 文件沿真实拖拽方向扇形飞出。
      takeoverEl.style.setProperty("--takeover-exit-ms", "240ms");
      const angle = spitAngleRef.current;
      if (angle !== null) {
        takeoverEl.style.setProperty("--spit-angle", `${(angle + 90).toFixed(1)}deg`);
      }
      return;
    }

    const drop = dropPointRef.current;
    if (takeoverLogo && drop && bigHeight > 0) {
      const nestedInTakeover = takeoverLogo.offsetParent === takeoverEl;
      const logoLeft = (nestedInTakeover ? takeoverEl.offsetLeft : 0) + takeoverLogo.offsetLeft;
      const logoTop = (nestedInTakeover ? takeoverEl.offsetTop : 0) + takeoverLogo.offsetTop;
      // 岛内坐标（tauri-runtime 下窗口即岛；demo 模式下减去岛原点）。
      const localX = drop.x - islandRect.left;
      const localY = drop.y - islandRect.top;
      // CSS px → SVG 用户单位（viewBox 原点 -16,-36），再减图元组原点 32,36。
      const svgX = -16 + ((localX - logoLeft) / takeoverLogo.offsetWidth) * 96 - 32;
      const svgY = -36 + ((localY - logoTop) / bigHeight) * 108 - 36;
      takeoverEl.style.setProperty("--eat-from-x", `${svgX.toFixed(1)}px`);
      takeoverEl.style.setProperty("--eat-from-y", `${svgY.toFixed(1)}px`);
      takeoverEl.style.setProperty("--eat-from-r", `${svgX >= 0 ? 24 : -24}deg`);
    }
  }, [takeover, islandRef, atollIndicatorRef, dropPointRef, spitAngleRef]);

  // 岛形编舞：
  // 吃 —— 命中下压 → 第一只文件落嘴时原生窗口"咕咚"脉冲 → 咀嚼期间岛身
  //      同频呼吸 → eat4 的 BURP 处岛身跟着抖一下。
  // 吐 —— 向拖拽反方向蓄力压缩（coil）→ 沿拖拽方向弹射 + 窗口负脉冲
  //      （果冻被捏了一下）→ 阻尼摆回（wobble）。
  useEffect(() => {
    if (!takeover) {
      return;
    }
    const islandEl = islandRef.current;
    if (!islandEl) {
      return;
    }
    const later = (fn: () => void, ms: number) => {
      beatTimersRef.current.push(window.setTimeout(fn, motionDelay(ms)));
    };

    if (takeover.reaction === "spit") {
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
        void pulseIslandShape({ heightDelta: -8, outMs: 120, backMs: 220 });
      }, ISLAND_SHAPE_MS.coil);
      later(() => {
        playIslandShape(islandEl, "wobble", { direction });
      }, ISLAND_SHAPE_MS.coil + 200);
      return () => {
        beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
        beatTimersRef.current = [];
      };
    }

    const reactionMs = ATOLL_REACTION_MS[takeover.reaction];
    const drop = dropPointRef.current;
    const islandWidth = islandEl.clientWidth || 1;
    const skew =
      drop !== null
        ? Math.max(-1, Math.min(1, (drop.x - islandWidth / 2) / (islandWidth / 2))) * 0.6
        : 0;
    playIslandShape(islandEl, "squash", { direction: { skew } });
    later(() => {
      void pulseIslandShape({ heightDelta: 12, outMs: 150, backMs: 240 });
    }, reactionMs * 0.22);
    later(() => {
      playIslandShape(islandEl, "chew", { durationMs: Math.round(reactionMs * 0.5) });
    }, reactionMs * 0.32);
    if (takeover.reaction === "eat4") {
      later(() => {
        playIslandShape(islandEl, "squash", { durationMs: 280 });
      }, reactionMs * 0.56);
    }
    return () => {
      beatTimersRef.current.forEach((timer) => window.clearTimeout(timer));
      beatTimersRef.current = [];
    };
  }, [takeover, islandRef, dropPointRef, spitAngleRef]);

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

  return { takeover, takeoverExiting, takeoverElRef, stashToast, stashToastLeaving };
}
