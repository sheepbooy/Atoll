import { useEffect, useRef, type PointerEvent, type MouseEvent } from "react";
import { getWindowsEdgePosition, moveWindowsEdgeIsland } from "../tauri";

const DRAG_THRESHOLD = 6;

/** Serialize native moves; intermediate pointer events coalesce into one update.
 * The native side samples physical cursor coordinates for mixed-DPI displays. */
export function useWindowsEdgeDrag(enabled: boolean, onDraggingChange?: (active: boolean) => void) {
  const onDraggingChangeRef = useRef(onDraggingChange);
  onDraggingChangeRef.current = onDraggingChange;
  const gestureRef = useRef<{
    pointerId: number;
    startY: number;
    dragged: boolean;
    finished: boolean;
    pending: boolean;
    origin: ReturnType<typeof getWindowsEdgePosition>;
    worker: Promise<void> | null;
  } | null>(null);
  const suppressClickRef = useRef(false);

  useEffect(() => () => {
    const gesture = gestureRef.current;
    if (gesture?.dragged) { gesture.finished = true; gesture.pending = true; pump(gesture); }
    if (gesture) onDraggingChangeRef.current?.(false);
  }, []);

  function pump(gesture: NonNullable<typeof gestureRef.current>) {
    if (gesture.worker) return;
    gesture.worker = (async () => {
      const origin = await gesture.origin;
      while (gesture.pending) {
        gesture.pending = false;
        const finished = gesture.finished;
        await moveWindowsEdgeIsland(origin, finished);
        if (finished) break;
      }
    })().catch(() => {
      gesture.pending = false;
      gesture.finished = true;
    }).finally(() => {
      gesture.worker = null;
      if (gesture.pending) pump(gesture);
    });
  }

  function finish(event: PointerEvent<HTMLElement>) {
    const gesture = gestureRef.current;
    if (!gesture || gesture.pointerId !== event.pointerId) return;
    gesture.finished = true;
    if (gesture.dragged) {
      gesture.pending = true;
      suppressClickRef.current = true;
      pump(gesture);
    }
    gestureRef.current = null;
    onDraggingChangeRef.current?.(false);
    if (event.currentTarget.hasPointerCapture?.(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  }

  return {
    onPointerDown(event: PointerEvent<HTMLElement>) {
      if (!enabled || event.button !== 0 || gestureRef.current) return;
      suppressClickRef.current = false;
      if ((event.target as HTMLElement).closest("[data-no-drag], button, input, textarea, [contenteditable='true']")) return;
      // Avoid focus-triggered expansion before we can distinguish drag from click.
      event.preventDefault();
      event.currentTarget.setPointerCapture?.(event.pointerId);
      const origin = getWindowsEdgePosition();
      // Handle a rejected read immediately, even if this turns out to be a click.
      void origin.catch(() => undefined);
      onDraggingChangeRef.current?.(true);
      gestureRef.current = { pointerId: event.pointerId, startY: event.screenY,
        dragged: false, finished: false, pending: false, origin, worker: null };
    },
    onPointerMove(event: PointerEvent<HTMLElement>) {
      const gesture = gestureRef.current;
      if (!gesture || gesture.pointerId !== event.pointerId || gesture.finished) return;
      if (Math.abs(event.screenY - gesture.startY) > DRAG_THRESHOLD) gesture.dragged = true;
      if (!gesture.dragged) return;
      gesture.pending = true;
      pump(gesture);
    },
    onPointerUp: finish,
    onPointerCancel: finish,
    onLostPointerCapture: finish,
    onClickCapture(event: MouseEvent<HTMLElement>) {
      if (!suppressClickRef.current) return;
      suppressClickRef.current = false;
      event.preventDefault();
      event.stopPropagation();
    },
  };
}
