import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AtollActivity } from "./AtollLogo";
import { ATOLL_ENTER_MS, ATOLL_EXIT_MS } from "./atollTransitions";
import { useAtollPhase } from "./useAtollPhase";

function PhaseProbe({ targetAct }: { targetAct: AtollActivity }) {
  const { renderAct, phase } = useAtollPhase(targetAct);
  return (
    <div
      data-testid="phase"
      data-render-act={renderAct}
      data-phase={phase}
    />
  );
}

function currentPhase() {
  const el = screen.getByTestId("phase");
  return {
    renderAct: el.dataset.renderAct,
    phase: el.dataset.phase,
  };
}

describe("useAtollPhase", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("moves an active pose from enter to loop", () => {
    render(<PhaseProbe targetAct="coffee" />);

    expect(currentPhase()).toEqual({ renderAct: "coffee", phase: "enter" });

    act(() => {
      vi.advanceTimersByTime(ATOLL_ENTER_MS);
    });

    expect(currentPhase()).toEqual({ renderAct: "coffee", phase: "loop" });
  });

  it("keeps transition timers alive when target changes during enter", () => {
    const { rerender } = render(<PhaseProbe targetAct="coffee" />);

    rerender(<PhaseProbe targetAct="idea" />);
    expect(currentPhase()).toEqual({ renderAct: "coffee", phase: "exit" });

    act(() => {
      vi.advanceTimersByTime(ATOLL_EXIT_MS);
    });
    expect(currentPhase()).toEqual({ renderAct: "idea", phase: "enter" });

    act(() => {
      vi.advanceTimersByTime(ATOLL_ENTER_MS);
    });
    expect(currentPhase()).toEqual({ renderAct: "idea", phase: "loop" });
  });
});
