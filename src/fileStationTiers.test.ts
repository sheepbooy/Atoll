import { describe, expect, it } from "vitest";
// @ts-expect-error 项目未安装 @types/node；vitest 运行于 node，运行时可用
import { readFileSync } from "node:fs";
import { ATOLL_REACTION_MS } from "./atollTransitions";
import { eatReactionForCount, stashBellyLevel } from "./fileStationTiers";

const css = readFileSync("./src/styles.css", "utf8");

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
      const seconds = ATOLL_REACTION_MS[reaction] / 1000;
      const pattern = new RegExp(
        `\\.atoll-logo\\.is-reaction-${reaction}[^{]*\\{[^}]*${seconds}s`,
        "s",
      );
      expect(css, `${reaction} CSS duration`).toMatch(pattern);
    }
  });
});
