export type AtollPhase = "enter" | "loop" | "exit";

/** Duration of idle → activity enter choreography (ms). Keep in sync with CSS. */
export const ATOLL_ENTER_MS = 880;

/** Duration of activity → idle exit choreography (ms). Keep in sync with CSS. */
export const ATOLL_EXIT_MS = 680;

/** @deprecated Use IDLE_EASTER_EGG_ACTIVITIES from logoStates */
export { IDLE_EASTER_EGG_ACTIVITIES as EASTER_EGG_ACTIVITIES } from "./logoStates";
