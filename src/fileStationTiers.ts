import type { AtollReaction } from "./AtollLogo";

export type StashBellyLevel = 0 | 1 | 2 | 3;

/**
 * 一次性"吃掉"反应档位（按吃下后的存量总数）：
 * - 1–2：eat1 小口吃
 * - 3–5：eat2 大快朵颐
 * - 6–9：eat3 吃撑冒汗
 * - ≥10：eat4 饱嗝特效
 */
export function eatReactionForCount(count: number): AtollReaction | null {
  if (count <= 0) return null;
  if (count <= 2) return "eat1";
  if (count <= 5) return "eat2";
  if (count <= 9) return "eat3";
  return "eat4";
}

/** 常驻鼓肚档位（有存货时肚子随数量变大）。 */
export function stashBellyLevel(count: number): StashBellyLevel {
  if (count <= 0) return 0;
  if (count <= 2) return 1;
  if (count <= 5) return 2;
  return 3;
}
