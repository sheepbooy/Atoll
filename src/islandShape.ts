import { ISLAND_SHAPE_MS, type IslandShapeBeat } from "./atollTransitions";
import { motionDelay, prefersReducedMotion } from "./animationTiming";

/** 方向向量（CSS px / deg），由调用方从真实拖拽方向换算：
 *  - dx/dy：弹射/摆动的位移方向与幅度；
 *  - skew：挤压时的倾斜角（符号即拖入侧）。 */
export interface IslandShapeDirection {
  dx?: number;
  dy?: number;
  skew?: number;
}

export interface PlayIslandShapeOptions {
  /** 覆盖默认拍点时长（如 chew 跟随 eat 反应总时长）。 */
  durationMs?: number;
  direction?: IslandShapeDirection;
}

const BEAT_CLASS: Record<IslandShapeBeat, string> = {
  squash: "is-shape-squash",
  coil: "is-shape-coil",
  launch: "is-shape-launch",
  wobble: "is-shape-wobble",
  chew: "is-shape-chew",
};

/** 每个岛元素按拍点分键跟踪摘除定时器：新拍点只清自己的旧定时器，
 *  不会误取消上一个拍点的摘除（否则上一个 class 会卡死——若卡的是
 *  chew 这类 infinite 动画，岛会永久停在形变里）。 */
const activeTimers = new WeakMap<HTMLElement, Map<IslandShapeBeat, number>>();

/**
 * 整岛形变编排器：把拍点 class + 方向变量挂到 `.island` 上，播完自动摘除。
 *
 * 约束（与原生窗口的配合）：
 * - 岛贴屏幕顶边、窗口由 CALayer mask 裁剪 → CSS 形变只能收缩/倾斜/位移，
 *   放大超出窗口会被裁掉；"吞咽鼓起"由原生窗口脉冲（pulse_island_shape）
 *   负责，这里只做窗口内的果冻形变。
 * - reduced-motion 时整段跳过（与全局 1ms 降级策略一致）。
 * - CSS 消费的变量：--shape-dx / --shape-dy / --shape-skew，默认 0。
 */
export function playIslandShape(
  island: HTMLElement | null,
  beat: IslandShapeBeat,
  options?: PlayIslandShapeOptions,
): void {
  if (!island || prefersReducedMotion()) {
    return;
  }
  const duration = motionDelay(options?.durationMs ?? ISLAND_SHAPE_MS[beat]);
  if (duration <= 0) {
    return;
  }
  let timers = activeTimers.get(island);
  if (!timers) {
    timers = new Map();
    activeTimers.set(island, timers);
  }
  const previousTimer = timers.get(beat);
  if (previousTimer !== undefined) {
    window.clearTimeout(previousTimer);
  }
  island.classList.remove(BEAT_CLASS[beat]);
  // 强制 reflow：同一拍点连续触发时让 keyframes 立即重启动画。
  void island.offsetWidth;
  if (options?.direction) {
    const { dx = 0, dy = 0, skew = 0 } = options.direction;
    island.style.setProperty("--shape-dx", `${dx.toFixed(2)}px`);
    island.style.setProperty("--shape-dy", `${dy.toFixed(2)}px`);
    island.style.setProperty("--shape-skew", `${skew.toFixed(2)}deg`);
  }
  island.classList.add(BEAT_CLASS[beat]);
  timers.set(
    beat,
    window.setTimeout(() => {
      island.classList.remove(BEAT_CLASS[beat]);
      timers.delete(beat);
    }, duration),
  );
}

/**
 * 复制飞行动画：一枚文件缩略从列表行位置飞向 header logo 嘴里（WAAPI
 * 驱动的一次性 DOM 节点，结束自动移除）。用于点击复制与拖出锚定失败的
 * 复制回退。reduced-motion 时跳过。
 */
export function flyCopyGlyph(
  island: HTMLElement | null,
  logoEl: HTMLElement | null,
  originRect: DOMRect | null,
): void {
  if (!island || !originRect || prefersReducedMotion()) {
    return;
  }
  const islandRect = island.getBoundingClientRect();
  const fromX = originRect.left - islandRect.left + originRect.width / 2;
  const fromY = originRect.top - islandRect.top + originRect.height / 2;
  let toX = fromX;
  let toY = fromY - 48;
  if (logoEl) {
    const logoRect = logoEl.getBoundingClientRect();
    toX = logoRect.left + logoRect.width / 2 - islandRect.left;
    toY = logoRect.top + logoRect.height / 2 - islandRect.top;
  }
  const glyph = document.createElement("div");
  glyph.className = "stash-copy-glyph";
  glyph.style.left = `${fromX.toFixed(1)}px`;
  glyph.style.top = `${fromY.toFixed(1)}px`;
  island.appendChild(glyph);
  const dx = toX - fromX;
  const dy = toY - fromY;
  const animation = glyph.animate(
    [
      { transform: "translate(-50%, -50%) scale(1) rotate(0deg)", opacity: 1 },
      {
        transform: `translate(calc(-50% + ${(dx * 0.5).toFixed(1)}px), calc(-50% + ${(dy * 0.5 - 16).toFixed(1)}px)) scale(0.85) rotate(-12deg)`,
        opacity: 1,
        offset: 0.55,
      },
      {
        transform: `translate(calc(-50% + ${dx.toFixed(1)}px), calc(-50% + ${dy.toFixed(1)}px)) scale(0.25) rotate(-20deg)`,
        opacity: 0.35,
      },
    ],
    { duration: motionDelay(520), easing: "cubic-bezier(0.3, 0.5, 0.3, 1)" },
  );
  animation.onfinish = () => glyph.remove();
  window.setTimeout(() => glyph.remove(), motionDelay(900));
}
