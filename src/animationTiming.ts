import {
  ATOLL_ENTER_MS,
  ATOLL_EXIT_MS,
  ATOLL_REACTION_MS,
  ISLAND_SHAPE_MS,
  TAKEOVER_EXIT_MS,
} from "./atollTransitions";
import {
  COLLAPSE_ANIMATION_MS,
  PANEL_EXIT_MS,
  RESOLVE_FEEDBACK_MS,
} from "./islandPresentation";

/**
 * 动效时长的唯一出处与“减弱动态效果”判定。
 *
 * 三层各管一段，保持对齐：
 * - CSS：`@media (prefers-reduced-motion: reduce)` 全局规则把动画/过渡压到 1ms；
 * - JS 编排定时器：用 `motionDelay()` 包裹时长，降级时瞬时完成；
 * - Rust：`platform::prefers_reduced_motion()` 让原生窗口动画直接 snap。
 *
 * TS 定时器与 CSS 动画共享的时长以 TS 常量为准，启动时注入为 CSS 变量
 * （见 ANIMATION_TIMING_VARS），CSS 一律 `var(...)` 引用、不落数字，
 * 从根上消除“Keep in sync with CSS”式的人肉同步。
 */

/** 系统是否开启了“减弱动态效果”。 */
export function prefersReducedMotion(): boolean {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** JS 编排定时器的时长：降级时归零，与 CSS 侧 1ms 降级行为对齐。 */
export function motionDelay(ms: number): number {
  return prefersReducedMotion() ? 0 : ms;
}

const ms = (value: number) => `${value}ms`;

/** 注入到 :root 的动效时长变量（值即 TS 常量本身）。 */
export const ANIMATION_TIMING_VARS = {
  "--atoll-enter-ms": ms(ATOLL_ENTER_MS),
  "--atoll-exit-ms": ms(ATOLL_EXIT_MS),
  "--reaction-cheer-ms": ms(ATOLL_REACTION_MS.cheer),
  "--reaction-collapse-ms": ms(ATOLL_REACTION_MS.collapse),
  "--reaction-revive-ms": ms(ATOLL_REACTION_MS.revive),
  "--reaction-eat1-ms": ms(ATOLL_REACTION_MS.eat1),
  "--reaction-eat2-ms": ms(ATOLL_REACTION_MS.eat2),
  "--reaction-eat3-ms": ms(ATOLL_REACTION_MS.eat3),
  "--reaction-eat4-ms": ms(ATOLL_REACTION_MS.eat4),
  "--reaction-spit-ms": ms(ATOLL_REACTION_MS.spit),
  "--shape-squash-ms": ms(ISLAND_SHAPE_MS.squash),
  "--shape-coil-ms": ms(ISLAND_SHAPE_MS.coil),
  "--shape-launch-ms": ms(ISLAND_SHAPE_MS.launch),
  "--shape-wobble-ms": ms(ISLAND_SHAPE_MS.wobble),
  "--takeover-exit-ms": ms(TAKEOVER_EXIT_MS),
  "--duration-expand": ms(COLLAPSE_ANIMATION_MS),
  "--panel-exit-ms": ms(PANEL_EXIT_MS),
  "--resolve-feedback-ms": ms(RESOLVE_FEEDBACK_MS),
} as const;

export type AnimationTimingVarName = keyof typeof ANIMATION_TIMING_VARS;

/** 在应用启动（首次 render 前）把时长变量写入 :root。 */
export function injectAnimationTimingVars(): void {
  if (typeof document === "undefined") return;
  for (const [name, value] of Object.entries(ANIMATION_TIMING_VARS)) {
    document.documentElement.style.setProperty(name, value);
  }
}
