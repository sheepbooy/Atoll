import { describe, expect, it } from "vitest";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS } from "./atollTransitions";
import {
  ANIMATION_TIMING_VARS,
  injectAnimationTimingVars,
} from "./animationTiming";
import { readStylesSource } from "./test-utils/stylesSource";

const css = readStylesSource();

/** var(--atoll-enter-ms) → 秒数，取自注入表的唯一出处。 */
function varToSeconds(name: string): number {
  const value = (ANIMATION_TIMING_VARS as Record<string, string>)[name];
  if (!value) throw new Error(`未注册的时长变量：${name}`);
  return Number(value.replace("ms", "")) / 1000;
}

interface PhaseRule {
  activity: string;
  phase: "enter" | "exit";
  /** 规则内出现的时长：字面秒数，或 var() 引用解析后的秒数。 */
  durations: number[];
}

/** Collect root-body enter/exit rules (`.atoll-logo.is-X.is-phase-Y { … }`). */
function collectRootPhaseRules(): PhaseRule[] {
  const rules: PhaseRule[] = [];
  const re = /\.atoll-logo\.is-([a-z]+)\.is-phase-(enter|exit)\s*\{([^}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(css))) {
    const durations = [
      ...[...match[3].matchAll(/(\d+(?:\.\d+)?)s\b/g)].map((d) => Number(d[1])),
      ...[...match[3].matchAll(/var\((--[\w-]+)\)/g)].map((v) =>
        varToSeconds(v[1]),
      ),
    ];
    rules.push({
      activity: match[1],
      phase: match[2] as "enter" | "exit",
      durations,
    });
  }
  return rules;
}

describe("atoll logo CSS/TS timing sync", () => {
  it("enter rules match ATOLL_ENTER_MS", () => {
    const enters = collectRootPhaseRules().filter((r) => r.phase === "enter");
    expect(enters.length).toBeGreaterThanOrEqual(6);
    for (const rule of enters) {
      expect(rule.durations, `${rule.activity} enter`).toContain(ATOLL_ENTER_MS / 1000);
    }
  });

  it("exit rules match ATOLL_EXIT_MS", () => {
    const exits = collectRootPhaseRules().filter((r) => r.phase === "exit");
    expect(exits.length).toBeGreaterThanOrEqual(6);
    for (const rule of exits) {
      expect(rule.durations, `${rule.activity} exit`).toContain(ATOLL_EXIT_MS / 1000);
    }
  });

  it("reduced-motion coverage is global (no curated selector list)", () => {
    expect(css).toMatch(
      /@media \(prefers-reduced-motion: reduce\)\s*\{\s*\*,\s*\*::before,\s*\*::after\s*\{/,
    );
  });

  it("injectAnimationTimingVars writes TS constants onto :root", () => {
    injectAnimationTimingVars();
    const rootStyle = document.documentElement.style;
    expect(rootStyle.getPropertyValue("--atoll-enter-ms")).toBe(`${ATOLL_ENTER_MS}ms`);
    expect(rootStyle.getPropertyValue("--atoll-exit-ms")).toBe(`${ATOLL_EXIT_MS}ms`);
    expect(rootStyle.getPropertyValue("--duration-expand")).toMatch(/ms$/);
  });

  it("coupled durations are not hardcoded in CSS sources", () => {
    // 与 JS 定时器共享的时长只允许出现在 animationTiming.ts；CSS 侧一旦
    // 重新出现字面量定义（含 tokens 兜底）即视为同步漂移。
    for (const name of ["--duration-expand", "--panel-exit-ms", "--resolve-feedback-ms"]) {
      expect(css, name).not.toMatch(new RegExp(`${name}\\s*:\\s*\\d`));
    }
    // 吉祥物 enter/exit 与反应时长不得再以裸数字出现在规则里。
    expect(css).not.toMatch(/\.atoll-logo\.is-[a-z]+\.is-phase-enter[^{]*\{[^}]*\b0\.88s/);
    expect(css).not.toMatch(/\.atoll-logo\.is-reaction-[a-z0-9]+[^{]*\{[^}]*\b1\.[46]s/);
  });
});
