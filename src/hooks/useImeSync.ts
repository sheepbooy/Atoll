// Tracks IME composition and mirrors it to the native window so the macOS
// input-method candidate window floats above the island. Extracted verbatim
// from App.tsx.
import { useEffect } from "react";
import { setImeActive } from "../tauri";
import { isImeTextTarget, isTextEntryActive } from "../imeHelpers";

export function useImeSync() {
  useEffect(() => {
    let composing = false;

    const syncIme = (active: boolean) => {
      void setImeActive(active);
    };

    const onFocusIn = (event: Event) => {
      if (isImeTextTarget(event.target)) {
        syncIme(true);
      }
    };
    const onFocusOut = (event: globalThis.FocusEvent) => {
      if (!isImeTextTarget(event.target) || composing) {
        return;
      }
      if (isImeTextTarget(event.relatedTarget)) {
        return;
      }
      syncIme(false);
    };
    const onCompositionStart = () => {
      composing = true;
      syncIme(true);
    };
    const onCompositionEnd = () => {
      composing = false;
      syncIme(isTextEntryActive());
    };

    document.addEventListener("focusin", onFocusIn);
    document.addEventListener("focusout", onFocusOut);
    document.addEventListener("compositionstart", onCompositionStart);
    document.addEventListener("compositionend", onCompositionEnd);
    return () => {
      document.removeEventListener("focusin", onFocusIn);
      document.removeEventListener("focusout", onFocusOut);
      document.removeEventListener("compositionstart", onCompositionStart);
      document.removeEventListener("compositionend", onCompositionEnd);
      syncIme(false);
    };
  }, []);
}
