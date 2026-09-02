import type { GlobalShortcutConfig, ShortcutAction } from "./tauri";

/**
 * Pure helpers for global-shortcut accelerators. Mirrors
 * src-tauri/src/shortcuts.rs: same accepted aliases, same canonical stored
 * form `[Cmd|Ctrl][+Alt][+Shift]+Key`, same validation rules. The backend is
 * the authority — this file exists for instant UI feedback and display
 * formatting.
 */

export type ShortcutPlatform = "macos" | "other";

export const SHORTCUT_ACTIONS: ShortcutAction[] = ["summon", "approve", "deny", "always"];

/**
 * Defaults match the Rust-side defaults before platform normalization:
 * "CmdOrCtrl" resolves to Cmd on macOS and Ctrl elsewhere. Chosen to avoid
 * common OS shortcuts (Cmd+Space/Spotlight, Alt+Space window menus,
 * Cmd+Tab, Ctrl+Shift+Esc).
 */
export const DEFAULT_GLOBAL_SHORTCUTS: GlobalShortcutConfig = {
  enabled: true,
  summon: "CmdOrCtrl+Shift+Space",
  approve: "CmdOrCtrl+Shift+Y",
  deny: "CmdOrCtrl+Shift+N",
  always: "CmdOrCtrl+Shift+A",
};

export function shortcutPlatform(platform: string = navigator.platform): ShortcutPlatform {
  const value = platform.toLowerCase();
  return value.includes("mac") || value.includes("iphone") || value.includes("ipad")
    ? "macos"
    : "other";
}

export type AcceleratorParse = { ok: true; value: string } | { ok: false; error: string };

const PRIMARY_TOKENS: Record<string, "Cmd"> = {
  CMD: "Cmd",
  COMMAND: "Cmd",
  META: "Cmd",
  SUPER: "Cmd",
  WINDOWS: "Cmd",
  WIN: "Cmd",
};

const PUNCTUATION_KEYS = new Set(["-", "=", "[", "]", ",", ".", "/", ";", "'", "`", "\\"]);

const NAMED_KEYS: Record<string, string> = {
  SPACE: "Space",
  ENTER: "Enter",
  TAB: "Tab",
  ESCAPE: "Escape",
  ESC: "Escape",
  UP: "Up",
  ARROWUP: "Up",
  DOWN: "Down",
  ARROWDOWN: "Down",
  LEFT: "Left",
  ARROWLEFT: "Left",
  RIGHT: "Right",
  ARROWRIGHT: "Right",
};

const FUNCTION_KEY = /^F([1-9]|1[0-9]|2[0-4])$/;

function isFunctionKey(key: string): boolean {
  return FUNCTION_KEY.test(key);
}

function normalizeKey(upper: string, input: string): string {
  const invalid = () => {
    throw new Error(`accelerator "${input}" uses unsupported key "${upper}"`);
  };
  if (upper.length === 1) {
    const byte = upper.charCodeAt(0);
    if ((byte >= 65 && byte <= 90) || (byte >= 48 && byte <= 57)) {
      return upper;
    }
    if (PUNCTUATION_KEYS.has(upper)) {
      return upper;
    }
    return invalid();
  }
  const keyLetter = /^KEY([A-Z])$/.exec(upper);
  if (keyLetter) {
    return keyLetter[1];
  }
  const digit = /^DIGIT([0-9])$/.exec(upper);
  if (digit) {
    return digit[1];
  }
  const fnMatch = /^F(\d{1,2})$/.exec(upper);
  if (fnMatch) {
    const number = Number(fnMatch[1]);
    if (number >= 1 && number <= 24) {
      return `F${number}`;
    }
    return invalid();
  }
  const named = NAMED_KEYS[upper];
  if (named) {
    return named;
  }
  return invalid();
}

