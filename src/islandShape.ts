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
  /** 覆盖默认拍点时长。 */
  durationMs?: number;
  direction?: IslandShapeDirection;
}

const BEAT_CLASS: Record<IslandShapeBeat, string> = {
  squash: "is-shape-squash",
  coil: "is-shape-coil",
  launch: "is-shape-launch",
  wobble: "is-shape-wobble",
};

/** 每个岛元素按拍点分键跟踪摘除定时器：新拍点只清自己的旧定时器，
 *  不会误取消上一个拍点的摘除（否则上一个 class 会卡死在形变里）。 */
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

const FLY_MS = 380;
const FLY_STAGGER = 70;
/** 飞行弧线的抬升高度（px）：路径更"抛"，运动更可读。 */
const FLY_ARC = 22;

export interface FlyStashFilesOptions {
  /** 第一枚文件命中吉祥物（被吃掉）的时刻回调：触发原生脉冲与涟漪。 */
  onFirstHit?: () => void;
}

function stashGlyph(island: HTMLElement, x: number, y: number): HTMLDivElement {
  const glyph = document.createElement("div");
  glyph.className = "stash-fly-file";
  glyph.style.left = `${x.toFixed(1)}px`;
  glyph.style.top = `${y.toFixed(1)}px`;
  island.appendChild(glyph);
  return glyph;
}

function logoCenterIn(islandRect: DOMRect, logoEl: HTMLElement | null) {
  if (logoEl) {
    const rect = logoEl.getBoundingClientRect();
    return {
      x: rect.left + rect.width / 2 - islandRect.left,
      y: rect.top + rect.height / 2 - islandRect.top,
    };
  }
  return { x: islandRect.width / 2, y: 24 };
}

/** 飞行路径插值：直线 + 正弦弧线抬升，与主飞行动画的关键帧一致。 */
function flyPathPoint(
  sx: number,
  sy: number,
  dx: number,
  dy: number,
  fraction: number,
) {
  return {
    x: sx + dx * fraction,
    y: sy + dy * fraction - Math.sin(fraction * Math.PI) * FLY_ARC,
  };
}

/** 像素拖尾：在路径 fraction 处生成一枚快速消散的小方块。 */
function spawnTrail(island: HTMLElement, x: number, y: number): void {
  const dot = document.createElement("div");
  dot.className = "stash-fly-trail";
  dot.style.left = `${x.toFixed(1)}px`;
  dot.style.top = `${y.toFixed(1)}px`;
  island.appendChild(dot);
  const animation = dot.animate(
    [
      { transform: "translate(-50%, -50%) scale(1)", opacity: 0.75 },
      { transform: "translate(-50%, -50%) scale(0.4)", opacity: 0 },
    ],
    { duration: 280, easing: "ease-out", fill: "both" },
  );
  animation.onfinish = () => dot.remove();
  window.setTimeout(() => dot.remove(), 480);
}

/** 命中闪块：文件入嘴处在吉祥物位置弹出一枚像素星光。 */
function spawnHitSpark(island: HTMLElement, x: number, y: number): void {
  const spark = document.createElement("div");
  spark.className = "stash-fly-spark";
  spark.style.left = `${x.toFixed(1)}px`;
  spark.style.top = `${y.toFixed(1)}px`;
  island.appendChild(spark);
  const animation = spark.animate(
    [
      { transform: "translate(-50%, -50%) scale(0.3) rotate(0deg)", opacity: 0.95 },
      { transform: "translate(-50%, -50%) scale(1.35) rotate(45deg)", opacity: 0 },
    ],
    { duration: 260, easing: "cubic-bezier(0.2, 0.6, 0.3, 1)", fill: "both" },
  );
  animation.onfinish = () => spark.remove();
  window.setTimeout(() => spark.remove(), 460);
}

/**
 * 吸入飞行层：count 枚文件缩略从真实 drop 点错峰飞向角落吉祥物。
 * 文件全程保持醒目大小（起飞先弹一下），只在进嘴最后一瞬缩掉；
 * 路径上撒像素拖尾，每枚命中时弹一枚命中闪块。
 * DOM 一次性节点 + WAAPI，不经过 React。
 * reduced-motion 时跳过飞行但立即回调 onFirstHit（保持脉冲/涟漪语义）。
 */
