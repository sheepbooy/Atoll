import type { NotchMetrics } from "./tauri";
import type { UsageDisplayMode } from "./displayPrefs";
import type { SalaryCurrency } from "./salarySettings";
import { formatCompactCost } from "./costFormat";
import { formatSalaryEarnings } from "./salaryFormat";
import {
  estimateTokenDisplayWidth,
  formatCompactTokenCount,
  tokenCompactLevel,
} from "./tokenCounterFormat";

/** Keep in sync with EXPANDED_WINDOW_WIDTH in src-tauri/src/lib.rs. */
export const COMPACT_MAX_WINDOW_WIDTH = 560;

export const COMPACT_ICON_SLOT = 24;
export const COMPACT_ICON_GAP = 4;
/** Global Atoll logo slot in the compact menu bar row. */
export const COMPACT_ATOLL_LOGO_SLOT = 34;
export const COMPACT_LISTENER_SLOT = 10;
export const COMPACT_SIDE_MIN = 56;
export const COMPACT_OVERFLOW_SLOT = 28;
export const COMPACT_OUTER_PADDING = 8;
/** Outer padding on notched displays: the capsule fuses with the camera
 *  housing, so its outer corners need more breathing room than the 8pt
 *  non-notch menu-bar row. Rendered via --compact-outer-padding (set in
 *  islandLayout.applyWindowMetrics) — keep the two in sync. */
export const COMPACT_NOTCH_OUTER_PADDING = 14;
export const COMPACT_NOTCH_INNER_GAP = 6;
/** Space between left sessions and right metrics on non-notched displays. */
export const COMPACT_HEADER_GAP = 8;
/** Fixed extra width budget on non-notched displays so `space-between` always
 *  has room to distribute as even spacing between logo and metrics. */
export const COMPACT_DISTRIBUTE_BUDGET = 40;
/** Width reserved for the scrolling-lyrics middle column on non-notched
 *  displays. Must match `.lyrics-marquee { width: 140px }` in styles.css. */
export const COMPACT_LYRICS_COLUMN = 140;
export const COMPACT_PENDING_BADGE_SLOT = 28;
/** Space between right session icons and the token counter in header-metrics. */
export const COMPACT_METRICS_GAP = 10;
/** Album-artwork thumbnail slot in compact mode (width + gap). */
export const COMPACT_MEDIA_THUMB_SLOT = 18 + COMPACT_METRICS_GAP;
/** One battery ring's width and the gap between rings. */
export const COMPACT_BATTERY_RING = 20;
export const COMPACT_BATTERY_RING_GAP = 4;
/** Ring count ceiling — keeps the capsule width bounded with noisy data. */
export const BATTERY_RING_MAX = 4;

/** Width budget for the battery rings: n rings + inner gaps + the metrics
 * gap that separates them from neighbors. 0 when there are none. */
export function batteryRingsSlot(count: number): number {
  if (count <= 0) {
    return 0;
  }
  const rings = Math.min(count, BATTERY_RING_MAX);
  return (
    rings * COMPACT_BATTERY_RING +
    (rings - 1) * COMPACT_BATTERY_RING_GAP +
    COMPACT_METRICS_GAP
  );
}

export const MIN_MAX_COMPACT_ICONS = 1;
export const ABSOLUTE_MAX_COMPACT_ICONS = 8;

export interface CompactHeaderLayout {
  leftIconCount: number;
  rightIconCount: number;
  overflowCount: number;
  tokenCompactLevel: number;
}

/** Counter body widths by formatting tier, including the inline scope mark.
 * Salary keeps two decimals; its single tier cannot be abbreviated by layout. */
export function estimateCompactCounterWidths(
  mode: UsageDisplayMode,
  value: number,
  currency: SalaryCurrency = "¥",
): number[] {
  return (mode === "salary" ? [0] : [0, 1, 2]).map((level) => {
    const text = mode === "salary"
      ? formatSalaryEarnings(value, currency, 0, value)
      : mode === "cost"
        ? formatCompactCost(value, level, value)
        : formatCompactTokenCount(value, level, value);
    // Fixed odometer cells already include letter spacing. Reserve 4px for
    // the mark and 5px for its gap; the salary prefix also has an auto-width
    // glyph and a small margin, corrected by the mounted body measurement.
    const prefixExtra = mode === "salary" ? 2 : 0;
    return Math.ceil(estimateTokenDisplayWidth(text) + 9 + prefixExtra);
  });
}

