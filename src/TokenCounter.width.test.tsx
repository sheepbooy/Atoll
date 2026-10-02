import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TokenCounter } from "./TokenCounter";
import { DEFAULT_SALARY_SETTINGS } from "./salarySettings";
import { mockMotionPreference } from "./test-utils/motionPreference";

const usage = { inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0 };

describe("compact counter width notifications", () => {
  let bodyWidth: number;
  let resize: () => void;
  let observe: ReturnType<typeof vi.fn>;
  let disconnect: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 9, 2, 0, 0, 9, 900));
    mockMotionPreference(true);
    bodyWidth = 53.1;
    observe = vi.fn();
    disconnect = vi.fn();
    vi.stubGlobal("ResizeObserver", class {
      constructor(callback: () => void) { resize = callback; }
      observe = observe;
      disconnect = disconnect;
    });
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      return { width: this.classList.contains("token-counter-body") ? bodyWidth : 500 } as DOMRect;
    });
  });
  afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

  it.each(["¥", "$", "€"] as const)("reports only body width changes as %s earnings tick", (currency) => {
    const onWidth = vi.fn();
    const salary = { ...DEFAULT_SALARY_SETTINGS, currency, monthlyWage: 3600, workDaysPerMonth: 1, workHoursPerDay: 1 };
    const { container, unmount } = render(<TokenCounter value={0} usage={usage}
      displayMode="salary" salary={salary} compactTokenLevel={0} onCompactWidthChange={onWidth} />);
    const body = container.querySelector(".token-counter-body")!;
    expect(observe).toHaveBeenCalledWith(body);
    expect(onWidth).toHaveBeenCalledExactlyOnceWith(54, 0);
    expect(body.textContent).toBe(`${currency}9.90`);

    act(() => vi.advanceTimersByTime(250));
    expect(body.textContent).toBe(`${currency}10.15`);
    expect(onWidth).toHaveBeenCalledTimes(1);
    bodyWidth = 61.2;
    act(() => resize());
    expect(onWidth).toHaveBeenLastCalledWith(62, 0);
    bodyWidth = 61.4;
    act(() => { resize(); vi.advanceTimersByTime(1000); });
    expect(onWidth).toHaveBeenCalledTimes(2);

    vi.setSystemTime(new Date(2026, 9, 3, 0, 0, 0));
    act(() => vi.advanceTimersByTime(250));
    expect(body.textContent).toBe(`${currency}0.25`);
    bodyWidth = 53.1;
    act(() => resize());
    expect(onWidth).toHaveBeenLastCalledWith(54, 0);
    unmount();
    expect(disconnect).toHaveBeenCalledOnce();
  });

  it("suspends measurement during transitions and resumes on settlement", () => {
    const onWidth = vi.fn();
    const { rerender } = render(<TokenCounter value={0} usage={usage} suppressAnimations onCompactWidthChange={onWidth} />);
    expect(onWidth).not.toHaveBeenCalled();
    rerender(<TokenCounter value={0} usage={usage} onCompactWidthChange={onWidth} />);
    expect(onWidth).toHaveBeenCalledExactlyOnceWith(54, 0);
    rerender(<TokenCounter value={0} usage={usage} suppressAnimations onCompactWidthChange={onWidth} />);
    expect(disconnect).toHaveBeenCalledOnce();
  });

  it("waits for new text before measuring a new display mode or compression tier", () => {
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      return { width: (this.textContent ?? "").length * 10 } as DOMRect;
    });
    const onWidth = vi.fn();
    const { rerender } = render(<TokenCounter value={1234.56} usage={usage} displayMode="salary"
      salary={DEFAULT_SALARY_SETTINGS} compactTokenLevel={0} onCompactWidthChange={onWidth} />);
    onWidth.mockClear();
    rerender(<TokenCounter value={1234.56} usage={usage} displayMode="cost"
      compactTokenLevel={2} onCompactWidthChange={onWidth} />);
    expect(onWidth).toHaveBeenCalledExactlyOnceWith(30, 2);
    onWidth.mockClear();
    rerender(<TokenCounter value={1234.56} usage={usage} displayMode="cost"
      compactTokenLevel={1} onCompactWidthChange={onWidth} />);
    expect(onWidth).toHaveBeenCalledExactlyOnceWith(60, 1);
  });

  it.each(["expanded", "micro"] as const)("does not change %s geometry", (variant) => {
    const onWidth = vi.fn();
    render(<TokenCounter value={0} usage={usage} variant={variant} onCompactWidthChange={onWidth} />);
    expect(onWidth).not.toHaveBeenCalled();
    expect(observe).not.toHaveBeenCalled();
  });
});
