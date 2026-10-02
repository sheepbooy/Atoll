import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useCompactLayout } from "./useCompactLayout";
import { DEFAULT_SALARY_SETTINGS } from "../salarySettings";
import { computeMicroWindowWidth } from "../compactLayout";
import { setCompactLayout } from "../tauri";

vi.mock("../tauri", () => ({ setCompactLayout: vi.fn().mockResolvedValue(undefined) }));
type Options = Parameters<typeof useCompactLayout>[0];

function options(): Options {
  return {
    notchMetrics: { hasNotch: true, width: 200, height: 38, leftAreaWidth: 656, rightAreaWidth: 656 },
    sessions: [{ sessionId: "one", agent: "claude", cwd: "/tmp/one", pendingCount: 0, totalCount: 0,
      lastActivity: "2026-10-02T00:00:00Z", transcriptPath: null }],
    maxCompactIcons: 8, activeSessionTokenTotal: 0, activeSessionCostTotal: 0,
    foldedCounterDisplay: "tokens", salarySettings: DEFAULT_SALARY_SETTINGS, pendingCount: 0,
    nowPlayingTrack: { title: "Track", artist: null, album: null, duration: null,
      position: null, playing: true, artworkBase64: "art", app: null },
    compactIndicator: "both", lyricsEnabled: false, lyricsData: null, bluetoothRingCount: 0,
    phase: "compact", phaseRef: { current: "compact" }, usesMicroIslandRef: { current: false },
    supportsMicroIsland: false, collapsedModeRef: { current: "compact" }, collapsedWindowWidthRef: { current: 0 },
    compactLeftPaneWidthRef: { current: 0 }, microPresentationWidthRef: { current: 0 },
  };
}

describe("compact counter geometry", () => {
  beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(new Date(2026, 9, 2, 12)); vi.mocked(setCompactLayout).mockResolvedValue(undefined).mockClear(); });
  afterEach(() => vi.useRealTimers());

  it("replaces token sizing with salary and cost sizing and clears old measurements", () => {
    const input = options();
    const { result, rerender } = renderHook(useCompactLayout, { initialProps: input });
    const tokenWidth = result.current.collapsedWindowWidth;
    const salary = { ...input, foldedCounterDisplay: "salary" as const };
    rerender(salary);
    expect(result.current.collapsedWindowWidth).toBeGreaterThan(tokenWidth);
    expect(result.current.compactHeaderLayout.tokenCompactLevel).toBe(0);
    act(() => result.current.onCompactCounterWidthChange(100, 0));
    const measured = result.current.collapsedWindowWidth;
    const nativeCalls = vi.mocked(setCompactLayout).mock.calls.length;
    act(() => { result.current.onCompactCounterWidthChange(100, 0); vi.advanceTimersByTime(1000); });
    expect(vi.mocked(setCompactLayout).mock.calls).toHaveLength(nativeCalls);

    rerender(input);
    expect(result.current.collapsedWindowWidth).toBe(tokenWidth);
    rerender(salary);
    expect(result.current.collapsedWindowWidth).toBeLessThan(measured);
    rerender({ ...input, foldedCounterDisplay: "cost", activeSessionCostTotal: 1234.56 });
    expect(result.current.collapsedWindowWidth).toBeGreaterThan(tokenWidth);
  });

  it("invalidates salary measurements on settings changes and accepts midnight shrinkage", () => {
    const input = { ...options(), foldedCounterDisplay: "salary" as const };
    const { result, rerender } = renderHook(useCompactLayout, { initialProps: input });
    act(() => result.current.onCompactCounterWidthChange(100, 0));
    const previous = result.current.collapsedWindowWidth;
    rerender({ ...input, salarySettings: { ...input.salarySettings, currency: "€", monthlyWage: 0 } });
    expect(result.current.collapsedWindowWidth).toBeLessThan(previous);
    act(() => result.current.onCompactCounterWidthChange(80, 0));
    const beforeMidnight = result.current.collapsedWindowWidth;
    act(() => result.current.onCompactCounterWidthChange(40, 0));
    expect(beforeMidnight - result.current.collapsedWindowWidth).toBe(40);
  });

  it("freezes rendered icons and ignores measurements until a transition settles", () => {
    const input = { ...options(), foldedCounterDisplay: "salary" as const };
    const { result, rerender } = renderHook(useCompactLayout, { initialProps: input });
    const header = result.current.compactHeaderLayout;
    input.phaseRef.current = "closing";
    const closing = { ...input, phase: "closing" as const, bluetoothRingCount: 4 };
    vi.mocked(setCompactLayout).mockClear();
    rerender(closing);
    expect(result.current.compactHeaderLayout).toBe(header);
    const width = result.current.collapsedWindowWidth;
    act(() => result.current.onCompactCounterWidthChange(100, 0));
    expect(result.current.collapsedWindowWidth).toBe(width);
    expect(setCompactLayout).not.toHaveBeenCalled();
    input.phaseRef.current = "compact";
    rerender({ ...closing, phase: "compact" });
    act(() => result.current.onCompactCounterWidthChange(100, 0));
    expect(result.current.collapsedWindowWidth).toBeGreaterThan(width);
    expect(setCompactLayout).toHaveBeenCalled();
  });

  it("keeps the Windows micro width unchanged", () => {
    const input = options();
    input.phase = "micro";
    input.phaseRef.current = "micro";
    input.usesMicroIslandRef.current = true;
    input.supportsMicroIsland = true;
    const { rerender } = renderHook(useCompactLayout, { initialProps: input });
    const width = input.microPresentationWidthRef.current;
    expect(width).toBe(computeMicroWindowWidth(1, 0));
    rerender({ ...input, foldedCounterDisplay: "salary" });
    expect(input.microPresentationWidthRef.current).toBe(width);
    expect(setCompactLayout).not.toHaveBeenCalled();
  });

  it("retains measured widths for every tier so compression cannot oscillate", () => {
    const input = { ...options(), foldedCounterDisplay: "cost" as const,
      activeSessionCostTotal: 1234.56, bluetoothRingCount: 4 };
    const { result } = renderHook(useCompactLayout, { initialProps: input });
    expect(result.current.compactHeaderLayout.tokenCompactLevel).toBe(0);
    act(() => result.current.onCompactCounterWidthChange(150, 0));
    expect(result.current.compactHeaderLayout.tokenCompactLevel).toBe(1);
    act(() => result.current.onCompactCounterWidthChange(150, 1));
    expect(result.current.compactHeaderLayout.tokenCompactLevel).toBe(2);
    act(() => result.current.onCompactCounterWidthChange(60, 2));
    expect(result.current.compactHeaderLayout.tokenCompactLevel).toBe(2);
  });
});