export interface CompactContentBudget {
  /** Empty when hidden; one tier for salary, three tiers for tokens/cost. */
  counterWidths?: readonly number[];
  hasMediaArtwork?: boolean;
  showMediaIndicator?: boolean;
  showLyrics?: boolean;
  batteryRingCount?: number;
  metricsGap?: number;
}

function compactExtras(notch: NotchMetrics, pendingCount: number, content: CompactContentBudget) {
  const gap = content.metricsGap ?? COMPACT_METRICS_GAP;
  const rings = content.batteryRingCount ?? 0;
  return {
    right: (pendingCount > 0 ? COMPACT_PENDING_BADGE_SLOT + gap : 0) +
      (content.showMediaIndicator && content.hasMediaArtwork ? 18 + gap : 0) +
      (rings > 0 ? batteryRingsSlot(rings) - COMPACT_METRICS_GAP + gap : 0),
    middle: content.showLyrics && !notch.hasNotch
      ? COMPACT_LYRICS_COLUMN + COMPACT_HEADER_GAP
      : 0,
  };
}

function counterWidthAtLevel(tokenTotal: number, level: number, content: CompactContentBudget) {
  return content.counterWidths !== undefined
    ? content.counterWidths[level] ?? 0
    : tokenTotal > 0
      ? estimateTokenDisplayWidth(formatCompactTokenCount(tokenTotal, level, tokenTotal))
      : 0;
}

function compactWingWidths(
  notch: NotchMetrics,
  layout: CompactHeaderLayout,
  tokenTotal: number,
  pendingCount: number,
  content: CompactContentBudget,
) {
  const outerPadding = compactOuterPadding(notch);
  const counterWidth = counterWidthAtLevel(tokenTotal, layout.tokenCompactLevel, content);
  return {
    left: COMPACT_ATOLL_LOGO_SLOT + COMPACT_LISTENER_SLOT + outerPadding +
      iconRowWidth(layout.leftIconCount) +
      (layout.overflowCount > 0 && layout.rightIconCount === 0 ? COMPACT_OVERFLOW_SLOT : 0),
    right: iconRowWidth(layout.rightIconCount) +
      (layout.overflowCount > 0 && layout.rightIconCount > 0 ? COMPACT_OVERFLOW_SLOT : 0) +
      (layout.rightIconCount > 0 && counterWidth > 0 ? content.metricsGap ?? COMPACT_METRICS_GAP : 0) +
      counterWidth + compactExtras(notch, pendingCount, content).right + outerPadding,
  };
}

export function iconRowWidth(count: number): number {
  if (count <= 0) return 0;
  return count * COMPACT_ICON_SLOT + (count - 1) * COMPACT_ICON_GAP;
}

/** Flex gap between right session icons and the token counter in header-metrics. */
export function compactMetricsSessionTokenGap(
  rightIconCount: number,
  hasToken: boolean,
): number {
  return rightIconCount > 0 && hasToken ? COMPACT_METRICS_GAP : 0;
}

/** Outer padding for the collapsed capsule on the given display: notched
 *  capsules fuse with the camera housing and need more breathing room at
 *  their outer corners. */
export function compactOuterPadding(
  notchMetrics: Pick<NotchMetrics, "hasNotch">,
): number {
  return notchMetrics.hasNotch
    ? COMPACT_NOTCH_OUTER_PADDING
    : COMPACT_OUTER_PADDING;
}

