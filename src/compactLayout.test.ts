import { describe, expect, it } from "vitest";
import type { NotchMetrics } from "./tauri";
import {
  ABSOLUTE_MAX_COMPACT_ICONS,
  COMPACT_HEADER_GAP,
  COMPACT_MAX_WINDOW_WIDTH,
  COMPACT_METRICS_GAP,
  COMPACT_NOTCH_INNER_GAP,
  COMPACT_SIDE_MIN,
  compactMetricsSessionTokenGap,
  computeCollapsedWindowWidth,
  computeCompactHeaderLayout,
  computeCompactLeftPaneWidth,
  computeCompactLeftWidth,
  computeCompactSideColumnBudget,
  computeMaxCompactIconLimit,
  computeMicroWindowWidth,
  MICRO_WINDOW_MIN_WIDTH,
  type CompactHeaderLayout,
} from "./compactLayout";

/** Typical 14" MacBook Pro menu-bar geometry (logical pt). */
const NOTCH_14: NotchMetrics = {
  hasNotch: true,
  width: 200,
  height: 38,
  leftAreaWidth: 656,
  rightAreaWidth: 656,
};

const NO_NOTCH: NotchMetrics = {
  hasNotch: false,
  width: 0,
  height: 0,
};

function assertLayoutInvariants(
  notch: NotchMetrics,
  sessionCount: number,
  maxCompactIcons: number,
  tokenTotal: number,
  pendingCount: number,
  layout: CompactHeaderLayout,
) {
  const visibleCap = Math.min(sessionCount, maxCompactIcons);
  expect(layout.leftIconCount + layout.rightIconCount).toBe(visibleCap);
  expect(layout.leftIconCount + layout.rightIconCount + layout.overflowCount).toBe(
    sessionCount,
  );
  expect(layout.leftIconCount).toBeGreaterThanOrEqual(0);
  expect(layout.rightIconCount).toBeGreaterThanOrEqual(0);
  expect(layout.overflowCount).toBeGreaterThanOrEqual(0);

  if (layout.overflowCount > 0) {
    expect(layout.rightIconCount).toBeGreaterThan(0);
  }

  const width = computeCollapsedWindowWidth(
    notch,
    sessionCount,
    maxCompactIcons,
    tokenTotal,
    pendingCount,
  );
  expect(width).toBeLessThanOrEqual(COMPACT_MAX_WINDOW_WIDTH);
  if (notch.hasNotch) {
    expect(width).toBeGreaterThanOrEqual(notch.width);
  }

  const leftPane = computeCompactLeftPaneWidth(layout);
  if (notch.leftAreaWidth) {
    expect(leftPane).toBeLessThanOrEqual(
      notch.leftAreaWidth - 8,
    );
  }
}

describe("compactLayout", () => {
  it("computes left width for visible session icons", () => {
    expect(computeCompactLeftWidth(4, false)).toBe(34 + 10 + 4 * 24 + 3 * 4 + 8);
    expect(computeCompactLeftWidth(4, true)).toBe(34 + 10 + 4 * 24 + 3 * 4 + 28 + 8);
  });

  it("allows more icons on notched displays by spilling to the right", () => {
    const limit = computeMaxCompactIconLimit(NOTCH_14);
    expect(limit).toBeGreaterThanOrEqual(6);
    expect(limit).toBeLessThanOrEqual(ABSOLUTE_MAX_COMPACT_ICONS);
  });

  it("computes left pane width for notch anchoring", () => {
    expect(
      computeCompactLeftPaneWidth({
        leftIconCount: 4,
        rightIconCount: 0,
        overflowCount: 0,
      }),
    ).toBe(34 + 10 + 8 + 4 * 24 + 3 * 4 + 6);
  });

  it("computes side column budgets for notched and plain displays", () => {
    expect(computeCompactSideColumnBudget(NOTCH_14)).toBe(
      (COMPACT_MAX_WINDOW_WIDTH - NOTCH_14.width - COMPACT_NOTCH_INNER_GAP * 2) / 2,
    );
    expect(computeCompactSideColumnBudget(NO_NOTCH)).toBe(
      COMPACT_MAX_WINDOW_WIDTH - COMPACT_HEADER_GAP - COMPACT_SIDE_MIN,
    );
  });

  it("keeps all icons on the left when the bar is wide enough", () => {
    const layout = computeCompactHeaderLayout(NOTCH_14, 6, 8, 12_345, 0);

    expect(layout.leftIconCount).toBe(6);
    expect(layout.rightIconCount).toBe(0);
    expect(layout.overflowCount).toBe(0);
    expect(layout.tokenCompactLevel).toBe(0);
  });

  it("spills icons to the right before truncating to overflow", () => {
    const layout = computeCompactHeaderLayout(NOTCH_14, 10, 8, 50_000, 0);

    expect(layout.leftIconCount + layout.rightIconCount).toBe(8);
    expect(layout.overflowCount).toBe(2);
    expect(layout.rightIconCount).toBeGreaterThan(0);
  });

  it("prefers full token digits when the right side has room", () => {
    const layout = computeCompactHeaderLayout(NOTCH_14, 2, 8, 12_345, 0);
    expect(layout.tokenCompactLevel).toBe(0);
  });

  it("allows the full icon range on non-notched displays", () => {
    expect(computeMaxCompactIconLimit(NO_NOTCH)).toBe(ABSOLUTE_MAX_COMPACT_ICONS);
  });

  it("reserves metrics gap between right sessions and token counter", () => {
    expect(compactMetricsSessionTokenGap(2, true)).toBe(COMPACT_METRICS_GAP);
    expect(compactMetricsSessionTokenGap(0, true)).toBe(0);
    expect(compactMetricsSessionTokenGap(2, false)).toBe(0);
  });

  it("adds non-notch header gap between left sessions and right metrics", () => {
    const layout = computeCompactHeaderLayout(NO_NOTCH, 2, 8, 12_345, 0);
    const width = computeCollapsedWindowWidth(NO_NOTCH, 2, 8, 12_345, 0);

    expect(layout.rightIconCount).toBe(0);
    expect(width).toBeLessThanOrEqual(COMPACT_MAX_WINDOW_WIDTH);
    expect(width).toBeGreaterThanOrEqual(120 + COMPACT_HEADER_GAP);
  });

  it("includes metrics gap when sessions spill to the right on non-notch displays", () => {
    const withToken = computeCollapsedWindowWidth(NO_NOTCH, 10, 4, 12_345, 0);
    const withoutToken = computeCollapsedWindowWidth(NO_NOTCH, 10, 4, 0, 0);
    const layout = computeCompactHeaderLayout(NO_NOTCH, 10, 4, 12_345, 0);

    expect(layout.rightIconCount).toBeGreaterThan(0);
    expect(withToken - withoutToken).toBeGreaterThan(COMPACT_METRICS_GAP);
  });

  it("keeps collapsed width within the compact window cap on notch screens", () => {
    const width = computeCollapsedWindowWidth(
      NOTCH_14,
      8,
      8,
      1_250_000_000,
      1,
    );
    expect(width).toBeLessThanOrEqual(COMPACT_MAX_WINDOW_WIDTH);
  });

  it("uses the idle micro width when no sessions are active", () => {
    expect(computeMicroWindowWidth(0, 0, 0)).toBe(MICRO_WINDOW_MIN_WIDTH);
  });

  it("widens the micro island when sessions and tokens are present", () => {
    const width = computeMicroWindowWidth(1, 12_345, 0);
    expect(width).toBeGreaterThan(MICRO_WINDOW_MIN_WIDTH);
  });
});

