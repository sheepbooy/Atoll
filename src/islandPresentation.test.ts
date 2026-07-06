import { describe, expect, it } from "vitest";

import {
  beginCollapse,
  beginExpand,
  finishCollapse,
  finishExpand,
} from "./islandPresentation";

describe("beginExpand", () => {
  it("starts opening from compact", () => {
    expect(beginExpand("compact")).toBe("opening");
  });

  it("starts opening from micro", () => {
    expect(beginExpand("micro")).toBe("opening");
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

  it("ignores a late opening completion after reversing to closing", () => {
    expect(finishExpand("closing")).toBe("closing");
  });
});

describe("beginCollapse", () => {
  it("does not start closing from compact", () => {
    expect(beginCollapse("compact")).toBe("compact");
  });

  it("does not start closing from micro", () => {
    expect(beginCollapse("micro")).toBe("micro");
  });

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

  it("finishes closing as micro when requested", () => {
    expect(finishCollapse("closing", true)).toBe("micro");
  });

  it("ignores a late closing completion after reopening", () => {
    expect(finishCollapse("opening")).toBe("opening");
  });
});
