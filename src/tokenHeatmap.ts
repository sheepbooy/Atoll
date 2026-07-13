import type { ModelRate } from "./pricing";
import { byModelCostUsd } from "./pricing";
import type { TokenUsage } from "./tauri";
import type { UsageDisplayMode } from "./displayPrefs";
import { resolveIntlLocale } from "./i18n";

export const HEATMAP_WEEKS = 26;

export function tokenTotal(usage: Pick<TokenUsage, "inputTokens" | "outputTokens">): number {
  return usage.inputTokens + usage.outputTokens;
}

export function maxTokenUsage(a: TokenUsage, b: TokenUsage): TokenUsage {
  return {
    inputTokens: Math.max(a.inputTokens, b.inputTokens),
    outputTokens: Math.max(a.outputTokens, b.outputTokens),
    cacheReadTokens: Math.max(a.cacheReadTokens ?? 0, b.cacheReadTokens ?? 0),
    cacheCreationTokens: Math.max(a.cacheCreationTokens ?? 0, b.cacheCreationTokens ?? 0),
  };
}

export function mergeByModelMax(
  a: Record<string, TokenUsage> = {},
  b: Record<string, TokenUsage> = {},
): Record<string, TokenUsage> {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  const merged: Record<string, TokenUsage> = {};
  for (const key of keys) {
    const left = a[key];
    const right = b[key];
    if (left && right) merged[key] = maxTokenUsage(left, right);
    else merged[key] = left ?? right!;
  }
  return merged;
}

export function dayDisplayTotal(
  day: {
    inputTokens: number;
    outputTokens: number;
    byModel?: Record<string, TokenUsage>;
  },
  displayMode: UsageDisplayMode = "tokens",
  pricingRates: Record<string, ModelRate> = {},
): number {
  if (displayMode === "cost") {
    return byModelCostUsd(day.byModel, pricingRates);
  }
  return tokenTotal(day);
}

