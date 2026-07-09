import { describe, expect, it } from "vitest";
import { formatCompactCost } from "./costFormat";
import { byModelCostUsd, pricingRateMap, usageCostUsd } from "./pricing";

describe("costFormat", () => {
  it("formats small and large costs", () => {
    expect(formatCompactCost(0)).toBe("$0.00");
    expect(formatCompactCost(1.23)).toBe("$1.23");
    expect(formatCompactCost(1234, 1, 1234)).toBe("$1.2K");
  });
});

describe("pricing", () => {
  it("computes usage cost across all dimensions", () => {
    const cost = usageCostUsd(
      {
        inputTokens: 1_000_000,
        outputTokens: 500_000,
        cacheReadTokens: 100_000,
        cacheCreationTokens: 50_000,
      },
      {
        inputPerMillion: 3,
        outputPerMillion: 15,
        cacheReadPerMillion: 0.3,
        cacheWritePerMillion: 3.75,
      },
    );
    expect(cost).toBeCloseTo(10.7175, 4);
  });

  it("pricingRateMap skips unpriced models", () => {
    const map = pricingRateMap([
      {
        modelId: "a",
        displayName: "A",
        isCustom: false,
        isUnpriced: true,
        rate: {
          inputPerMillion: 0,
          outputPerMillion: 0,
          cacheReadPerMillion: 0,
          cacheWritePerMillion: 0,
        },
      },
      {
        modelId: "b",
        displayName: "B",
        isCustom: false,
        isUnpriced: false,
        rate: {
          inputPerMillion: 1,
          outputPerMillion: 2,
          cacheReadPerMillion: 0,
          cacheWritePerMillion: 0,
        },
      },
    ]);
    expect(map.a).toBeUndefined();
    expect(map.b.inputPerMillion).toBe(1);
  });

  it("excludes unknown models from by-model totals", () => {
    const total = byModelCostUsd(
      {
        _unknown: {
          inputTokens: 1_000_000,
          outputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
        "gpt-4o": {
          inputTokens: 1_000_000,
          outputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
      },
      {
        "gpt-4o": {
          inputPerMillion: 2.5,
          outputPerMillion: 10,
          cacheReadPerMillion: 1.25,
          cacheWritePerMillion: 2.5,
        },
      },
    );
    expect(total).toBeCloseTo(2.5, 4);
  });
});
