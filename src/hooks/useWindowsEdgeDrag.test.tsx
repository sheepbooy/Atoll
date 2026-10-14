import { act, render, fireEvent } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useWindowsEdgeDrag } from "./useWindowsEdgeDrag";

const bridge = vi.hoisted(() => ({
  getWindowsEdgePosition: vi.fn(), moveWindowsEdgeIsland: vi.fn(),
}));
vi.mock("../tauri", () => bridge);

function Harness({ enabled = true }: { enabled?: boolean }) {
  const handlers = useWindowsEdgeDrag(enabled);
  return <header {...handlers} onClick={() => clicked()} data-testid="handle">Drag<button data-no-drag>Action</button></header>;
}
const clicked = vi.fn();
const origin = { ratio: 0.9, dragRatio: 0.4, cursorY: 1500 };
class TestPointerEvent extends MouseEvent {
  pointerId: number;
  constructor(type: string, init: PointerEventInit = {}) {
    super(type, init); this.pointerId = init.pointerId ?? 1;
  }
}
function pointer(element: HTMLElement, type: string, y: number, x = 0) {
  fireEvent(element, new TestPointerEvent(type, { bubbles: true, cancelable: true, button: 0, pointerId: 1, screenY: y, screenX: x }));
}
describe("Windows edge drag", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("PointerEvent", TestPointerEvent);
    bridge.getWindowsEdgePosition.mockResolvedValue(origin);
    bridge.moveWindowsEdgeIsland.mockResolvedValue(0.5);
  });
  afterEach(() => vi.unstubAllGlobals());

  it("keeps a click and movement up to six pixels as a click", async () => {
    const { getByTestId } = render(<Harness />); const handle = getByTestId("handle");
    pointer(handle, "pointerdown", 100);
    pointer(handle, "pointermove", 106, 300);
    pointer(handle, "pointerup", 106);
    fireEvent.click(handle);
    await act(async () => {});
    expect(clicked).toHaveBeenCalledOnce();
    expect(bridge.moveWindowsEdgeIsland).not.toHaveBeenCalled();
  });

  it("moves vertically and persists at release without opening the panel", async () => {
    const { getByTestId } = render(<Harness />); const handle = getByTestId("handle");
    pointer(handle, "pointerdown", 100);
    await act(async () => { pointer(handle, "pointermove", 107); });
    expect(bridge.moveWindowsEdgeIsland).toHaveBeenCalledWith(origin, false);
    await act(async () => { pointer(handle, "pointerup", 107); });
    fireEvent.click(handle);
    expect(bridge.moveWindowsEdgeIsland).toHaveBeenLastCalledWith(origin, true);
    expect(clicked).not.toHaveBeenCalled();
  });

  it("serializes a delayed position read and move before the final save", async () => {
    let resolveOrigin!: (value: typeof origin) => void;
    bridge.getWindowsEdgePosition.mockReturnValue(new Promise(resolve => { resolveOrigin = resolve; }));
    const { getByTestId } = render(<Harness />); const handle = getByTestId("handle");
    pointer(handle, "pointerdown", 100);
    pointer(handle, "pointermove", 120);
    pointer(handle, "pointerup", 120);
    fireEvent.click(handle);
    await act(async () => { resolveOrigin(origin); });
    expect(bridge.moveWindowsEdgeIsland).toHaveBeenCalledOnce();
    expect(bridge.moveWindowsEdgeIsland).toHaveBeenCalledWith(origin, true);
    expect(clicked).not.toHaveBeenCalled();
  });

  it("saves on lost pointer capture and ignores action buttons", async () => {
    const { getByTestId, getByRole } = render(<Harness />); const handle = getByTestId("handle");
    pointer(getByRole("button"), "pointerdown", 100);
    expect(bridge.getWindowsEdgePosition).not.toHaveBeenCalled();
    pointer(handle, "pointerdown", 100);
    await act(async () => { pointer(handle, "pointermove", 120); });
    await act(async () => { pointer(handle, "lostpointercapture", 120); });
    expect(bridge.moveWindowsEdgeIsland).toHaveBeenLastCalledWith(origin, true);
  });

  it("does not move when disabled or when the backend read fails", async () => {
    const { getByTestId, rerender } = render(<Harness enabled={false} />); const handle = getByTestId("handle");
    pointer(handle, "pointerdown", 100); pointer(handle, "pointermove", 120);
    expect(bridge.getWindowsEdgePosition).not.toHaveBeenCalled();
    rerender(<Harness />);
    bridge.getWindowsEdgePosition.mockRejectedValue(new Error("unavailable"));
    await act(async () => {
      pointer(handle, "pointerdown", 100); pointer(handle, "pointermove", 120); pointer(handle, "pointerup", 120);
    });
    expect(bridge.moveWindowsEdgeIsland).not.toHaveBeenCalled();
  });
});
