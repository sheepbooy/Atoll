// @ts-expect-error 项目未安装 @types/node；vitest 运行于 node，运行时可用
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS } from "./atollTransitions";

// vitest 始终以仓库根目录为 CWD 运行
const css = readFileSync("./src/styles.css", "utf8");

interface PhaseRule {
  activity: string;
  phase: "enter" | "exit";
  durations: number[];
}

/** Collect root-body enter/exit rules (`.atoll-logo.is-X.is-phase-Y { … }`). */
function collectRootPhaseRules(): PhaseRule[] {
  const rules: PhaseRule[] = [];
  const re = /\.atoll-logo\.is-([a-z]+)\.is-phase-(enter|exit)\s*\{([^}]*)\}/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(css))) {
    const durations = [...match[3].matchAll(/(\d+(?:\.\d+)?)s\b/g)].map((d) => Number(d[1]));
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

  it("reduced-motion block covers the atoll logo", () => {
    expect(css).toMatch(/@media \(prefers-reduced-motion: reduce\)[\s\S]{0,600}\.atoll-logo/s);
  });
});
