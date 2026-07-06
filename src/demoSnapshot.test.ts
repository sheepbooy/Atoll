import { afterEach, describe, expect, it, vi } from "vitest";
import { getDemoSnapshot } from "./demoSnapshot";

describe("demoSnapshot", () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it("generates request timestamps at snapshot time", () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-06T10:00:00.000Z"));
    const first = getDemoSnapshot("approval");

    vi.setSystemTime(new Date("2026-07-06T10:05:00.000Z"));
    const second = getDemoSnapshot("approval");

    expect(first.activeRequest?.requestedAt).toBe("2026-07-06T10:00:00.000Z");
    expect(second.activeRequest?.requestedAt).toBe("2026-07-06T10:05:00.000Z");
    expect(first.sessions[0]?.lastActivity).toBe("2026-07-06T10:00:00.000Z");
    expect(second.sessions[0]?.lastActivity).toBe("2026-07-06T10:05:00.000Z");
  });
});