function notchPaneBudgets(notchMetrics: NotchMetrics) {
  if (!notchMetrics.hasNotch) return null;
  const leftArea = notchMetrics.leftAreaWidth || notchMetrics.width;
  const rightArea = notchMetrics.rightAreaWidth || notchMetrics.width;
  return {
    left: Math.max(
      COMPACT_SIDE_MIN,
      leftArea - COMPACT_OUTER_PADDING - COMPACT_NOTCH_INNER_GAP,
    ),
    right: Math.max(
      COMPACT_SIDE_MIN,
      rightArea - COMPACT_OUTER_PADDING - COMPACT_NOTCH_INNER_GAP,
    ),
  };
}

/** Width of the left header column — used to anchor the window on notched displays. */
export function computeCompactLeftPaneWidth(
  layout: Pick<
    CompactHeaderLayout,
    "leftIconCount" | "rightIconCount" | "overflowCount"
  >,
  outerPadding: number = COMPACT_OUTER_PADDING,
): number {
  const overflowOnLeft =
    layout.overflowCount > 0 && layout.rightIconCount === 0;
  return (
    COMPACT_ATOLL_LOGO_SLOT +
    COMPACT_LISTENER_SLOT +
    outerPadding +
    iconRowWidth(layout.leftIconCount) +
    (overflowOnLeft ? COMPACT_OVERFLOW_SLOT : 0) +
    COMPACT_NOTCH_INNER_GAP
  );
}

export function pickTokenCompactLevelForWidth(
  value: number,
  widthPx: number,
  sessionCount: number,
  maxCompactIcons: number,
): number {
  const fullText = formatCompactTokenCount(value, 0, value);
  if (estimateTokenDisplayWidth(fullText) <= widthPx) {
    return 0;
  }

  const sessionFloor = tokenCompactLevel(sessionCount, maxCompactIcons);
  for (const level of [1, 2]) {
    const text = formatCompactTokenCount(value, Math.max(sessionFloor, level), value);
    if (estimateTokenDisplayWidth(text) <= widthPx) {
      return Math.max(sessionFloor, level);
    }
  }

  return 2;
}

