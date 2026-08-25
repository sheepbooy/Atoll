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

export interface TrendChartPadding {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface TrendChartPoint {
  x: number;
  y: number;
}

export interface TrendGeometry {
  linePath: string;
  areaPath: string;
  points: TrendChartPoint[];
}

/** Map daily totals into a monotone cubic SVG path in pixel space. */
export function buildTrendGeometry(
  values: number[],
  width: number,
  height: number,
  pad: TrendChartPadding,
): TrendGeometry {
  const innerW = Math.max(width - pad.left - pad.right, 0);
  const innerH = Math.max(height - pad.top - pad.bottom, 0);
  const maxVal = Math.max(...values, 1);
  const count = values.length;
  const points: TrendChartPoint[] = values.map((value, index) => ({
    x: pad.left + (count > 1 ? (index / (count - 1)) * innerW : innerW / 2),
    y: pad.top + innerH - (value / maxVal) * innerH,
  }));

  if (count === 0 || width <= 0 || height <= 0) {
    return { linePath: "", areaPath: "", points };
  }

  const linePath = monotoneCubicPath(points);
  const first = points[0];
  const last = points[count - 1];
  const baseline = height - pad.bottom;
  const areaPath = `${linePath} L ${last.x} ${baseline} L ${first.x} ${baseline} Z`;
  return { linePath, areaPath, points };
}

function monotoneCubicPath(points: TrendChartPoint[]): string {
  if (points.length === 0) return "";
  if (points.length === 1) return `M ${points[0].x} ${points[0].y}`;
  if (points.length === 2) {
    return `M ${points[0].x} ${points[0].y} L ${points[1].x} ${points[1].y}`;
  }

  const n = points.length;
  const dx: number[] = [];
  const slope: number[] = [];
  for (let i = 0; i < n - 1; i += 1) {
    const span = points[i + 1].x - points[i].x;
    dx[i] = span;
    slope[i] = span === 0 ? 0 : (points[i + 1].y - points[i].y) / span;
  }

  const tangent = new Array<number>(n);
  tangent[0] = slope[0];
  tangent[n - 1] = slope[n - 2];
  for (let i = 1; i < n - 1; i += 1) {
    if (slope[i - 1] * slope[i] <= 0) {
      tangent[i] = 0;
    } else {
      const h1 = dx[i - 1];
      const h2 = dx[i];
      const w1 = 2 * h2 + h1;
      const w2 = h2 + 2 * h1;
      tangent[i] = (w1 + w2) / (w1 / slope[i - 1] + w2 / slope[i]);
    }
  }

  for (let i = 0; i < n - 1; i += 1) {
    if (Math.abs(slope[i]) < 1e-12) {
      tangent[i] = 0;
      tangent[i + 1] = 0;
      continue;
    }
    const a = tangent[i] / slope[i];
    const b = tangent[i + 1] / slope[i];
    const sum = a * a + b * b;
    if (sum > 9) {
      const t = 3 / Math.sqrt(sum);
      tangent[i] = t * a * slope[i];
      tangent[i + 1] = t * b * slope[i];
    }
  }

  let d = `M ${points[0].x} ${points[0].y}`;
  for (let i = 0; i < n - 1; i += 1) {
    const h = dx[i];
    const c1x = points[i].x + h / 3;
    const c1y = points[i].y + (tangent[i] * h) / 3;
    const c2x = points[i + 1].x - h / 3;
    const c2y = points[i + 1].y - (tangent[i + 1] * h) / 3;
    d += ` C ${c1x} ${c1y} ${c2x} ${c2y} ${points[i + 1].x} ${points[i + 1].y}`;
  }
  return d;
}