export function formatHeatmapDate(dateKey: string): string {
  const [year, month, day] = dateKey.split("-").map(Number);
  if (!year || !month || !day) return dateKey;
  const date = new Date(year, month - 1, day);
  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return dateKey;
  }
  return date.toLocaleDateString(resolveIntlLocale(), {
    weekday: "short",
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

export function localDayKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function heatmapLevel(value: number, max: number): 0 | 1 | 2 | 3 | 4 {
  if (value <= 0 || max <= 0) return 0;
  const ratio = value / max;
  if (ratio >= 0.75) return 4;
  if (ratio >= 0.5) return 3;
  if (ratio >= 0.25) return 2;
  return 1;
}

export interface HeatmapCell {
  date: string;
  total: number;
  usage: TokenUsage;
  byAgent: Record<string, TokenUsage>;
  byModel: Record<string, TokenUsage>;
  inRange: boolean;
}

export interface HeatmapGrid {
  rows: HeatmapCell[][];
  maxTotal: number;
  startDate: string;
  endDate: string;
}

function startOfWeekMonday(date: Date): Date {
  const copy = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  const weekday = copy.getDay();
  const diff = weekday === 0 ? 6 : weekday - 1;
  copy.setDate(copy.getDate() - diff);
  return copy;
}

function addDays(date: Date, days: number): Date {
  const copy = new Date(date.getFullYear(), date.getMonth(), date.getDate());
  copy.setDate(copy.getDate() + days);
  return copy;
}

export function buildHeatmapGrid(
  days: Array<{
    date: string;
    inputTokens: number;
    outputTokens: number;
    cacheReadTokens: number;
    cacheCreationTokens: number;
    byAgent: Record<string, TokenUsage>;
    byModel?: Record<string, TokenUsage>;
  }>,
  weeks = HEATMAP_WEEKS,
  displayMode: UsageDisplayMode = "tokens",
  pricingRates: Record<string, ModelRate> = {},
): HeatmapGrid {
  const today = new Date();
  const endDate = localDayKey(today);
  const todayWeekStart = startOfWeekMonday(today);
  const gridStart = addDays(todayWeekStart, -(weeks - 1) * 7);
  const rangeStart = gridStart;

  const byDate = new Map(
    days.map((day) => [
      day.date,
      {
        total: dayDisplayTotal(day, displayMode, pricingRates),
        usage: {
          inputTokens: day.inputTokens,
          outputTokens: day.outputTokens,
          cacheReadTokens: day.cacheReadTokens,
          cacheCreationTokens: day.cacheCreationTokens,
        },
        byAgent: day.byAgent,
        byModel: day.byModel ?? {},
      },
    ]),
  );

  const columns: HeatmapCell[][] = Array.from({ length: weeks }, () => []);
  let maxTotal = 0;

  for (let week = 0; week < weeks; week += 1) {
    for (let row = 0; row < 7; row += 1) {
      const cellDate = addDays(gridStart, week * 7 + row);
      const dateKey = localDayKey(cellDate);
      const inRange = dateKey >= localDayKey(rangeStart) && dateKey <= endDate;
      const entry = byDate.get(dateKey);
      const total = entry?.total ?? 0;
      if (inRange && total > maxTotal) {
        maxTotal = total;
      }
      columns[week][row] = {
        date: dateKey,
        total,
        usage: entry?.usage ?? {
          inputTokens: 0,
          outputTokens: 0,
          cacheReadTokens: 0,
          cacheCreationTokens: 0,
        },
        byAgent: entry?.byAgent ?? {},
        byModel: entry?.byModel ?? {},
        inRange,
      };
    }
  }

  const rows = Array.from({ length: 7 }, (_, row) =>
    columns.map((column) => column[row]),
  );

  return {
    rows,
    maxTotal,
    startDate: localDayKey(rangeStart),
    endDate,
  };
}

export function summarizeHeatmap(
  days: Array<{
    date: string;
    inputTokens: number;
    outputTokens: number;
    byModel?: Record<string, TokenUsage>;
  }>,
  displayMode: UsageDisplayMode = "tokens",
  pricingRates: Record<string, ModelRate> = {},
) {
  const totals = days.map((day) => ({
    date: day.date,
    total: dayDisplayTotal(day, displayMode, pricingRates),
  }));
  const todayKey = localDayKey(new Date());
  const today = totals.find((day) => day.date === todayKey)?.total ?? 0;
  const lastSeven = totals.slice(-7);
  const sevenDay = lastSeven.reduce((sum, day) => sum + day.total, 0);
  const best = totals.reduce(
    (current, day) => (day.total > current.total ? day : current),
    { date: "", total: 0 },
  );
  return { today, sevenDay, best };
}

export interface AgentSlice {
  agent: string;
  total: number;
  ratio: number;
}

export function aggregateByAgent(
  days: Array<{
    byAgent: Record<string, TokenUsage>;
    byModel?: Record<string, TokenUsage>;
    inputTokens?: number;
    outputTokens?: number;
  }>,
  displayMode: UsageDisplayMode = "tokens",
  pricingRates: Record<string, ModelRate> = {},
): AgentSlice[] {
  const totals = new Map<string, number>();

  for (const day of days) {
    if (displayMode === "cost") {
      const dayCost = byModelCostUsd(day.byModel, pricingRates);
      if (dayCost <= 0) continue;
      const agentEntries = Object.entries(day.byAgent);
      const dayTokens = agentEntries.reduce((sum, [, usage]) => sum + tokenTotal(usage), 0);
      if (dayTokens <= 0) continue;
      for (const [agent, usage] of agentEntries) {
        const share = tokenTotal(usage) / dayTokens;
        totals.set(agent, (totals.get(agent) ?? 0) + dayCost * share);
      }
      continue;
    }

    for (const [agent, usage] of Object.entries(day.byAgent)) {
      totals.set(agent, (totals.get(agent) ?? 0) + tokenTotal(usage));
    }
  }

  const grand = Array.from(totals.values()).reduce((a, b) => a + b, 0);
  return Array.from(totals.entries())
    .map(([agent, total]) => ({
      agent,
      total,
      ratio: grand > 0 ? total / grand : 0,
    }))
    .sort((a, b) => b.total - a.total);
}

export interface TrendPoint {
  date: string;
  total: number;
}

export function buildTrendSeries(
  days: Array<{
    date: string;
    inputTokens: number;
    outputTokens: number;
    byModel?: Record<string, TokenUsage>;
  }>,
  n = 30,
  displayMode: UsageDisplayMode = "tokens",
  pricingRates: Record<string, ModelRate> = {},
): TrendPoint[] {
  const today = new Date();
  const result: TrendPoint[] = [];

  const byDate = new Map(
    days.map((day) => [day.date, dayDisplayTotal(day, displayMode, pricingRates)]),
  );

  for (let offset = n - 1; offset >= 0; offset -= 1) {
    const date = addDays(today, -offset);
    const key = localDayKey(date);
    result.push({ date: key, total: byDate.get(key) ?? 0 });
  }

  return result;
}
