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
});
