import type { NotchMetrics } from "./tauri";
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
export const COMPACT_NOTCH_INNER_GAP = 6;
/** Space between left sessions and right metrics on non-notched displays. */
export const COMPACT_HEADER_GAP = 18;
export const COMPACT_PENDING_BADGE_SLOT = 28;
/** Space between right session icons and the token counter in header-metrics. */
export const COMPACT_METRICS_GAP = 10;

export const MIN_MAX_COMPACT_ICONS = 1;
export const ABSOLUTE_MAX_COMPACT_ICONS = 8;

export interface CompactHeaderLayout {
  leftIconCount: number;
  rightIconCount: number;
  overflowCount: number;
  tokenCompactLevel: number;
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
): number {
  const overflowOnLeft =
    layout.overflowCount > 0 && layout.rightIconCount === 0;
  return (
    COMPACT_ATOLL_LOGO_SLOT +
    COMPACT_LISTENER_SLOT +
    COMPACT_OUTER_PADDING +
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
): CompactHeaderLayout {
  const visibleTarget = Math.min(sessionCount, maxCompactIcons);
  const notchWidth = notchMetrics.hasNotch ? notchMetrics.width : 0;
  const outerGaps = notchMetrics.hasNotch
    ? COMPACT_NOTCH_INNER_GAP * 2
    : COMPACT_HEADER_GAP;
  const contentBudget = COMPACT_MAX_WINDOW_WIDTH - notchWidth - outerGaps;
  const hasToken = tokenTotal > 0;
  const pendingExtra =
    pendingCount > 0 ? COMPACT_PENDING_BADGE_SLOT + COMPACT_METRICS_GAP : 0;
  const leftBase =
    COMPACT_ATOLL_LOGO_SLOT + COMPACT_LISTENER_SLOT + COMPACT_OUTER_PADDING;
  const rightColumnBase = COMPACT_OUTER_PADDING + pendingExtra;

  const paneBudgets = notchPaneBudgets(notchMetrics);

  let best: CompactHeaderLayout = {
    leftIconCount: 0,
    rightIconCount: 0,
    overflowCount: sessionCount,
    tokenCompactLevel: 2,
  };
  let bestScore = Number.NEGATIVE_INFINITY;

  for (let left = visibleTarget; left >= 0; left -= 1) {
    const right = visibleTarget - left;
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
    const sessionTokenGap = compactMetricsSessionTokenGap(right, hasToken);
    const tokenSpace =
      contentBudget -
      leftWidth -
      rightIconsWidth -
      sessionTokenGap -
      rightColumnBase;

    if (tokenSpace < 20) continue;

    const tokenLevel = pickTokenCompactLevelForWidth(
      tokenTotal,
      tokenSpace,
      sessionCount,
      maxCompactIcons,
    );
    const tokenText = formatCompactTokenCount(tokenTotal, tokenLevel, tokenTotal);
    const tokenWidth = hasToken ? estimateTokenDisplayWidth(tokenText) : 0;
    const rightWidth =
      rightIconsWidth + sessionTokenGap + tokenWidth + rightColumnBase;

    if (leftWidth + rightWidth > contentBudget + 0.5) continue;

    if (paneBudgets) {
      if (leftWidth + COMPACT_NOTCH_INNER_GAP > paneBudgets.left + 0.5) continue;
      if (rightWidth + COMPACT_NOTCH_INNER_GAP > paneBudgets.right + 0.5) continue;
    }

    const score =
      (left + right) * 1_000_000 -
      overflow * 100_000 -
      tokenLevel * 10_000 +
      (tokenLevel === 0 ? 5_000 : 0) +
      left * 1_000 +
      right * 100 -
      (overflowOnLeft ? 30_000 : 0);

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

export function computeCollapsedWindowWidth(
  notchMetrics: NotchMetrics,
  sessionCount: number,
  maxCompactIcons: number,
  tokenTotal: number,
  pendingCount: number,
): number {
  const layout = computeCompactHeaderLayout(
    notchMetrics,
    sessionCount,
    maxCompactIcons,
    tokenTotal,
    pendingCount,
  );

  const overflowOnLeft = layout.overflowCount > 0 && layout.rightIconCount === 0;
  const overflowOnRight = layout.overflowCount > 0 && layout.rightIconCount > 0;

  const leftWidth =
    COMPACT_ATOLL_LOGO_SLOT +
    COMPACT_LISTENER_SLOT +
    COMPACT_OUTER_PADDING +
    iconRowWidth(layout.leftIconCount) +
    (overflowOnLeft ? COMPACT_OVERFLOW_SLOT : 0);

  const hasToken = tokenTotal > 0;
  const tokenText = formatCompactTokenCount(
    tokenTotal,
    layout.tokenCompactLevel,
    tokenTotal,
  );
  const sessionTokenGap = compactMetricsSessionTokenGap(
    layout.rightIconCount,
    hasToken,
  );
  const rightWidth =
    iconRowWidth(layout.rightIconCount) +
    (overflowOnRight ? COMPACT_OVERFLOW_SLOT : 0) +
    sessionTokenGap +
    (hasToken ? estimateTokenDisplayWidth(tokenText) : 0) +
    (pendingCount > 0 ? COMPACT_PENDING_BADGE_SLOT + COMPACT_METRICS_GAP : 0) +
    COMPACT_OUTER_PADDING;

  const notchWidth = notchMetrics.hasNotch ? notchMetrics.width : 0;
  const outerGaps = notchMetrics.hasNotch
    ? COMPACT_NOTCH_INNER_GAP * 2
    : COMPACT_HEADER_GAP;

  if (notchMetrics.hasNotch) {
    return Math.min(
      COMPACT_MAX_WINDOW_WIDTH,
      Math.ceil(notchWidth + leftWidth + rightWidth + outerGaps),
    );
  }

  const contentWidth = leftWidth + rightWidth + outerGaps;
  const minWidth = COMPACT_SIDE_MIN * 2 + outerGaps;

  return Math.min(
    COMPACT_MAX_WINDOW_WIDTH,
    Math.ceil(Math.max(contentWidth, minWidth)),
  );
}
