import { describe, expect, it } from "vitest";

import {
  beginCollapse,
  beginExpand,
  COLLAPSE_ANIMATION_MS,
  finishCollapse,
  finishExpand,
  IDLE_COLLAPSE_DELAY_MS,
} from "./islandPresentation";

describe("island presentation timing", () => {
  it("uses the expected collapse animation duration", () => {
    expect(COLLAPSE_ANIMATION_MS).toBe(420);
  });

  it("uses the expected idle collapse delay", () => {
    expect(IDLE_COLLAPSE_DELAY_MS).toBe(500);
  });
});

describe("beginExpand", () => {
  it("starts opening from compact", () => {
    expect(beginExpand("compact")).toBe("opening");
  });

  it("does not restart while opening", () => {
    expect(beginExpand("opening")).toBe("opening");
  });

  it("does not restart while expanded", () => {
    expect(beginExpand("expanded")).toBe("expanded");
  });

  it("reverses closing back to opening", () => {
    expect(beginExpand("closing")).toBe("opening");
  });
});

describe("finishExpand", () => {
  it("finishes opening as expanded", () => {
    expect(finishExpand("opening")).toBe("expanded");
  });
});

describe("beginCollapse", () => {
  it("reverses opening into closing", () => {
    expect(beginCollapse("opening")).toBe("closing");
  });

  it("starts closing from expanded", () => {
    expect(beginCollapse("expanded")).toBe("closing");
  });

  it("does not restart while closing", () => {
    expect(beginCollapse("closing")).toBe("closing");
  });
});

describe("finishCollapse", () => {
  it("finishes closing as compact", () => {
    expect(finishCollapse("closing")).toBe("compact");
  });
});