export function computeCompactHeaderLayout(
  notchMetrics: NotchMetrics,
  sessionCount: number,
  maxCompactIcons: number,
  tokenTotal: number,
  pendingCount: number,
  content: CompactContentBudget = {},
): CompactHeaderLayout {
  const visibleTarget = Math.min(sessionCount, maxCompactIcons);
  const notchWidth = notchMetrics.hasNotch ? notchMetrics.width : 0;
  const outerGaps = notchMetrics.hasNotch
    ? COMPACT_NOTCH_INNER_GAP * 2
    : COMPACT_HEADER_GAP;
  const extras = compactExtras(notchMetrics, pendingCount, content);
  const contentBudget = COMPACT_MAX_WINDOW_WIDTH - notchWidth - outerGaps - extras.middle;
  const hasToken = content.counterWidths !== undefined
    ? content.counterWidths.length > 0
    : tokenTotal > 0;
  const outerPadding = compactOuterPadding(notchMetrics);
  const leftBase =
    COMPACT_ATOLL_LOGO_SLOT + COMPACT_LISTENER_SLOT + outerPadding;
  const rightColumnBase = outerPadding + extras.right;

  const paneBudgets = notchPaneBudgets(notchMetrics);

  let best: CompactHeaderLayout = {
    leftIconCount: 0,
    rightIconCount: 0,
    overflowCount: sessionCount,
    tokenCompactLevel: content.counterWidths?.length === 1 ? 0 : 2,
  };
  let bestScore = Number.NEGATIVE_INFINITY;

  // Include smaller icon counts: optional metrics must fit before we commit
  // the native width, rather than clipping a too-wide row at the window cap.
  for (let visible = visibleTarget; visible >= 0; visible -= 1) {
    for (let left = visible; left >= 0; left -= 1) {
      const right = visible - left;
      const overflow = sessionCount - left - right;
      if (overflow < 0) continue;

      const overflowOnLeft = overflow > 0 && right === 0;
      const overflowOnRight = overflow > 0 && right > 0;
      const leftWidth =
        leftBase +
        iconRowWidth(left) +
        (overflowOnLeft ? COMPACT_OVERFLOW_SLOT : 0);
      const rightIconsWidth =
        iconRowWidth(right) + (overflowOnRight ? COMPACT_OVERFLOW_SLOT : 0);
      const sessionTokenGap = right > 0 && hasToken ? content.metricsGap ?? COMPACT_METRICS_GAP : 0;
      const tokenSpace =
        contentBudget -
        leftWidth -
        rightIconsWidth -
        sessionTokenGap -
        rightColumnBase;

      if (tokenSpace < 0) continue;

      const tokenLevel = content.counterWidths !== undefined
        ? Math.max(0, content.counterWidths.findIndex((width) => width <= tokenSpace))
        : pickTokenCompactLevelForWidth(tokenTotal, tokenSpace, sessionCount, maxCompactIcons);
      const tokenWidth = counterWidthAtLevel(tokenTotal, tokenLevel, content);
      const rightWidth =
        rightIconsWidth + sessionTokenGap + tokenWidth + rightColumnBase;

      if (leftWidth + rightWidth > contentBudget + 0.5) continue;

      if (paneBudgets) {
        if (leftWidth + COMPACT_NOTCH_INNER_GAP > paneBudgets.left + 0.5) continue;
        if (rightWidth + COMPACT_NOTCH_INNER_GAP > paneBudgets.right + 0.5) continue;
      }

      // Notch bars balance the rendered wing widths around the camera housing
      // (the logo/listener live in the left wing, the token counter in the
      // right, so equal icon counts alone are not visually symmetric).
      // No-notch bars keep every session on the left so CSS can space them
      // evenly in one row. The width penalty stays below one token-compression
      // tier so readability still wins over perfect symmetry.
      const sidePreference = notchMetrics.hasNotch
        ? -Math.abs(leftWidth - rightWidth) * 20 - Math.abs(left - right)
        : left * 1_000 + right * 100;
      const score =
        (left + right) * 1_000_000 -
        overflow * 100_000 -
        tokenLevel * 10_000 +
        (tokenLevel === 0 ? 5_000 : 0) +
        sidePreference -
        (notchMetrics.hasNotch && overflowOnLeft ? 30_000 : 0);

      if (score > bestScore) {
        bestScore = score;
        best = {
          leftIconCount: left,
          rightIconCount: right,
          overflowCount: overflow,
          tokenCompactLevel: tokenLevel,
        };
      }
    }
  }

  return best;
}

/** @deprecated Use computeCompactHeaderLayout for width calculations. */
export function computeCompactLeftWidth(
  shownIcons: number,
  hasOverflow: boolean,
): number {
  return (
    COMPACT_ATOLL_LOGO_SLOT +
    COMPACT_LISTENER_SLOT +
    iconRowWidth(shownIcons) +
    (hasOverflow ? COMPACT_OVERFLOW_SLOT : 0) +
    COMPACT_OUTER_PADDING
  );
}

export function computeCompactSideColumnBudget(
  notchMetrics: NotchMetrics,
): number {
  if (notchMetrics.hasNotch) {
    return (
      (COMPACT_MAX_WINDOW_WIDTH -
        notchMetrics.width -
        COMPACT_NOTCH_INNER_GAP * 2) /
      2
    );
  }

  return COMPACT_MAX_WINDOW_WIDTH - COMPACT_HEADER_GAP - COMPACT_SIDE_MIN;
}

export function computeMaxCompactIconLimit(
  notchMetrics: NotchMetrics,
): number {
  for (
    let icons = ABSOLUTE_MAX_COMPACT_ICONS;
    icons >= MIN_MAX_COMPACT_ICONS;
    icons -= 1
  ) {
    const layout = computeCompactHeaderLayout(
      notchMetrics,
      icons,
      icons,
      9_999_999_999,
      1,
    );
    if (
      layout.leftIconCount + layout.rightIconCount >= icons &&
      layout.overflowCount === 0
    ) {
      return icons;
    }
  }

  return MIN_MAX_COMPACT_ICONS;
}

