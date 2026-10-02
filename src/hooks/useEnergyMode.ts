import { useCallback, useEffect, useRef, useState } from "react";
import { getEnergyMode, setEnergyMode, type EnergyMode } from "../tauri/settings";
import { onIslandHoverChanged, onIslandOpenRequested } from "../tauri/island";
import { manageAsyncUnlisten } from "../asyncUnlisten";

export const ENERGY_IDLE_MS = 10_000;

export function useEnergyMode({ phase, requestId, pendingCount, busy }: {
  phase: string;
  requestId?: string;
  pendingCount?: number;
  busy: boolean;
}) {
  const [mode, setMode] = useState<EnergyMode>("auto");
  const [saving, setSaving] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>();
  const hovered = useRef(false);
  const held = useRef(busy);
  held.current = busy;
  const changedMode = useRef(false);
  const modeRef = useRef(mode);
  modeRef.current = mode;
  const wake = useCallback(() => {
    clearTimeout(timer.current);
    setSaving(false);
    if (modeRef.current === "auto" && !hovered.current && !held.current) {
      timer.current = setTimeout(() => setSaving(true), ENERGY_IDLE_MS);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getEnergyMode().then(value => { if (!cancelled && !changedMode.current) setMode(value); }).catch(() => undefined);
    const unsubscribe = manageAsyncUnlisten(onIslandHoverChanged(({ cursorOverWindow, hovering }) => {
      hovered.current = cursorOverWindow ?? hovering;
      wake();
    }));
    const unsubscribeOpen = manageAsyncUnlisten(onIslandOpenRequested(wake));
    const interact = (event: Event) => {
      if (event.target instanceof Element && event.target.closest(".island")) wake();
    };
    const enter = (event: Event) => {
      if (event.target instanceof Element && event.target.classList.contains("island")) {
        hovered.current = event.type === "pointerenter";
        wake();
      }
    };
    document.addEventListener("pointerenter", enter, true);
    document.addEventListener("pointerleave", enter, true);
    document.addEventListener("pointerdown", interact, true);
    document.addEventListener("keydown", interact, true);
    return () => {
      cancelled = true;
      clearTimeout(timer.current);
      unsubscribe();
      unsubscribeOpen();
      document.removeEventListener("pointerenter", enter, true);
      document.removeEventListener("pointerleave", enter, true);
      document.removeEventListener("pointerdown", interact, true);
      document.removeEventListener("keydown", interact, true);
    };
  }, [wake]);
  useEffect(wake, [mode, busy, phase, requestId, pendingCount, wake]);

  const changeMode = useCallback(async (next: EnergyMode) => {
    changedMode.current = true;
    const saved = await setEnergyMode(next);
    setMode(saved);
  }, []);
  return { energyMode: mode, energySaving: mode === "auto" && saving && !busy,
    changeEnergyMode: changeMode, wake };
}
