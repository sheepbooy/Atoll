import { describe, expect, it } from "vitest";
import { deriveAtollReaction } from "./useAtollReaction";

describe("deriveAtollReaction", () => {
  it("collapses when going offline", () => {
    expect(deriveAtollReaction("idle", "offline")).toBe("collapse");
    expect(deriveAtollReaction("working", "offline")).toBe("collapse");
    expect(deriveAtollReaction("pending", "offline")).toBe("collapse");
  });

  it("revives when coming back online", () => {
    expect(deriveAtollReaction("offline", "idle")).toBe("revive");
    expect(deriveAtollReaction("offline", "working")).toBe("revive");
  });

  it("cheers when work clears to idle", () => {
    expect(deriveAtollReaction("pending", "idle")).toBe("cheer");
    expect(deriveAtollReaction("working", "idle")).toBe("cheer");
  });

  it("stays quiet on non-event transitions", () => {
    expect(deriveAtollReaction("idle", "working")).toBeNull();
    expect(deriveAtollReaction("idle", "pending")).toBeNull();
    expect(deriveAtollReaction("pending", "working")).toBeNull();
    expect(deriveAtollReaction("idle", "idle")).toBeNull();
    expect(deriveAtollReaction("offline", "offline")).toBeNull();
  });
});
