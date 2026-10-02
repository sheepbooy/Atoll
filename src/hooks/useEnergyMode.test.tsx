import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEnergyMode, ENERGY_IDLE_MS } from "./useEnergyMode";

const mocks = vi.hoisted(() => ({ hover: null as null | ((e: { hovering: boolean; cursorOverWindow: boolean }) => void), get: vi.fn(), set: vi.fn() }));
vi.mock("../tauri/settings", () => ({ getEnergyMode: mocks.get, setEnergyMode: mocks.set }));
vi.mock("../tauri/island", () => ({ onIslandOpenRequested: vi.fn(async () => () => undefined), onIslandHoverChanged: vi.fn(async cb => { mocks.hover = cb; return () => undefined; }) }));
beforeEach(() => { vi.useFakeTimers(); mocks.get.mockResolvedValue("auto"); mocks.set.mockImplementation(async value => value); });
afterEach(() => vi.useRealTimers());
describe("automatic energy saving", () => {
  it("rests after 10 seconds, without extending idle time on unrelated renders", async () => {
    const { result, rerender } = renderHook(() => useEnergyMode({ phase: "compact", busy: false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(9_000); });
    rerender();
    await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
    expect(result.current.energySaving).toBe(true);
  });
  it("wakes immediately for hover and stays awake while hovered", async () => {
    const { result } = renderHook(() => useEnergyMode({ phase: "compact", busy: false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS); });
    act(() => mocks.hover?.({ hovering: true, cursorOverWindow: true }));
    expect(result.current.energySaving).toBe(false);
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS * 2); });
    expect(result.current.energySaving).toBe(false);
    act(() => mocks.hover?.({ hovering: false, cursorOverWindow: false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS); });
    expect(result.current.energySaving).toBe(true);
  });
  it("wakes for a new request and never rests during file drag", async () => {
    const { result, rerender } = renderHook(props => useEnergyMode(props), { initialProps: { phase: "compact", busy: false, requestId: "a" } });
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS); });
    rerender({ phase: "compact", busy: false, requestId: "b" });
    expect(result.current.energySaving).toBe(false);
    rerender({ phase: "compact", busy: true, requestId: "b" });
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS * 2); });
    expect(result.current.energySaving).toBe(false);
  });
  it("supports persisted full motion", async () => {
    mocks.get.mockResolvedValue("full");
    const { result } = renderHook(() => useEnergyMode({ phase: "compact", busy: false }));
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS * 2); });
    expect(result.current.energySaving).toBe(false);
  });
  it("wakes for queued approvals even when the active request does not change", async () => {
    const { result, rerender } = renderHook(props => useEnergyMode(props), { initialProps: { phase: "compact", busy: false, requestId: "same", pendingCount: 1 } });
    await act(async () => { await vi.advanceTimersByTimeAsync(ENERGY_IDLE_MS); });
    rerender({ phase: "compact", busy: false, requestId: "same", pendingCount: 2 });
    expect(result.current.energySaving).toBe(false);
  });
});