export function flyStashFiles(
  island: HTMLElement | null,
  logoEl: HTMLElement | null,
  fromPoint: { x: number; y: number } | null,
  count: number,
  options?: FlyStashFilesOptions,
): void {
  if (!island) {
    return;
  }
  if (prefersReducedMotion()) {
    options?.onFirstHit?.();
    return;
  }
  const islandRect = island.getBoundingClientRect();
  const to = logoCenterIn(islandRect, logoEl);
  const fromX = fromPoint ? fromPoint.x : islandRect.width * 0.7;
  const fromY = fromPoint ? fromPoint.y : -10;
  let firstHitFired = false;
  for (let i = 0; i < count; i++) {
    const jitterX = (i - (count - 1) / 2) * 16;
    const startX = fromX + jitterX;
    const dx = to.x - startX;
    const dy = to.y - fromY;
    const delay = i * FLY_STAGGER;
    if (!firstHitFired) {
      firstHitFired = true;
      window.setTimeout(() => options?.onFirstHit?.(), delay + FLY_MS);
    }
    const glyph = stashGlyph(island, startX, fromY);
    const animation = glyph.animate(
      [
        { transform: "translate(-50%, -50%) scale(1) rotate(20deg)", opacity: 1 },
        {
          transform: `translate(calc(-50% + ${(dx * 0.12).toFixed(1)}px), calc(-50% + ${(dy * 0.12 - FLY_ARC * 0.55).toFixed(1)}px)) scale(1.16) rotate(8deg)`,
          opacity: 1,
          offset: 0.16,
        },
        {
          transform: `translate(calc(-50% + ${(dx * 0.55).toFixed(1)}px), calc(-50% + ${(dy * 0.55 - FLY_ARC * 0.8).toFixed(1)}px)) scale(1.05) rotate(-8deg)`,
          opacity: 1,
          offset: 0.6,
        },
        {
          transform: `translate(calc(-50% + ${dx.toFixed(1)}px), calc(-50% + ${dy.toFixed(1)}px)) scale(0.3) rotate(-18deg)`,
          opacity: 0.4,
        },
      ],
      {
        duration: FLY_MS,
        delay,
        easing: "cubic-bezier(0.3, 0.4, 0.35, 1)",
        fill: "both",
      },
    );
    animation.onfinish = () => {
      glyph.remove();
      spawnHitSpark(island, to.x, to.y);
    };
    window.setTimeout(() => glyph.remove(), delay + FLY_MS + 200);
    // 拖尾：沿同一路径在 22%/48%/74% 处撒小方块。
    for (const fraction of [0.22, 0.48, 0.74]) {
      const point = flyPathPoint(startX, fromY, dx, dy, fraction);
      window.setTimeout(
        () => spawnTrail(island, point.x, point.y),
        delay + FLY_MS * fraction,
      );
    }
  }
}

/**
 * 吐出飞行层：count 枚文件缩略从吉祥物嘴里沿拖拽方向（CSS deg）扇形飞出，
 * ±12° 散布、错峰 55ms、近大远小。DOM 一次性节点 + WAAPI。
 */
export function flySpitFiles(
  island: HTMLElement | null,
  logoEl: HTMLElement | null,
  angleDeg: number,
  count: number,
): void {
  if (!island || prefersReducedMotion()) {
    return;
  }
  const islandRect = island.getBoundingClientRect();
  const from = logoCenterIn(islandRect, logoEl);
  for (let i = 0; i < count; i++) {
    const spread = (i - (count - 1) / 2) * 12;
    const radians = ((angleDeg + spread) * Math.PI) / 180;
    const distance = 110 + Math.abs(spread) * 1.5;
    const dx = Math.cos(radians) * distance;
    const dy = Math.sin(radians) * distance;
    const delay = i * 55;
    const glyph = stashGlyph(island, from.x, from.y);
    const animation = glyph.animate(
      [
        { transform: "translate(-50%, -50%) scale(0.35) rotate(0deg)", opacity: 0 },
        {
          transform: `translate(calc(-50% + ${(dx * 0.3).toFixed(1)}px), calc(-50% + ${(dy * 0.3).toFixed(1)}px)) scale(1) rotate(10deg)`,
          opacity: 1,
          offset: 0.25,
        },
        {
          transform: `translate(calc(-50% + ${dx.toFixed(1)}px), calc(-50% + ${dy.toFixed(1)}px)) scale(0.85) rotate(22deg)`,
          opacity: 0,
        },
      ],
      {
        duration: 480,
        delay,
        easing: "cubic-bezier(0.2, 0.5, 0.4, 1)",
        fill: "both",
      },
    );
    animation.onfinish = () => glyph.remove();
    window.setTimeout(() => glyph.remove(), delay + 480 + 200);
  }
}

/**
 * 吞咽涟漪：岛底一条光带从命中点向两侧扩散一圈后淡出——"咽下去"的
 * 水面反馈，与 .island::after 的常驻光带同一视觉语言。
 */
export function playSwallowRipple(
  island: HTMLElement | null,
  xLocal: number,
): void {
  if (!island || prefersReducedMotion()) {
    return;
  }
  const ripple = document.createElement("div");
  ripple.className = "stash-gulp-ripple";
  ripple.style.left = `${Math.min(Math.max(xLocal, 0), island.clientWidth || 0).toFixed(1)}px`;
  island.appendChild(ripple);
  const animation = ripple.animate(
    [
      { transform: "translateX(-50%) scaleX(0.02)", opacity: 0.9 },
      { transform: "translateX(-50%) scaleX(1)", opacity: 0 },
    ],
    { duration: 620, easing: "cubic-bezier(0.2, 0.6, 0.3, 1)" },
  );
  animation.onfinish = () => ripple.remove();
  window.setTimeout(() => ripple.remove(), 900);
}

/**
 * 复制飞行动画：一枚文件缩略从列表行位置飞向 header logo（WAAPI 驱动的
 * 一次性 DOM 节点，结束自动移除）。用于点击复制与拖出锚定失败的复制回退。
 * reduced-motion 时跳过。
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
  const to = logoCenterIn(islandRect, logoEl);
  const dx = to.x - fromX;
  const dy = to.y - fromY;
  const glyph = stashGlyph(island, fromX, fromY);
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
