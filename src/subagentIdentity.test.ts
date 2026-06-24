import { describe, expect, it } from "vitest";
import {
  getSessionColor,
  getSubagentColor,
  getSubagentMood,
  paletteIndex,
  SESSION_PALETTE,
  stringHash,
} from "./subagentIdentity";

describe("subagentIdentity", () => {
  it("derives stable palette indices from keys", () => {
    expect(paletteIndex("session-a", SESSION_PALETTE.length)).toBe(
      paletteIndex("session-a", SESSION_PALETTE.length),
    );
    expect(paletteIndex("session-a", SESSION_PALETTE.length)).not.toBe(
      paletteIndex("session-b", SESSION_PALETTE.length),
    );
  });

  it("assigns stable color and mood per subagent id", () => {
    const agentId = "subagent-123";
    expect(getSubagentColor(agentId)).toEqual(getSubagentColor(agentId));
    expect(getSubagentMood(agentId, false)).toEqual(getSubagentMood(agentId, false));
    expect(getSubagentMood(agentId, true)).toEqual(getSubagentMood(agentId, true));
  });

  it("varies color and mood across different subagent ids", () => {
    const ids = ["sub-a", "sub-b", "sub-c", "sub-d", "sub-e", "sub-f"];
    const colors = new Set(ids.map(getSubagentColor));
    const runningMoods = new Set(ids.map((id) => getSubagentMood(id, false)));

    expect(colors.size).toBeGreaterThan(1);
    expect(runningMoods.size).toBeGreaterThan(1);
  });

  it("keeps session color hashing compatible", () => {
    expect(getSessionColor("session-xyz").tone).toBeTruthy();
    expect(stringHash("")).toBe(0);
  });
});
