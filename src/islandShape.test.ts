import { afterEach, describe, expect, it, vi } from "vitest";
import { ISLAND_SHAPE_MS } from "./atollTransitions";
import { flyStashFiles, playIslandShape } from "./islandShape";
import { mockMotionPreference } from "./test-utils/motionPreference";

function makeIsland() {
  const island = document.createElement("div");
  document.body.appendChild(island);
  return island;
}

describe("islandShape", () => {
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

  it("cancels flights, trails and late hit callbacks together", () => {
    vi.useFakeTimers();
    const reduce = mockMotionPreference();
    const animations: { cancel: ReturnType<typeof vi.fn>; onfinish: (() => void) | null }[] = [];
    Object.defineProperty(HTMLElement.prototype, "animate", {
      configurable: true, value: vi.fn(() => {
        const animation = { cancel: vi.fn(), onfinish: null };
        animations.push(animation);
        return animation;
      }),
    });
    try {
      const island = makeIsland();
      const hit = vi.fn();
      const cancel = flyStashFiles(island, null, null, 3, { onFirstHit: hit });
      vi.advanceTimersByTime(150);
      expect(island.children.length).toBeGreaterThan(3);
      reduce(true);
      cancel();
      expect(island.children).toHaveLength(0);
      for (const animation of animations) expect(animation.cancel).toHaveBeenCalledOnce();
      vi.advanceTimersByTime(2000);
      expect(hit).not.toHaveBeenCalled();
      expect(island.children).toHaveLength(0);
      expect(vi.getTimerCount()).toBe(0);
    } finally {
      Reflect.deleteProperty(HTMLElement.prototype, "animate");
    }
  });
  it("adds the beat class and removes it after the beat duration", () => {
    vi.useFakeTimers();
    try {
      const island = makeIsland();
      playIslandShape(island, "squash");
      expect(island.classList.contains("is-shape-squash")).toBe(true);
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.squash - 1);
      expect(island.classList.contains("is-shape-squash")).toBe(true);
      vi.advanceTimersByTime(2);
      expect(island.classList.contains("is-shape-squash")).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("restarts the same beat when replayed mid-flight", () => {
    vi.useFakeTimers();
    try {
      const island = makeIsland();
      playIslandShape(island, "squash");
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.squash * 0.6);
      playIslandShape(island, "squash");
      // 第二次触发重新计时：完整时长后才摘除，而不是剩余时间。
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.squash * 0.4);
      expect(island.classList.contains("is-shape-squash")).toBe(true);
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.squash * 0.6 + 1);
      expect(island.classList.contains("is-shape-squash")).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("does not clobber another beat's removal timer", () => {
    vi.useFakeTimers();
    try {
      const island = makeIsland();
      playIslandShape(island, "launch");
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.launch - 50);
      playIslandShape(island, "wobble");
      // launch 在自己的时长摘除，不被 wobble 的启动取消。
      vi.advanceTimersByTime(50);
      expect(island.classList.contains("is-shape-launch")).toBe(false);
      expect(island.classList.contains("is-shape-wobble")).toBe(true);
      vi.advanceTimersByTime(ISLAND_SHAPE_MS.wobble + 1);
      expect(island.classList.contains("is-shape-wobble")).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it("writes direction variables for the CSS keyframes", () => {
    const island = makeIsland();
    playIslandShape(island, "squash", { direction: { dx: 3, dy: -2, skew: 0.6 } });
    expect(island.style.getPropertyValue("--shape-dx")).toBe("3.00px");
    expect(island.style.getPropertyValue("--shape-dy")).toBe("-2.00px");
    expect(island.style.getPropertyValue("--shape-skew")).toBe("0.60deg");
  });
});
