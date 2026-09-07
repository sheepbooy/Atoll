export type AtollPhase = "enter" | "loop" | "exit";

/** Duration of idle → activity enter choreography (ms). Keep in sync with CSS. */
export const ATOLL_ENTER_MS = 880;

/** Duration of activity → idle exit choreography (ms). Keep in sync with CSS. */
export const ATOLL_EXIT_MS = 680;

/** One-shot state-transition reactions (ms). Keep in sync with CSS. */
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
