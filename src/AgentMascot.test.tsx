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

  it("renders the official Cursor 2.5D cube mark", () => {
    const cursor = render(<AgentMascot agent="cursor" mood="calm" animated={false} />);
    expect(cursor.container.querySelector(".cursor-mascot-mark")).not.toBeNull();
    expect(cursor.container.querySelector(".cursor-mascot-cube")).not.toBeNull();
    expect(cursor.container.querySelectorAll(".cursor-mascot-outline path")).toHaveLength(5);
    expect(cursor.container.querySelectorAll(".cursor-mascot-mark path")).toHaveLength(10);
    expect(
      cursor.container.querySelector(".cursor-mascot-outline path")?.getAttribute("vector-effect"),
    ).toBe("non-scaling-stroke");
    expect(cursor.container.querySelector(".cursor-mascot-pointer-head")).not.toBeNull();
    expect(cursor.container.querySelector(".cursor-mascot-claw")).toBeNull();
    expect(cursor.container.querySelector(".cursor-mascot-leg")).toBeNull();
  });

  it("renders the official Codex blossom-and-prompt mark", () => {
    const codex = render(<AgentMascot agent="codex" mood="calm" animated={false} />);
    expect(codex.container.querySelector(".codex-mark")).not.toBeNull();
    expect(codex.container.querySelectorAll(".codex-mark path")).toHaveLength(1);
    expect(codex.container.querySelector(".codex-mark path")?.getAttribute("fill-rule")).toBe("evenodd");
    expect(codex.container.querySelector(".codex-chassis")).toBeNull();
    expect(codex.container.querySelector(".codex-screen")).toBeNull();
    expect(codex.container.querySelector(".codex-leg")).toBeNull();
    expect(codex.container.querySelector(".codex-mark path")?.getAttribute("fill")).toBe("#f4f4f4");
  });

  it("renders the official Gemini spark instead of a reused Clawd", () => {
    const gemini = render(<AgentMascot agent="gemini" mood="calm" animated={false} />);
    expect(gemini.container.querySelector(".gemini-mark path")).not.toBeNull();
    // The spark carries its own brand gradient — no Clawd body parts.
    expect(gemini.container.querySelector(".gemini-mark path")?.getAttribute("fill")?.startsWith("url(#gemini-spark-")).toBe(true);
    expect(gemini.container.querySelector(".clawd")).toBeNull();
    expect(gemini.container.querySelector(".gemini-leg")).toBeNull();

    const sleeping = render(<AgentMascot agent="gemini" mood="sleeping" animated={false} />);
    const sleepingFill = sleeping.container
      .querySelector(".gemini-mark path")
      ?.getAttribute("fill");
    expect(sleepingFill).not.toBe(gemini.container.querySelector(".gemini-mark path")?.getAttribute("fill"));
  });

  it("recolors the Gemini spark with the session accent", () => {
    const tinted = render(
      <AgentMascot agent="gemini" mood="calm" accent="#ff8175" accentDark="#c05a54" animated={false} />,
    );
    const gradient = tinted.container.querySelector(".gemini-mark linearGradient");
    expect(gradient?.querySelectorAll("stop")).toHaveLength(2);
    expect(gradient?.querySelector("stop")?.getAttribute("stop-color")).toBe("#ff8175");
    expect(gradient?.querySelectorAll("stop")[1]?.getAttribute("stop-color")).toBe("#c05a54");

    const dead = render(
      <AgentMascot agent="gemini" mood="dead" accent="#ff8175" animated={false} />,
    );
    const deadStops = dead.container.querySelectorAll(".gemini-mark linearGradient stop");
    expect(deadStops[0]?.getAttribute("stop-color")).toBe("#8a8a8a");
  });

  it("marks gemini mascots static when animation is disabled", () => {
    vi.useFakeTimers();
    const setTimeoutSpy = vi.spyOn(window, "setTimeout");
    const { container } = render(
      <AgentMascot agent="gemini" mood="alert" animated={false} />,
    );

    expect(setTimeoutSpy).not.toHaveBeenCalled();
    expect(container.querySelector(".gemini.is-static")).not.toBeNull();
  });

  it("recolors official marks for offline and error moods", () => {
    const deadCodex = render(<AgentMascot agent="codex" mood="dead" animated={false} />);
    expect(deadCodex.container.querySelector(".codex-mark path")?.getAttribute("fill")).toBe("#8a8a8a");

    const sickCursor = render(<AgentMascot agent="cursor" mood="worried" animated={false} />);
    expect(sickCursor.container.querySelector(".cursor-mascot-face-bottom")?.getAttribute("fill")).toBe("#6d8a6d");
  });

  it("paints a session-colored outline on Cursor cubes", () => {
    const cursor = render(
      <AgentMascot agent="cursor" mood="calm" accent="#ff8175" animated={false} />,
    );
    const cursorEl = cursor.container.querySelector(".cursor-mascot") as HTMLElement;
    expect(cursorEl.style.getPropertyValue("--cursor-outline")).toBe("#ff8175");
  });

  it("recolors Codex blossoms with the session accent", () => {
    const codex = render(
      <AgentMascot agent="codex" mood="calm" accent="#ff8175" animated={false} />,
    );
    expect(codex.container.querySelector(".codex-mark path")?.getAttribute("fill")).toBe(
      "#ff8175",
    );

    const sleeping = render(
      <AgentMascot agent="codex" mood="sleeping" accent="#80b0f8" animated={false} />,
    );
    expect(sleeping.container.querySelector(".codex-mark path")?.getAttribute("fill")).toBe(
      "#80b0f8",
    );

    const dead = render(
      <AgentMascot agent="codex" mood="dead" accent="#ff8175" animated={false} />,
    );
    expect(dead.container.querySelector(".codex-mark path")?.getAttribute("fill")).toBe(
      "#8a8a8a",
    );
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
