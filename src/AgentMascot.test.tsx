import { render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AgentMascot } from "./AgentMascot";

describe("AgentMascot", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("keeps blink animation timers enabled by default", () => {
    vi.useFakeTimers();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");

    render(<AgentMascot agent="claude" mood="alert" />);

    expect(setTimeoutSpy).toHaveBeenCalled();
  });

  it("does not register blink timers when animation is disabled", () => {
    vi.useFakeTimers();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");

    render(<AgentMascot agent="claude" mood="alert" animated={false} />);

    expect(setTimeoutSpy).not.toHaveBeenCalled();
  });

  it("marks cursor mascots static when animation is disabled", () => {
    vi.useFakeTimers();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");
    const { container } = render(
      <AgentMascot agent="cursor" mood="alert" animated={false} />,
    );

    expect(setTimeoutSpy).not.toHaveBeenCalled();
    expect(container.querySelector(".cursor-mascot.is-static")).not.toBeNull();
  });

  it("marks codex mascots static when animation is disabled", () => {
    vi.useFakeTimers();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");
    const { container } = render(
      <AgentMascot agent="codex" mood="alert" animated={false} />,
    );

    expect(setTimeoutSpy).not.toHaveBeenCalled();
    expect(container.querySelector(".codex.is-static")).not.toBeNull();
  });

  it("gives cursor a cube-and-pointer silhouette, not Clawd's wide body", () => {
    const cursor = render(<AgentMascot agent="cursor" mood="calm" animated={false} />);
    expect(cursor.container.querySelectorAll(".cursor-mascot-claw")).toHaveLength(2);
    expect(cursor.container.querySelectorAll(".cursor-mascot-leg")).toHaveLength(2);
    expect(cursor.container.querySelector(".cursor-mascot-pointer")).not.toBeNull();
    expect(cursor.container.querySelector(".cursor-mascot-face-front")?.getAttribute("width")).toBe("56");
    expect(cursor.container.querySelector(".cursor-mascot-face-side")).not.toBeNull();
  });

  it("gives codex a simple screen-and-legs silhouette", () => {
    const codex = render(<AgentMascot agent="codex" mood="calm" animated={false} />);
    expect(codex.container.querySelectorAll(".codex-claw")).toHaveLength(0);
    expect(codex.container.querySelectorAll(".codex-leg")).toHaveLength(2);
    expect(codex.container.querySelector(".codex-chassis")?.getAttribute("width")).toBe("64");
    expect(codex.container.querySelector(".codex-chassis")?.getAttribute("height")).toBe("48");
    expect(codex.container.querySelector(".codex-screen")).not.toBeNull();
    expect(codex.container.querySelector(".codex-prompt")).not.toBeNull();
    expect(codex.container.querySelector(".codex-cursor")).not.toBeNull();
    expect(codex.container.querySelector(".codex-cursor")).not.toBeNull();
    expect(codex.container.querySelector(".codex-chin")).toBeNull();
    expect(codex.container.querySelector(".codex-stand")).toBeNull();
    expect(codex.container.querySelector(".codex-keyboard")).toBeNull();
  });

  it("does not attach glow CSS variables", () => {
    const cursor = render(<AgentMascot agent="cursor" mood="calm" animated={false} />);
    const cursorEl = cursor.container.querySelector(".cursor-mascot") as HTMLElement;
    expect(cursorEl.style.getPropertyValue("--cursor-mascot-glow-inner")).toBe("");
    expect(cursorEl.style.getPropertyValue("--cursor-mascot-glow-outer")).toBe("");

    const codex = render(<AgentMascot agent="codex" mood="calm" animated={false} />);
    const codexEl = codex.container.querySelector(".codex") as HTMLElement;
    expect(codexEl.style.getPropertyValue("--codex-glow-inner")).toBe("");
    expect(codexEl.style.getPropertyValue("--codex-glow-outer")).toBe("");
  });
});
