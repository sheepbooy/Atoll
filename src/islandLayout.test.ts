import { describe, expect, it } from "vitest";
import { resolveCollapsedMode } from "./islandLayout";

const IDLE_PHASE = "compact" as const;

describe("resolveCollapsedMode", () => {
  it("rests dormant when idle with nothing to show", () => {
    expect(
      resolveCollapsedMode(false, false, 0, 0, IDLE_PHASE, false, false),
    ).toBe("dormant");
  });

  it("stays compact when lyrics are showing", () => {
    expect(resolveCollapsedMode(false, false, 0, 0, IDLE_PHASE, true, false)).toBe(
      "compact",
    );
  });

  it("stays compact when the battery ring is active", () => {
    expect(resolveCollapsedMode(false, false, 0, 0, IDLE_PHASE, false, true)).toBe(
      "compact",
    );
  });

  it("stays compact with active sessions regardless of the ring", () => {
    expect(resolveCollapsedMode(false, false, 2, 0, IDLE_PHASE, false, false)).toBe(
      "compact",
    );
  });
});
