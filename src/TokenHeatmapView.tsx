import { useEffect, useMemo, useRef, useState } from "react";
import { formatCompactCost } from "./costFormat";
import { formatCompactTokenCount } from "./tokenCounterFormat";
import type { UsageDisplayMode } from "./displayPrefs";
import type { ModelRate } from "./pricing";
import { byModelCostUsd } from "./pricing";
import {
  aggregateByAgent,
  buildHeatmapGrid,
  buildTrendSeries,
  dayDisplayTotal,
  formatHeatmapDate,
  HEATMAP_WEEKS,
  heatmapLevel,
  localDayKey,
  mergeByModelMax,
  maxTokenUsage,
  summarizeHeatmap,
  tokenTotal,
  type AgentSlice,
  type TrendPoint,
} from "./tokenHeatmap";
import { getTokenHistory, type TokenHistoryResponse, type TokenUsage } from "./tauri";

const WEEKDAY_LABELS = ["Mon", "", "Wed", "", "Fri", "", "Sun"];

const AGENT_DOT_CLASS: Record<string, string> = {
  claude: "is-claude",
  codex: "is-codex",
  cursor: "is-cursor",
  gemini: "is-gemini",
  other: "is-other",
};

const AGENT_COLOR: Record<string, string> = {
  claude: "#ff8b78",
  codex: "#61d8f7",
  cursor: "#a78bfa",
  gemini: "#b2e578",
  other: "#c9bcff",
};

const AGENT_LABEL: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  cursor: "Cursor",
  gemini: "Gemini",
  other: "Other",
};

interface TokenHeatmapViewProps {
  todayTokens: TokenUsage;
  todayTokensByModel?: Record<string, TokenUsage>;
  displayMode?: UsageDisplayMode;
  pricingRates?: Record<string, ModelRate>;
}

