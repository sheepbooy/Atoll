import { describe, expect, it, vi } from "vitest";
import { playIslandShape } from "./islandShape";

function makeIsland() {
  const island = document.createElement("div");
  document.body.appendChild(island);
  return island;
}

describe("islandShape", () => {
  it("adds the beat class and removes it after the beat duration", () => {
    vi.useFakeTimers();
    try {
      const island = makeIsland();
      playIslandShape(island, "squash");
      expect(island.classList.contains("is-shape-squash")).toBe(true);
      vi.advanceTimersByTime(319);
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
      vi.advanceTimersByTime(200);
      playIslandShape(island, "squash");
      // 第二次触发重新计时：320ms 后才摘除，而不是剩余的 120ms。
      vi.advanceTimersByTime(120);
      expect(island.classList.contains("is-shape-squash")).toBe(true);
      vi.advanceTimersByTime(201);
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
      vi.advanceTimersByTime(330);
      playIslandShape(island, "wobble");
      // launch 在自己的 380ms 摘除，不被 wobble 的启动取消。
      vi.advanceTimersByTime(50);
      expect(island.classList.contains("is-shape-launch")).toBe(false);
      expect(island.classList.contains("is-shape-wobble")).toBe(true);
      vi.advanceTimersByTime(521);
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
