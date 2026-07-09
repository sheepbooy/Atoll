import { describe, expect, it } from "vitest";
import { formatCompactCost } from "./costFormat";

describe("formatCompactCost", () => {
  it("keeps two decimal places for ordinary dollar amounts", () => {
    expect(formatCompactCost(0)).toBe("$0.00");
    expect(formatCompactCost(0.004)).toBe("$0.00");
    expect(formatCompactCost(0.12)).toBe("$0.12");
    expect(formatCompactCost(1.2)).toBe("$1.20");
    expect(formatCompactCost(44.8314)).toBe("$44.83");
    expect(formatCompactCost(100.4)).toBe("$100.40");
    expect(formatCompactCost(999.999)).toBe("$1000.00");
  });

  it("does not drop decimals when compact pressure is low", () => {
    expect(formatCompactCost(44.83, 1, 44.83)).toBe("$44.83");
    expect(formatCompactCost(100.4, 1, 100.4)).toBe("$100.40");
  });

  it("abbreviates only large amounts under compact pressure", () => {
    expect(formatCompactCost(1_250, 1, 1_250)).toBe("$1.25K");
    expect(formatCompactCost(1_250_000, 1, 1_250_000)).toBe("$1.25M");
    expect(formatCompactCost(1_250, 2, 1_250)).toBe("$1K");
  });
});
