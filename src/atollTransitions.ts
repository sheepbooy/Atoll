export type AtollPhase = "enter" | "loop" | "exit";

/** Duration of idle → activity enter choreography (ms). Injected as --atoll-enter-ms. */
export const ATOLL_ENTER_MS = 880;

/** Duration of activity → idle exit choreography (ms). Injected as --atoll-exit-ms. */
export const ATOLL_EXIT_MS = 680;

/** One-shot state-transition reactions (ms). Injected as --reaction-*-ms.
 *  文件中转站按"快、准、轻"提速：灵动 = 短拍，大批量才延长。 */
export const ATOLL_REACTION_MS = {
  cheer: 1000,
  collapse: 1400,
  revive: 1200,
  /** 文件中转站：按存量总数分档的吃掉反应 + 打开面板时的吐出反应。 */
  eat1: 900,
  eat2: 1100,
  eat3: 1350,
  eat4: 1600,
  spit: 700,
} as const;

export type AtollReactionKind = keyof typeof ATOLL_REACTION_MS;

/** 整岛形变拍点时长（ms），注入为 --shape-*-ms。CSS keyframes 见
 *  styles/island.css 的 island-shape-*，由 islandShape.ts 编排挂摘。 */
export const ISLAND_SHAPE_MS = {
  /** drop 命中下压 → 弹回（300 = 110 压 + 190 弹）。 */
  squash: 300,
  /** 吐出蓄力：向拖拽反方向压缩。 */
  coil: 110,
  /** 顺拖拽方向弹射伸展（带过冲）。 */
  launch: 340,
  /** 弹射后阻尼摆回。 */
  wobble: 480,
} as const;

export type IslandShapeBeat = keyof typeof ISLAND_SHAPE_MS;
