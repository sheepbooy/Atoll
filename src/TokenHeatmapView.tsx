import { useEffect, useMemo, useState } from "react";
import { formatCompactTokenCount } from "./tokenCounterFormat";
import {
  aggregateByAgent,
  buildHeatmapGrid,
  buildTrendSeries,
  formatHeatmapDate,
  HEATMAP_WEEKS,
  heatmapLevel,
  localDayKey,
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
  gemini: "is-gemini",
  other: "is-other",
};

const AGENT_COLOR: Record<string, string> = {
  claude: "#ff8b78",
  codex: "#61d8f7",
  gemini: "#b2e578",
  other: "#c9bcff",
};

const AGENT_LABEL: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  other: "Other",
};

interface TokenHeatmapViewProps {
  todayTokens: TokenUsage;
}

export function TokenHeatmapView({ todayTokens }: TokenHeatmapViewProps) {
  const [history, setHistory] = useState<TokenHistoryResponse | null>(null);
  const [hoveredDate, setHoveredDate] = useState<string | null>(null);

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

  const days = useMemo(() => {
    const todayKey = localDayKey(new Date());
    const base = history?.days ?? [];
    if (base.length === 0) {
      return [
        {
          date: todayKey,
          ...todayTokens,
          byAgent: {},
        },
      ];
    }
    return base.map((day) =>
      day.date === todayKey
        ? {
            ...day,
            inputTokens: todayTokens.inputTokens,
            outputTokens: todayTokens.outputTokens,
            cacheReadTokens: todayTokens.cacheReadTokens,
            cacheCreationTokens: todayTokens.cacheCreationTokens,
          }
        : day,
    );
  }, [history, todayTokens]);

  const grid = useMemo(() => buildHeatmapGrid(days), [days]);
  const summary = useMemo(() => summarizeHeatmap(days), [days]);
  const agentSlices = useMemo(() => aggregateByAgent(days), [days]);
  const trendSeries = useMemo(() => buildTrendSeries(days, 30), [days]);
  const hoveredCell =
    hoveredDate === null
      ? null
      : grid.rows.flat().find((cell) => cell.date === hoveredDate && cell.inRange) ?? null;
  const todayKey = localDayKey(new Date());
  const hasHistory = days.some((day) => tokenTotal(day) > 0);

  return (
    <div className="settings-view" data-no-drag>
      <div className="token-heatmap-view">
        <div className="token-heatmap-summary">
        <div className="token-heatmap-stat">
          <span className="token-heatmap-stat-label">Today</span>
          <span className="token-heatmap-stat-value">
            {formatCompactTokenCount(summary.today, summary.today >= 1_000 ? 1 : 0, summary.today)}
          </span>
        </div>
        <div className="token-heatmap-stat">
          <span className="token-heatmap-stat-label">7-day</span>
          <span className="token-heatmap-stat-value">
            {formatCompactTokenCount(summary.sevenDay, summary.sevenDay >= 1_000 ? 1 : 0, summary.sevenDay)}
          </span>
        </div>
        <div className="token-heatmap-stat">
          <span className="token-heatmap-stat-label">Best day</span>
          <span className="token-heatmap-stat-value">
            {summary.best.total > 0
              ? `${formatCompactTokenCount(summary.best.total, summary.best.total >= 1_000 ? 1 : 0, summary.best.total)}`
              : "—"}
          </span>
        </div>
      </div>

      {!hasHistory ? (
        <p className="token-heatmap-empty">Recording starts today.</p>
      ) : null}

      <div className="token-heatmap-scroll">
        <div className="token-heatmap-grid-wrap">
          <div className="token-heatmap-weekdays" aria-hidden="true">
            {WEEKDAY_LABELS.map((label, index) => (
              <span key={`${label}-${index}`} className="token-heatmap-weekday">
                {label}
              </span>
            ))}
          </div>
          <div className="token-heatmap-grid" role="grid" aria-label="Daily token activity">
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
                      aria-label={`${formatHeatmapDate(cell.date)}: ${cell.total.toLocaleString()} tokens`}
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
              {hoveredCell.total.toLocaleString()} tokens
            </span>
            <span className="token-heatmap-tooltip-detail">
              in {hoveredCell.usage.inputTokens.toLocaleString()} · out{" "}
              {hoveredCell.usage.outputTokens.toLocaleString()}
            </span>
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

      <div className="token-heatmap-charts">
        <AgentDonutChart slices={agentSlices} />
        <TrendLineChart series={trendSeries} />
      </div>
    </div>
    </div>
  );
}

/* ── Agent donut chart ─────────────────────────────────────────── */

const DONUT_SIZE = 64;
const DONUT_STROKE = 8;
const DONUT_RADIUS = (DONUT_SIZE - DONUT_STROKE) / 2;
const DONUT_CIRCUMFERENCE = 2 * Math.PI * DONUT_RADIUS;

function AgentDonutChart({ slices }: { slices: AgentSlice[] }) {
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
    <div className="token-heatmap-section">
      <span className="token-heatmap-section-label">Agent breakdown</span>
      <div className="token-heatmap-agents">
        <svg
          className="token-heatmap-donut"
          viewBox={`0 0 ${DONUT_SIZE} ${DONUT_SIZE}`}
          width={DONUT_SIZE}
          height={DONUT_SIZE}
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
                  {formatCompactTokenCount(slice.total, slice.total >= 1_000 ? 1 : 0, slice.total)}
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

const TREND_HEIGHT = 64;
const TREND_PADDING_TOP = 4;
const TREND_PADDING_BOTTOM = 0;

function TrendLineChart({ series }: { series: TrendPoint[] }) {
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

  return (
    <div className="token-heatmap-section">
      <div className="token-heatmap-section-header">
        <span className="token-heatmap-section-label">30-day trend</span>
        {hovered ? (
          <span className="token-heatmap-trend-hover">
            {formatShortDate(hovered.date)} · {hovered.total.toLocaleString()}
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
