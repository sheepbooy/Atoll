// @ts-expect-error 项目未安装 @types/node；vitest 运行于 node，运行时可用
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// The history view reuses the settings chrome, whose body is a grid. The
// history list is written against a flex-column parent (flex:1 +
// min-height:0 own the scroll); without the flex override below, a short
// panel squeezes the entries until their rows visually overlap.
const css = readFileSync("./src/styles/history.css", "utf8");

function ruleBlock(selector: string): string {
  const start = css.indexOf(selector);
  if (start === -1) throw new Error(`selector not found: ${selector}`);
  const open = css.indexOf("{", start);
  const close = css.indexOf("}", open);
  return css.slice(open + 1, close);
}

describe("approval history layout guards", () => {
  it("overrides the settings-body grid with a flex column for the history view", () => {
    const body = ruleBlock(".approval-history-view .settings-body");
    expect(body).toContain("display: flex");
    expect(body).toContain("flex-direction: column");
  });

  it("keeps history entries from being squeezed by the flex list", () => {
    expect(ruleBlock(".history-entry")).toContain("flex-shrink: 0");
    expect(ruleBlock(".history-load-more")).toContain("flex-shrink: 0");
    expect(ruleBlock(".history-toolbar")).toContain("flex-shrink: 0");
  });
});
