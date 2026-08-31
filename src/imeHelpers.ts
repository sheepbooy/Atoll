export const NON_TEXT_INPUT_TYPES = new Set([
  "button",
  "checkbox",
  "radio",
  "range",
  "file",
  "submit",
  "reset",
  "hidden",
  "color",
  "image",
]);

export function isImeTextTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.tagName === "TEXTAREA" || target.isContentEditable) return true;
  if (target.tagName !== "INPUT") return false;
  return !NON_TEXT_INPUT_TYPES.has((target as HTMLInputElement).type);
}

// True only while a real text field is focused (the reply input). Used to keep
// the island open while typing, without letting a stray button focus pin it.
export function isTextEntryActive() {
  if (typeof document === "undefined") return false;
  return isImeTextTarget(document.activeElement);
}
