import { describe, expect, it } from "vitest";
import {
  aggregateByAgent,
  buildHeatmapGrid,
  buildTrendGeometry,
  buildTrendSeries,
  formatHeatmapDate,
  heatmapLevel,
  localDayKey,
  mergeByModelMax,
  summarizeHeatmap,
  tokenTotal,
} from "./tokenHeatmap";

describe("tokenHeatmap", () => {
  it("computes token totals from usage", () => {
    expect(tokenTotal({ inputTokens: 100, outputTokens: 50 })).toBe(150);
  });

  it("assigns heatmap intensity levels", () => {
    expect(heatmapLevel(0, 100)).toBe(0);
    expect(heatmapLevel(10, 100)).toBe(1);
    expect(heatmapLevel(40, 100)).toBe(2);
    expect(heatmapLevel(60, 100)).toBe(3);
    expect(heatmapLevel(90, 100)).toBe(4);
  });

  it("builds a 7 by 26 grid", () => {
    const grid = buildHeatmapGrid([]);
    expect(grid.rows).toHaveLength(7);
    expect(grid.rows[0]).toHaveLength(26);
  });

  it("today is always in range within the grid", () => {
    const todayKey = localDayKey(new Date());
    const grid = buildHeatmapGrid([
      { date: todayKey, inputTokens: 100, outputTokens: 50, cacheReadTokens: 0, cacheCreationTokens: 0, byAgent: {} },
    ]);
    const todayCell = grid.rows.flat().find((cell) => cell.date === todayKey);
    expect(todayCell).toBeDefined();
    expect(todayCell!.inRange).toBe(true);
    expect(todayCell!.total).toBe(150);
  });

  it("summarizes today, seven-day, and best day", () => {
    const today = localDayKey(new Date());
    const summary = summarizeHeatmap([
      { date: today, inputTokens: 100, outputTokens: 50 },
      { date: "2020-01-01", inputTokens: 1000, outputTokens: 0 },
    ]);
    expect(summary.today).toBe(150);
    expect(summary.best.total).toBe(1000);
  });

  it("aggregates token usage by agent across days", () => {
    const usage = (input: number, output: number) => ({
      inputTokens: input,
      outputTokens: output,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    });
    const days: Array<{ byAgent: Record<string, ReturnType<typeof usage>> }> = [
      { byAgent: { claude: usage(100, 50), codex: usage(200, 80) } },
      { byAgent: { claude: usage(300, 100), gemini: usage(50, 10) } },
    ];
    const slices = aggregateByAgent(days);
    expect(slices).toHaveLength(3);
    expect(slices[0].agent).toBe("claude");
    expect(slices[0].total).toBe(550);
    expect(slices[1].agent).toBe("codex");
    expect(slices[1].total).toBe(280);
    expect(slices[2].agent).toBe("gemini");
    expect(slices[2].total).toBe(60);
    expect(slices.reduce((sum, s) => sum + s.ratio, 0)).toBeCloseTo(1);
  });

  it("returns empty slices when no agent data", () => {
    expect(aggregateByAgent([{ byAgent: {} }])).toEqual([]);
  });

  it("allocates day cost across agents by token share in cost mode", () => {
    const usage = (input: number, output: number) => ({
      inputTokens: input,
      outputTokens: output,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    });
    const rate = {
      inputPerMillion: 2.5,
      outputPerMillion: 10,
      cacheReadPerMillion: 1.25,
      cacheWritePerMillion: 2.5,
    };
    const slices = aggregateByAgent(
      [
        {
          byAgent: { claude: usage(750_000, 0), cursor: usage(250_000, 0) },
          byModel: { "gpt-4o": usage(1_000_000, 0) },
        },
      ],
      "cost",
      { "gpt-4o": rate },
    );
    expect(slices).toHaveLength(2);
    expect(slices[0].agent).toBe("claude");
    expect(slices[0].total).toBeCloseTo(1.875, 4);
    expect(slices[1].agent).toBe("cursor");
    expect(slices[1].total).toBeCloseTo(0.625, 4);
    expect(slices[0].ratio).toBeCloseTo(0.75, 4);
  });

  it("formats valid heatmap dates and preserves invalid input", () => {
    expect(formatHeatmapDate("2026-07-06")).toBe("Mon, Jul 6, 2026");
    expect(formatHeatmapDate("not-a-date")).toBe("not-a-date");
    expect(formatHeatmapDate("2026-02-31")).toBe("2026-02-31");
  });

  it("builds a 30-point trend series ending today", () => {
    const todayKey = localDayKey(new Date());
    const series = buildTrendSeries(
      [{ date: todayKey, inputTokens: 500, outputTokens: 200 }],
      30,
    );
    expect(series).toHaveLength(30);
    expect(series[series.length - 1].date).toBe(todayKey);
    expect(series[series.length - 1].total).toBe(700);
    expect(series[0].total).toBe(0);
  });

  it("builds a monotone cubic trend path that stays within the value range", () => {
    const pad = { top: 8, right: 10, bottom: 8, left: 10 };
    const values = [0, 40, 10, 80, 20];
    const geometry = buildTrendGeometry(values, 200, 100, pad);

    expect(geometry.points).toHaveLength(values.length);
    expect(geometry.linePath.startsWith("M ")).toBe(true);
    expect(geometry.linePath).toContain(" C ");
    expect(geometry.areaPath.endsWith(" Z")).toBe(true);
    expect(geometry.points[0].x).toBeCloseTo(pad.left);
    expect(geometry.points[values.length - 1].x).toBeCloseTo(200 - pad.right);

    const maxVal = Math.max(...values);
    const innerH = 100 - pad.top - pad.bottom;
    const baseline = 100 - pad.bottom;
    const top = pad.top;
    for (const point of geometry.points) {
      expect(point.y).toBeGreaterThanOrEqual(top - 0.01);
      expect(point.y).toBeLessThanOrEqual(baseline + 0.01);
    }

    const samples = sampleCubicPath(geometry.linePath);
    expect(samples.length).toBeGreaterThan(values.length);
    for (const sample of samples) {
      expect(sample.y).toBeGreaterThanOrEqual(top - 0.5);
      expect(sample.y).toBeLessThanOrEqual(baseline + 0.5);
    }

    const peak = geometry.points[3];
    expect(peak.y).toBeCloseTo(top + innerH - (80 / maxVal) * innerH);
  });

  it("merges byModel with component-wise max", () => {
    const usage = (input: number, output = 0) => ({
      inputTokens: input,
      outputTokens: output,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    });
    expect(
      mergeByModelMax(
        { "gpt-4o": usage(1_000_000), "old-model": usage(10) },
        { "gpt-4o": usage(200_000), "new-model": usage(50) },
      ),
    ).toEqual({
      "gpt-4o": usage(1_000_000),
      "old-model": usage(10),
      "new-model": usage(50),
    });
  });

  it("uses byModel pricing in cost mode", () => {
    const todayKey = localDayKey(new Date());
    const summary = summarizeHeatmap(
      [
        {
          date: todayKey,
          inputTokens: 1_000_000,
          outputTokens: 0,
          byModel: {
            "gpt-4o": {
              inputTokens: 1_000_000,
              outputTokens: 0,
              cacheReadTokens: 0,
              cacheCreationTokens: 0,
            },
          },
        },
      ],
      "cost",
      {
        "gpt-4o": {
          inputPerMillion: 2.5,
          outputPerMillion: 10,
          cacheReadPerMillion: 1.25,
          cacheWritePerMillion: 2.5,
        },
      },
    );
    expect(summary.today).toBeCloseTo(2.5, 4);
  });
});

