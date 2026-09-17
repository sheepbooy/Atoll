import { describe, expect, it } from "vitest";
import { ATOLL_REACTION_MS } from "./atollTransitions";
import { ANIMATION_TIMING_VARS } from "./animationTiming";
import { eatReactionForCount, stashBellyLevel } from "./fileStationTiers";
import { readStylesSource } from "./test-utils/stylesSource";

const css = readStylesSource();

describe("file station eat tiers", () => {
  it("maps staged counts to eat reactions", () => {
    expect(eatReactionForCount(0)).toBeNull();
    expect(eatReactionForCount(1)).toBe("eat1");
    expect(eatReactionForCount(2)).toBe("eat1");
    expect(eatReactionForCount(3)).toBe("eat2");
    expect(eatReactionForCount(5)).toBe("eat2");
    expect(eatReactionForCount(6)).toBe("eat3");
    expect(eatReactionForCount(9)).toBe("eat3");
    expect(eatReactionForCount(10)).toBe("eat4");
    expect(eatReactionForCount(30)).toBe("eat4");
  });

  it("maps staged counts to belly levels", () => {
    expect(stashBellyLevel(0)).toBe(0);
    expect(stashBellyLevel(1)).toBe(1);
    expect(stashBellyLevel(2)).toBe(1);
    expect(stashBellyLevel(3)).toBe(2);
    expect(stashBellyLevel(5)).toBe(2);
    expect(stashBellyLevel(6)).toBe(3);
    expect(stashBellyLevel(30)).toBe(3);
    expect(stashBellyLevel(-1)).toBe(0);
  });

  it("eat/spit reaction durations stay in sync with CSS", () => {
    for (const reaction of ["eat1", "eat2", "eat3", "eat4", "spit"] as const) {
      // 反应时长在 CSS 中以注入变量引用：规则必须指向对应 var，
      // 且 var 注册值与 TS 常量一致（单一出处即 animationTiming.ts）。
      expect(
        ANIMATION_TIMING_VARS[`--reaction-${reaction}-ms` as const],
        `${reaction} registered value`,
      ).toBe(`${ATOLL_REACTION_MS[reaction]}ms`);
      const pattern = new RegExp(
        `\\.atoll-logo\\.is-reaction-${reaction}[^{]*\\{[^}]*var\\(--reaction-${reaction}-ms\\)`,
        "s",
      );
      expect(css, `${reaction} CSS duration`).toMatch(pattern);
    }
  });
});
