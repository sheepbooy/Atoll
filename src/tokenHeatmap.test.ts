import { describe, expect, it } from "vitest";
import {
  aggregateByAgent,
  buildHeatmapGrid,
  buildTrendSeries,
  formatHeatmapDate,
  heatmapLevel,
  localDayKey,
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
