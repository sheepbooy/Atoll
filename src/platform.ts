export const IS_WINDOWS =
  typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
export const IS_MACOS =
  typeof navigator !== "undefined" && /Mac/i.test(navigator.userAgent);

export const DECISION_SHORTCUTS = {
  deny: IS_WINDOWS ? "Del" : "⌫",
  approve: IS_WINDOWS ? "Enter" : "↵",
  always: IS_WINDOWS ? "Shift+Enter" : "⇧↵",
} as const;
