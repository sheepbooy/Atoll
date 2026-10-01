import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AtollLogo } from "./AtollLogo";
import { TokenCounter } from "./TokenCounter";
import { TokenHeatmapView } from "./TokenHeatmapView";
import { waitForMotion } from "./animationTiming";
import { ATOLL_REACTION_MS } from "./atollTransitions";
import { mockMotionPreference } from "./test-utils/motionPreference";

describe("live animation lifecycle", () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

  it("finishes animation waits when reduced motion is enabled", async () => {
    const reduce = mockMotionPreference();
    const done = vi.fn();
    const wait = waitForMotion(1000).then(done);
    act(() => reduce(true));
    await wait;
    expect(done).toHaveBeenCalledOnce();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("lands a live token interpolation immediately and removes its delta", () => {
    const reduce = mockMotionPreference();
    const usage = { inputTokens: 900, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 };
    const { container, rerender } = render(<TokenCounter value={0} usage={usage} />);
    rerender(<TokenCounter value={900} usage={usage} />);
    expect(container.querySelector(".token-counter-delta")).not.toBeNull();
    act(() => reduce(true));
    expect(container.querySelector(".token-counter-delta")).toBeNull();
    expect(container.querySelector(".token-counter-wrap--live")).toBeNull();
    expect(container.textContent).toContain("900");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("restarts repeated reactions without remounting the mascot", () => {
    mockMotionPreference();
    const reaction = { animationName: "atoll-eat-eyes", currentTime: 100 };
    const loop = { animationName: "atoll-float", currentTime: 400 };
    Object.defineProperty(HTMLElement.prototype, "getAnimations", {
      configurable: true, value: vi.fn(() => [reaction, loop]),
    });
    try {
      const { container, rerender } = render(<AtollLogo reaction="eat1" reactionKey={1} />);
      const svg = container.querySelector("svg");
      expect(reaction.currentTime).toBe(0);
      reaction.currentTime = 150;
      rerender(<AtollLogo reaction="eat1" reactionKey={2} />);
      expect(reaction.currentTime).toBe(0);
      expect(loop.currentTime).toBe(400);
      expect(container.querySelector("svg")).toBe(svg);
    } finally {
      Reflect.deleteProperty(HTMLElement.prototype, "getAnimations");
    }
  });

  it("does not replay expired reactions after a window transition", () => {
    mockMotionPreference();
    const { container, rerender } = render(<AtollLogo reaction="eat1" reactionKey={1} />);
    rerender(<AtollLogo reaction="eat1" reactionKey={1} motionPaused />);
    act(() => vi.advanceTimersByTime(ATOLL_REACTION_MS.eat1 + 1));
    rerender(<AtollLogo reaction="eat1" reactionKey={1} />);
    expect(container.querySelector(".is-reaction-eat1")).toBeNull();
  });

  it("runs reading eye scans without React timers during a resize", () => {
    mockMotionPreference();
    const { container } = render(<AtollLogo activity="reading" motionPaused />);
    expect(container.querySelector(".atoll-eye-scan")).not.toBeNull();
    expect(container.querySelector(".is-motion-paused")).not.toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("draws the heatmap as soon as the native presentation is ready", async () => {
    const usage = { inputTokens: 100, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 };
    const { container, rerender } = render(<TokenHeatmapView todayTokens={usage} presentationReady={false} />);
    await act(async () => { await Promise.resolve(); });
    expect(container.querySelectorAll('[role="gridcell"]')).toHaveLength(0);
    rerender(<TokenHeatmapView todayTokens={usage} presentationReady />);
    expect(container.querySelectorAll('[role="gridcell"]').length).toBeGreaterThan(0);
    act(() => vi.advanceTimersByTime(16));
    expect(container.querySelector(".is-settled")).not.toBeNull();
  });
});