function normalize(input: string, platform: ShortcutPlatform): string {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error("accelerator is empty");
  }
  let primary: "Cmd" | "Ctrl" | null = null;
  let alt = false;
  let shift = false;
  let key: string | null = null;
  for (const rawToken of trimmed.split("+")) {
    const token = rawToken.trim();
    if (!token) {
      throw new Error(`accelerator "${input}" has an empty part`);
    }
    const upper = token.toUpperCase();
    if (upper in PRIMARY_TOKENS) {
      if (primary && primary !== "Cmd") {
        throw new Error(`accelerator "${input}" combines Cmd and Ctrl`);
      }
      primary = "Cmd";
      continue;
    }
    if (upper === "CTRL" || upper === "CONTROL") {
      if (primary && primary !== "Ctrl") {
        throw new Error(`accelerator "${input}" combines Cmd and Ctrl`);
      }
      primary = "Ctrl";
      continue;
    }
    if (
      upper === "CMDORCTRL" ||
      upper === "COMMANDORCONTROL" ||
      upper === "COMMANDORCTRL" ||
      upper === "CMDORCONTROL"
    ) {
      const resolved = platform === "macos" ? "Cmd" : "Ctrl";
      if (primary && primary !== resolved) {
        throw new Error(`accelerator "${input}" combines Cmd and Ctrl`);
      }
      primary = resolved;
      continue;
    }
    if (upper === "ALT" || upper === "OPTION") {
      alt = true;
      continue;
    }
    if (upper === "SHIFT") {
      shift = true;
      continue;
    }
    if (key) {
      throw new Error(`accelerator "${input}" has more than one key`);
    }
    key = normalizeKey(upper, input);
  }
  if (!key) {
    throw new Error(`accelerator "${input}" is missing a key`);
  }
  const parts: string[] = [];
  if (primary) {
    parts.push(primary);
  }
  if (alt) {
    parts.push("Alt");
  }
  if (shift) {
    parts.push("Shift");
  }
  if (parts.length === 0 && !isFunctionKey(key)) {
    throw new Error(
      `accelerator "${input}" needs at least one modifier (bare F1-F24 are the exception)`,
    );
  }
  parts.push(key);
  return parts.join("+");
}

export function normalizeAccelerator(
  input: string,
  platform: ShortcutPlatform,
): AcceleratorParse {
  try {
    return { ok: true, value: normalize(input, platform) };
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : String(error) };
  }
}

/** Storage form → display form ("CmdOrCtrl" → Cmd/Ctrl, "Cmd" → Win elsewhere). */
export function formatAccelerator(value: string, platform: ShortcutPlatform): string {
  const trimmed = value.trim();
  if (!trimmed) {
    return "";
  }
  const macos = platform === "macos";
  return trimmed
    .split("+")
    .map((token) => {
      const upper = token.trim().toUpperCase();
      if (
        upper === "CMDORCTRL" ||
        upper === "COMMANDORCONTROL" ||
        upper === "COMMANDORCTRL" ||
        upper === "CMDORCONTROL"
      ) {
        return macos ? "Cmd" : "Ctrl";
      }
      if (upper === "CMD" || upper === "COMMAND" || upper === "META" || upper === "SUPER") {
        return macos ? "Cmd" : "Win";
      }
      return token.trim();
    })
    .join("+");
}

/** Physical keys of a keyboard event that map to accelerator keys. */
const NAMED_KEY_CODES: Record<string, string> = {
  Space: "Space",
  Enter: "Enter",
  Tab: "Tab",
  Escape: "Escape",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Semicolon: ";",
  Quote: "'",
  Backquote: "`",
  Backslash: "\\",
};

const MODIFIER_CODES = new Set([
  "MetaLeft",
  "MetaRight",
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
]);

function keyCode(code: string): string | null {
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) {
    return letter[1];
  }
  const digit = /^Digit([0-9])$/.exec(code);
  if (digit) {
    return digit[1];
  }
  if (FUNCTION_KEY.test(code)) {
    return code;
  }
  return NAMED_KEY_CODES[code] ?? null;
}

function singleCharKey(key: string): string | null {
  if (/^[a-z0-9]$/.test(key)) {
    return key.toUpperCase();
  }
  if (key.length === 1 && PUNCTUATION_KEYS.has(key)) {
    return key;
  }
  return null;
}

export interface ShortcutKeyEvent {
  code: string;
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
}

/**
 * Map a keyboard event to a canonical accelerator string, or null when the
 * event cannot form one (modifier alone, no modifier with a non-F key, or an
 * unsupported key). The recorder ignores the Windows key (Meta on non-macOS)
 * and refuses Cmd+Ctrl combos, which the backend rejects anyway.
 */
export function acceleratorFromKeyboardEvent(
  event: ShortcutKeyEvent,
  platform: ShortcutPlatform,
): string | null {
  if (MODIFIER_CODES.has(event.code)) {
    return null;
  }
  const key = keyCode(event.code) ?? singleCharKey(event.key);
  if (!key) {
    return null;
  }
  const macos = platform === "macos";
  if (macos && event.metaKey && event.ctrlKey) {
    return null;
  }
  const cmd = macos && event.metaKey;
  const ctrl = event.ctrlKey && !cmd;
  const alt = event.altKey;
  const shift = event.shiftKey;
  if (!cmd && !ctrl && !alt && !shift) {
    return isFunctionKey(key) ? key : null;
  }
  const parts = [cmd ? "Cmd" : ctrl ? "Ctrl" : null, alt ? "Alt" : null, shift ? "Shift" : null];
  return parts.filter(Boolean).concat(key).join("+");
}

export function withShortcutAction(
  config: GlobalShortcutConfig,
  action: ShortcutAction,
  value: string,
): GlobalShortcutConfig {
  return { ...config, [action]: value };
}
