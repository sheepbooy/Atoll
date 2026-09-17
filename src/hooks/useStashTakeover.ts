// File-station takeover animation (logo grows to fill the island) and the
// staged-files toast. Extracted from App.tsx; behavior unchanged.
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { RefObject } from "react";
import type { TFunction } from "i18next";
import type { AtollReaction } from "../AtollLogo";

interface UseStashTakeoverOptions {
  stashReaction: AtollReaction | null;
  stashReactionKey: number;
  lastStageResult: { added: number; evicted: number; skipped: number } | null;
  islandRef: RefObject<HTMLElement | null>;
  atollIndicatorRef: RefObject<HTMLSpanElement | null>;
  t: TFunction;
}

export function useStashTakeover({
  stashReaction,
  stashReactionKey,
  lastStageResult,
  islandRef,
  atollIndicatorRef,
  t,
}: UseStashTakeoverOptions) {
  const [takeover, setTakeover] = useState<{ reaction: AtollReaction; key: number } | null>(null);
  const [takeoverExiting, setTakeoverExiting] = useState(false);
  const takeoverTimerRef = useRef(0);
  const takeoverMountedRef = useRef(false);
  const takeoverElRef = useRef<HTMLDivElement | null>(null);
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
    takeoverTimerRef.current = window.setTimeout(() => {
      setTakeoverExiting(false);
      setTakeover(null);
    }, 340);
  }, [stashReaction, stashReactionKey]);
  useEffect(() => () => window.clearTimeout(takeoverTimerRef.current), []);
  // 每次接管开始时实测 header logo 相对整岛的位置，写入 CSS 变量：任何岛
  // 尺寸（会话 560x320 / 设置页 680x680）下都能精确“从角落长出/缩回角落”。
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
  }, [takeover]);

  const [stashToast, setStashToast] = useState<{ text: string; key: number } | null>(null);
  const stashToastTimerRef = useRef(0);
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
    window.clearTimeout(stashToastTimerRef.current);
    stashToastTimerRef.current = window.setTimeout(() => {
      setStashToast(null);
    }, 2800);
    return () => window.clearTimeout(stashToastTimerRef.current);
  }, [lastStageResult]);
  useEffect(() => () => window.clearTimeout(stashToastTimerRef.current), []);

  return { takeover, takeoverExiting, takeoverElRef, stashToast };
}
