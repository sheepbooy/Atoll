export type AtollPhase = "enter" | "loop" | "exit";

/** Duration of idle → activity enter choreography (ms). Injected as --atoll-enter-ms. */
export const ATOLL_ENTER_MS = 880;

/** Duration of activity → idle exit choreography (ms). Injected as --atoll-exit-ms. */
export const ATOLL_EXIT_MS = 680;

/** One-shot state-transition reactions (ms). Injected as --reaction-*-ms. */
export const ATOLL_REACTION_MS = {
  cheer: 1000,
  collapse: 1400,
  revive: 1200,
  /** 文件中转站：按存量总数分档的吃掉反应 + 打开面板时的吐出反应。 */
  eat1: 1400,
  eat2: 1800,
  eat3: 2200,
  eat4: 2600,
  spit: 1200,
} as const;

export type AtollReactionKind = keyof typeof ATOLL_REACTION_MS;

/** 整岛形变拍点时长（ms），注入为 --shape-*-ms。CSS keyframes 见
 *  styles/island.css 的 island-shape-*，由 islandShape.ts 编排挂摘。 */
export const ISLAND_SHAPE_MS = {
  /** drop 命中下压 → 弹回（320 = 120 压 + 200 弹）。 */
  squash: 320,
  /** 吐出蓄力：向拖拽反方向压缩。 */
  coil: 130,
  /** 顺拖拽方向弹射伸展（带过冲）。 */
  launch: 380,
  /** 弹射后阻尼摆回。 */
  wobble: 520,
  /** 咀嚼期间岛身同频呼吸：单周期 640ms（与 island.css 固定周期一致），
   *  总时长随 eat 反应由调用方传入 durationMs。 */
  chew: 640,
} as const;

export type IslandShapeBeat = keyof typeof ISLAND_SHAPE_MS;

/** 接管覆盖层退场时长（ms）：CSS atoll-takeover-vanish 与 JS 卸载定时器
 *  共享，注入为 --takeover-exit-ms，消除两处硬编码。 */
export const TAKEOVER_EXIT_MS = 320;