describe("compactLayout session counts (icon limit = 4, notch)", () => {
  const maxCompactIcons = 4;
  const tokenTotal = 123_456;

  it.each([
    { sessions: 1, left: 1, right: 0, overflow: 0, tokenLevel: 0 },
    { sessions: 2, left: 2, right: 0, overflow: 0, tokenLevel: 0 },
    { sessions: 3, left: 3, right: 0, overflow: 0, tokenLevel: 0 },
    { sessions: 4, left: 4, right: 0, overflow: 0, tokenLevel: 0 },
    { sessions: 5, left: 3, right: 1, overflow: 1, tokenLevel: 0 },
    { sessions: 6, left: 3, right: 1, overflow: 2, tokenLevel: 0 },
    { sessions: 8, left: 3, right: 1, overflow: 4, tokenLevel: 0 },
    { sessions: 10, left: 3, right: 1, overflow: 6, tokenLevel: 0 },
  ])(
    "$sessions sessions → left=$left right=$right +$overflow",
    ({ sessions, left, right, overflow, tokenLevel }) => {
      const layout = computeCompactHeaderLayout(
        NOTCH_14,
        sessions,
        maxCompactIcons,
        tokenTotal,
        0,
      );

      expect(layout).toMatchObject({
        leftIconCount: left,
        rightIconCount: right,
        overflowCount: overflow,
        tokenCompactLevel: tokenLevel,
      });
      assertLayoutInvariants(
        NOTCH_14,
        sessions,
        maxCompactIcons,
        tokenTotal,
        0,
        layout,
      );
    },
  );
});

describe("compactLayout session counts (icon limit = 8, notch)", () => {
  const maxCompactIcons = 8;
  const tokenTotal = 12_345_678;

  it.each([1, 2, 4, 6, 8, 10, 12])("%i sessions stay within window budget", (sessions) => {
    const layout = computeCompactHeaderLayout(
      NOTCH_14,
      sessions,
      maxCompactIcons,
      tokenTotal,
      0,
    );

    assertLayoutInvariants(
      NOTCH_14,
      sessions,
      maxCompactIcons,
      tokenTotal,
      0,
      layout,
    );
    expect(layout.leftIconCount + layout.rightIconCount).toBe(
      Math.min(sessions, maxCompactIcons),
    );
  });

  it("fits all visible icons on the left when the notch bar is wide enough", () => {
    const layout = computeCompactHeaderLayout(
      NOTCH_14,
      8,
      maxCompactIcons,
      tokenTotal,
      0,
    );

    expect(layout.leftIconCount).toBe(8);
    expect(layout.rightIconCount).toBe(0);
    expect(layout.overflowCount).toBe(0);
  });
});

describe("compactLayout with pending badge", () => {
  it("still fits 4 sessions with pending count on notch display", () => {
    const layout = computeCompactHeaderLayout(NOTCH_14, 4, 4, 999_999, 3);

    expect(layout.leftIconCount).toBe(4);
    expect(layout.rightIconCount).toBe(0);
    assertLayoutInvariants(NOTCH_14, 4, 4, 999_999, 3, layout);

    const width = computeCollapsedWindowWidth(NOTCH_14, 4, 4, 999_999, 3);
    expect(width).toBeLessThanOrEqual(COMPACT_MAX_WINDOW_WIDTH);
  });
});
