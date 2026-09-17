/**
 * 动效时长的唯一出处与“减弱动态效果”判定。
 *
 * 三层各管一段，保持对齐：
 * - CSS：`@media (prefers-reduced-motion: reduce)` 全局规则把动画/过渡压到 1ms；
 * - JS 编排定时器：用 `motionDelay()` 包裹时长，降级时瞬时完成；
 * - Rust：`platform::prefers_reduced_motion()` 让原生窗口动画直接 snap。
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