export function TokenHeatmapView({
  todayTokens,
  todayTokensByModel = {},
  displayMode = "tokens",
  pricingRates = {},
}: TokenHeatmapViewProps) {
  const [history, setHistory] = useState<TokenHistoryResponse | null>(null);
  const [hoveredDate, setHoveredDate] = useState<string | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let cancelled = false;
    getTokenHistory(HEATMAP_WEEKS * 7)
      .then((response) => {
        if (!cancelled) setHistory(response);
      })
      .catch(() => {
        if (!cancelled) {
          setHistory({ timezone: Intl.DateTimeFormat().resolvedOptions().timeZone, days: [] });
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    el.scrollLeft = el.scrollWidth;
  }, [history]);

  const days = useMemo(() => {
    const todayKey = localDayKey(new Date());
    const base = history?.days ?? [];
    if (base.length === 0) {
      return [
        {
          date: todayKey,
          ...todayTokens,
          byAgent: {},
          byModel: todayTokensByModel,
        },
      ];
    }
    return base.map((day) => {
      if (day.date !== todayKey) return day;
      const mergedUsage = maxTokenUsage(day, todayTokens);
      return {
        ...day,
        ...mergedUsage,
        byModel: mergeByModelMax(day.byModel, todayTokensByModel),
      };
    });
  }, [history, todayTokens, todayTokensByModel]);

  const grid = useMemo(
    () => buildHeatmapGrid(days, HEATMAP_WEEKS, displayMode, pricingRates),
    [days, displayMode, pricingRates],
  );
  const summary = useMemo(
    () => summarizeHeatmap(days, displayMode, pricingRates),
    [days, displayMode, pricingRates],
  );
  const agentSlices = useMemo(
    () => aggregateByAgent(days, displayMode, pricingRates),
    [days, displayMode, pricingRates],
  );
  const trendSeries = useMemo(
    () => buildTrendSeries(days, 30, displayMode, pricingRates),
    [days, displayMode, pricingRates],
  );
  const hoveredCell =
    hoveredDate === null
      ? null
      : grid.rows.flat().find((cell) => cell.date === hoveredDate && cell.inRange) ?? null;
  const todayKey = localDayKey(new Date());
  const hasHistory = days.some((day) => dayDisplayTotal(day, displayMode, pricingRates) > 0);
  const hasUnpricedHistory =
    displayMode === "cost" &&
    days.some((day) => tokenTotal(day) > 0 && byModelCostUsd(day.byModel, pricingRates) === 0);

  const formatSummaryValue = (value: number) =>
    displayMode === "cost"
      ? formatCompactCost(value, 0, value)
      : formatCompactTokenCount(value, value >= 1_000 ? 1 : 0, value);

  return (
    <div className="settings-view" data-no-drag>
      <div className="token-heatmap-view">
        <div className="token-heatmap-top">
          {hasUnpricedHistory ? (
            <p className="token-heatmap-empty">
              Older days without model metadata are not priced. Only new usage with known
              models appears in cost mode.
            </p>
          ) : null}

          <div className="token-heatmap-summary">
            <div className="token-heatmap-stat">
              <span className="token-heatmap-stat-label">Today</span>
              <span className="token-heatmap-stat-value">
                {formatSummaryValue(summary.today)}
              </span>
            </div>
            <div className="token-heatmap-stat">
              <span className="token-heatmap-stat-label">7-day</span>
              <span className="token-heatmap-stat-value">
                {formatSummaryValue(summary.sevenDay)}
              </span>
            </div>
            <div className="token-heatmap-stat">
              <span className="token-heatmap-stat-label">Best day</span>
              <span className="token-heatmap-stat-value">
                {summary.best.total > 0 ? formatSummaryValue(summary.best.total) : "—"}
              </span>
            </div>
          </div>

          {!hasHistory ? (
            <p className="token-heatmap-empty">Recording starts today.</p>
          ) : null}

          <div className="token-heatmap-scroll" ref={scrollRef}>
            <div className="token-heatmap-grid-wrap">
              <div className="token-heatmap-weekdays" aria-hidden="true">
                {WEEKDAY_LABELS.map((label, index) => (
                  <span key={`${label}-${index}`} className="token-heatmap-weekday">
                    {label}
                  </span>
                ))}
              </div>
              <div
                className="token-heatmap-grid"
                role="grid"
                aria-label={
                  displayMode === "cost" ? "Daily cost activity" : "Daily token activity"
                }
              >
                {grid.rows[0].map((_, weekIndex) => (
                  <div key={weekIndex} className="token-heatmap-week" role="row">
                    {grid.rows.map((row, rowIndex) => {
                      const cell = row[weekIndex];
                      const level = cell.inRange
                        ? heatmapLevel(cell.total, grid.maxTotal)
                        : 0;
                      const isToday = cell.date === todayKey;
                      return (
                        <button
                          key={`${cell.date}-${rowIndex}`}
                          type="button"
                          className={`token-heatmap-cell is-level-${level}${cell.inRange ? "" : " is-outside"}${isToday ? " is-today" : ""}`}
                          role="gridcell"
                          aria-label={`${formatHeatmapDate(cell.date)}: ${
                            displayMode === "cost"
                              ? formatCompactCost(cell.total, 0, cell.total)
                              : `${cell.total.toLocaleString()} tokens`
                          }`}
                          disabled={!cell.inRange}
                          onMouseEnter={() => setHoveredDate(cell.date)}
                          onMouseLeave={() => setHoveredDate(null)}
                          onFocus={() => setHoveredDate(cell.date)}
                          onBlur={() => setHoveredDate(null)}
                        />
                      );
                    })}
                  </div>
                ))}
              </div>
            </div>
          </div>

          <div className="token-heatmap-footer">
            <div className="token-heatmap-legend" aria-hidden="true">
              <span>Less</span>
              <span className="token-heatmap-cell is-level-0" />
              <span className="token-heatmap-cell is-level-1" />
              <span className="token-heatmap-cell is-level-2" />
              <span className="token-heatmap-cell is-level-3" />
              <span className="token-heatmap-cell is-level-4" />
              <span>More</span>
            </div>
            {hoveredCell ? (
              <div className="token-heatmap-tooltip" role="status">
                <span className="token-heatmap-tooltip-date">
                  {formatHeatmapDate(hoveredCell.date)}
                </span>
                <span className="token-heatmap-tooltip-total">
                  {displayMode === "cost"
                    ? formatCompactCost(hoveredCell.total, 0, hoveredCell.total)
                    : `${hoveredCell.total.toLocaleString()} tokens`}
                </span>
                {displayMode === "tokens" ? (
                  <span className="token-heatmap-tooltip-detail">
                    in {hoveredCell.usage.inputTokens.toLocaleString()} · out{" "}
                    {hoveredCell.usage.outputTokens.toLocaleString()}
                  </span>
                ) : (
                  <span className="token-heatmap-tooltip-detail">
                    Priced from model metadata only.
                  </span>
                )}
                {Object.keys(hoveredCell.byAgent).length > 0 ? (
                  <span className="token-heatmap-tooltip-agents">
                    {Object.entries(hoveredCell.byAgent).map(([agent, usage]) => (
                      <span
                        key={agent}
                        className={`token-heatmap-agent-dot ${AGENT_DOT_CLASS[agent] ?? "is-other"}`}
                        title={`${agent}: ${tokenTotal(usage).toLocaleString()}`}
                      />
                    ))}
                  </span>
                ) : null}
              </div>
            ) : (
              <span className="token-heatmap-timezone">
                {history?.timezone ?? Intl.DateTimeFormat().resolvedOptions().timeZone}
              </span>
            )}
          </div>
        </div>

        <div className="token-heatmap-charts">
          <AgentDonutChart slices={agentSlices} displayMode={displayMode} />
          <TrendLineChart series={trendSeries} displayMode={displayMode} />
        </div>
      </div>
    </div>
  );
}

/* ── Agent donut chart ─────────────────────────────────────────── */

const DONUT_SIZE = 120;
const DONUT_STROKE = 12;
const DONUT_RADIUS = (DONUT_SIZE - DONUT_STROKE) / 2;
const DONUT_CIRCUMFERENCE = 2 * Math.PI * DONUT_RADIUS;

function AgentDonutChart({
  slices,
  displayMode,
}: {
  slices: AgentSlice[];
  displayMode: UsageDisplayMode;
}) {
  const hasData = slices.length > 0;

  let offset = 0;
  const arcs = slices.map((slice) => {
    const dash = slice.ratio * DONUT_CIRCUMFERENCE;
    const gap = DONUT_CIRCUMFERENCE - dash;
    const arc = {
      agent: slice.agent,
      color: AGENT_COLOR[slice.agent] ?? AGENT_COLOR.other,
      dashArray: `${dash} ${gap}`,
      dashOffset: -offset,
    };
    offset += dash;
    return arc;
  });

  return (
    <div className="token-heatmap-section token-heatmap-section--agents">
      <span className="token-heatmap-section-label">Agent breakdown</span>
      <div className="token-heatmap-agents">
        <svg
          className="token-heatmap-donut"
          viewBox={`0 0 ${DONUT_SIZE} ${DONUT_SIZE}`}
          aria-hidden="true"
        >
          <circle
            cx={DONUT_SIZE / 2}
            cy={DONUT_SIZE / 2}
            r={DONUT_RADIUS}
            fill="none"
            stroke="rgba(255,255,255,0.08)"
            strokeWidth={DONUT_STROKE}
          />
          {hasData &&
            arcs.map((arc) => (
              <circle
                key={arc.agent}
                cx={DONUT_SIZE / 2}
                cy={DONUT_SIZE / 2}
                r={DONUT_RADIUS}
                fill="none"
                stroke={arc.color}
                strokeWidth={DONUT_STROKE}
                strokeDasharray={arc.dashArray}
                strokeDashoffset={arc.dashOffset}
                strokeLinecap="butt"
                transform={`rotate(-90 ${DONUT_SIZE / 2} ${DONUT_SIZE / 2})`}
              />
            ))}
        </svg>
        <div className="token-heatmap-agent-legend">
          {hasData ? (
            slices.map((slice) => (
              <div key={slice.agent} className="token-heatmap-agent-row">
                <span
                  className="token-heatmap-agent-swatch"
                  style={{ background: AGENT_COLOR[slice.agent] ?? AGENT_COLOR.other }}
                />
                <span className="token-heatmap-agent-name">
                  {AGENT_LABEL[slice.agent] ?? slice.agent}
                </span>
                <span className="token-heatmap-agent-pct">
                  {Math.round(slice.ratio * 100)}%
                </span>
                <span className="token-heatmap-agent-total">
                  {displayMode === "cost"
                    ? formatCompactCost(slice.total, 0, slice.total)
                    : formatCompactTokenCount(
                        slice.total,
                        slice.total >= 1_000 ? 1 : 0,
                        slice.total,
                      )}
                </span>
              </div>
            ))
          ) : (
            <span className="token-heatmap-no-data">No agent data</span>
          )}
        </div>
      </div>
    </div>
  );
}

/* ── Trend line chart ──────────────────────────────────────────── */

const TREND_HEIGHT = 140;
const TREND_PADDING_TOP = 8;
const TREND_PADDING_BOTTOM = 4;

function TrendLineChart({
  series,
  displayMode,
}: {
  series: TrendPoint[];
  displayMode: UsageDisplayMode;
}) {
  const [hoverIndex, setHoverIndex] = useState<number | null>(null);

  const maxVal = Math.max(...series.map((p) => p.total), 1);
  const chartHeight = TREND_HEIGHT - TREND_PADDING_TOP - TREND_PADDING_BOTTOM;
  const count = series.length;
  const hasData = series.some((p) => p.total > 0);

  function x(i: number) {
    return count > 1 ? (i / (count - 1)) * 100 : 50;
  }
  function y(value: number) {
    return TREND_PADDING_TOP + chartHeight - (value / maxVal) * chartHeight;
  }

  const linePoints = series.map((p, i) => `${x(i)},${y(p.total)}`).join(" ");
  const areaPoints = `0,${TREND_HEIGHT} ${linePoints} 100,${TREND_HEIGHT}`;

  const firstLabel = series.length > 0 ? formatShortDate(series[0].date) : "";
  const lastLabel = series.length > 0 ? formatShortDate(series[series.length - 1].date) : "";

  const hovered = hoverIndex !== null ? series[hoverIndex] : null;
  const hoverValue =
    hovered == null
      ? null
      : displayMode === "cost"
        ? formatCompactCost(hovered.total, 0, hovered.total)
        : hovered.total.toLocaleString();

  return (
    <div className="token-heatmap-section token-heatmap-section--trend">
      <div className="token-heatmap-section-header">
        <span className="token-heatmap-section-label">30-day trend</span>
        {hovered && hoverValue ? (
          <span className="token-heatmap-trend-hover">
            {formatShortDate(hovered.date)} · {hoverValue}
          </span>
        ) : null}
      </div>
      {!hasData ? (
        <span className="token-heatmap-no-data">No usage in the last 30 days</span>
      ) : (
        <svg
          className="token-heatmap-trend-svg"
          viewBox={`0 0 100 ${TREND_HEIGHT}`}
          preserveAspectRatio="none"
          onMouseLeave={() => setHoverIndex(null)}
        >
          <polygon
            points={areaPoints}
            fill="rgba(255,255,255,0.06)"
          />
          <polyline
            points={linePoints}
            fill="none"
            stroke="rgba(255,255,255,0.5)"
            strokeWidth="1"
            vectorEffect="non-scaling-stroke"
          />
          {series.map((p, i) => (
            <rect
              key={p.date}
              x={x(i) - 100 / count / 2}
              y={0}
              width={100 / count}
              height={TREND_HEIGHT}
              fill="transparent"
              onMouseEnter={() => setHoverIndex(i)}
            />
          ))}
          {hoverIndex !== null ? (
            <circle
              cx={x(hoverIndex)}
              cy={y(series[hoverIndex].total)}
              r="2"
              fill="#fff"
              vectorEffect="non-scaling-stroke"
            />
          ) : null}
        </svg>
      )}
      <div className="token-heatmap-trend-axis">
        <span>{firstLabel}</span>
        <span>{lastLabel}</span>
      </div>
    </div>
  );
}

function formatShortDate(dateKey: string): string {
  const [year, month, day] = dateKey.split("-").map(Number);
  const date = new Date(year, month - 1, day);
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