function sampleCubicPath(path: string): Array<{ x: number; y: number }> {
  const samples: Array<{ x: number; y: number }> = [];
  const tokens = path.trim().split(/[ ,]+/);
  let i = 0;
  let current = { x: 0, y: 0 };

  const read = () => Number(tokens[i++]);

  while (i < tokens.length) {
    const command = tokens[i++];
    if (command === "M") {
      current = { x: read(), y: read() };
      samples.push({ ...current });
    } else if (command === "C") {
      const c1 = { x: read(), y: read() };
      const c2 = { x: read(), y: read() };
      const end = { x: read(), y: read() };
      for (const t of [0.25, 0.5, 0.75, 1]) {
        samples.push(cubicBezier(current, c1, c2, end, t));
      }
      current = end;
    } else if (command === "L") {
      current = { x: read(), y: read() };
      samples.push({ ...current });
    } else {
      break;
    }
  }
  return samples;
}

function cubicBezier(
  p0: { x: number; y: number },
  p1: { x: number; y: number },
  p2: { x: number; y: number },
  p3: { x: number; y: number },
  t: number,
): { x: number; y: number } {
  const mt = 1 - t;
  return {
    x: mt ** 3 * p0.x + 3 * mt ** 2 * t * p1.x + 3 * mt * t ** 2 * p2.x + t ** 3 * p3.x,
    y: mt ** 3 * p0.y + 3 * mt ** 2 * t * p1.y + 3 * mt * t ** 2 * p2.y + t ** 3 * p3.y,
  };
}