/** One solution supplies both rendered icon placement and native geometry. */
export function computeCompactPresentation(
  notchMetrics: NotchMetrics,
  sessionCount: number,
  maxCompactIcons: number,
  tokenTotal: number,
  pendingCount: number,
  content: CompactContentBudget = {},
) {
  // Only tighten metrics spacing after all available icon-overflow choices
  // fail. This keeps full salary precision even at maximum wage with 4 rings.
  const gaps = content.counterWidths !== undefined ? [COMPACT_METRICS_GAP, 4, 1] : [COMPACT_METRICS_GAP];
  const paneBudgets = notchPaneBudgets(notchMetrics);
  let result;
  for (const metricsGap of gaps) {
    const budget = { ...content, metricsGap };
    const layout = computeCompactHeaderLayout(
      notchMetrics, sessionCount, maxCompactIcons, tokenTotal, pendingCount, budget,
    );
    const wings = compactWingWidths(notchMetrics, layout, tokenTotal, pendingCount, budget);
    const notchWidth = notchMetrics.hasNotch ? notchMetrics.width : 0;
    const outerGaps = notchMetrics.hasNotch ? COMPACT_NOTCH_INNER_GAP * 2 : COMPACT_HEADER_GAP;
    const contentWidth = notchWidth + wings.left + wings.right + outerGaps +
      compactExtras(notchMetrics, pendingCount, budget).middle;
    const distributeWidth = notchMetrics.hasNotch ? 0 : COMPACT_DISTRIBUTE_BUDGET;
    result = {
      layout,
      windowWidth: Math.min(COMPACT_MAX_WINDOW_WIDTH, Math.ceil(contentWidth + distributeWidth)),
      leftPaneWidth: computeCompactLeftPaneWidth(layout, compactOuterPadding(notchMetrics)),
      metricsGap,
    };
    if (contentWidth <= COMPACT_MAX_WINDOW_WIDTH && (!paneBudgets ||
      (wings.left + COMPACT_NOTCH_INNER_GAP <= paneBudgets.left &&
        wings.right + COMPACT_NOTCH_INNER_GAP <= paneBudgets.right))) break;
  }
  return result!;
}

export function computeCollapsedWindowWidth(
  notchMetrics: NotchMetrics,
  sessionCount: number,
  maxCompactIcons: number,
  tokenTotal: number,
  pendingCount: number,
  hasMediaArtwork = false,
  showMediaIndicator = false,
  showLyrics = false,
  batteryRingCount = 0,
): number {
  return computeCompactPresentation(
    notchMetrics, sessionCount, maxCompactIcons, tokenTotal, pendingCount,
    { hasMediaArtwork, showMediaIndicator, showLyrics, batteryRingCount },
  ).windowWidth;
}

/** Keep in sync with MICRO_WINDOW_WIDTH in src-tauri/src/lib.rs. */
export const MICRO_WINDOW_MIN_WIDTH = 72;
/** Windows micro header slots — keep in sync with .is-micro rules in styles.css. */
export const MICRO_LOGO_SLOT = 24;
export const MICRO_LISTENER_SLOT = 6;
export const MICRO_OUTER_PADDING = 10;
export const MICRO_HEADER_GAP = 3;
export const MICRO_INNER_GAP = 2;
export const MICRO_TOKEN_MARK_SLOT = 7;

/** Width budget for the Windows super-collapsed strip. */
export function computeMicroWindowWidth(
  sessionCount: number,
  tokenTotal: number,
  tokenCompactLevel = 0,
): number {
  if (sessionCount <= 0) {
    return MICRO_WINDOW_MIN_WIDTH;
  }

  const tokenText = formatCompactTokenCount(
    tokenTotal,
    tokenCompactLevel,
    tokenTotal,
  );
  const tokenWidth = tokenTotal > 0 ? estimateTokenDisplayWidth(tokenText) : 0;
  const content =
    MICRO_OUTER_PADDING +
    MICRO_LOGO_SLOT +
    MICRO_INNER_GAP +
    MICRO_LISTENER_SLOT +
    MICRO_HEADER_GAP +
    MICRO_TOKEN_MARK_SLOT +
    tokenWidth;

  return Math.max(MICRO_WINDOW_MIN_WIDTH, Math.ceil(content));
}
