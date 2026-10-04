import { render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { CompactSessionStack } from "./CompactSessionStack";
// @ts-expect-error The project does not install @types/node; Vitest runs in Node.
import { readFileSync } from "node:fs";
import type { SessionSummary } from "../tauri";

const islandCss = readFileSync("./src/styles/island.css", "utf8");
const sessions: SessionSummary[] = ["one", "two"].map((sessionId) => ({
  sessionId,
  agent: "codex",
  cwd: `/tmp/${sessionId}`,
  pendingCount: 0,
  totalCount: 0,
  lastActivity: "2026-10-05T00:00:00Z",
  transcriptPath: null,
}));

function Stack({ saving, visible = true, placement = "left" }: {
  saving: boolean;
  visible?: boolean;
  placement?: "left" | "right";
}) {
  return (
    <div className={`island${saving ? " is-energy-saving" : ""}`}>
      {visible && <CompactSessionStack sessions={sessions} placement={placement}
        animated={!saving} activeRequest={null} justResolved={false} />}
    </div>
  );
}

describe("compact session visibility during energy saving", () => {
  let stylesheet: HTMLStyleElement;
  beforeEach(() => {
    stylesheet = document.createElement("style");
    stylesheet.textContent = islandCss;
    document.head.append(stylesheet);
  });
  afterEach(() => stylesheet?.remove());

  it.each(["left", "right"] as const)("shows newly mounted %s icons without a paused transparent entrance", (placement) => {
    const { container, rerender } = render(<Stack saving visible={false} placement={placement} />);
    rerender(<Stack saving placement={placement} />);
    const dots = container.querySelectorAll(".compact-session-dot");
    expect(dots).toHaveLength(2);
    for (const dot of dots) {
      // Removing the entrance also removes its opacity:0 backwards fill,
      // including the staggered delay on the second icon.
      expect(getComputedStyle(dot).animation).toBe("none");
      expect(getComputedStyle(dot.querySelector(".codex")!).animationPlayState).toBe("paused");
    }
  });

  it("lands icons when saving begins during an entrance and keeps them visible after remounting", () => {
    const { container, rerender } = render(<Stack saving={false} />);
    expect(getComputedStyle(container.querySelector(".compact-session-dot")!).animation).toContain("session-dot-in");
    rerender(<Stack saving />);
    expect(getComputedStyle(container.querySelector(".compact-session-dot")!).animation).toBe("none");
    // Lifecycle theater temporarily replaces the stack, then restores it.
    rerender(<Stack saving visible={false} />);
    rerender(<Stack saving />);
    expect(getComputedStyle(container.querySelector(".compact-session-dot")!).animation).toBe("none");
  });

  it("preserves finite theater feedback while mascot loops stay paused", () => {
    const { container } = render(
      <div className="island is-energy-saving">
        <span className="agent-theater">
          <span className="compact-session-dot agent-theater-actor is-started" />
        </span>
      </div>,
    );
    const actor = container.querySelector(".agent-theater-actor")!;
    expect(getComputedStyle(actor).animation).toContain("theater-type");
    expect(getComputedStyle(actor).animationPlayState).toBe("running");
  });
});
