import { describe, expect, it } from "vitest";
import {
  DEFAULT_GLOBAL_SHORTCUTS,
  acceleratorFromKeyboardEvent,
  formatAccelerator,
  normalizeAccelerator,
  shortcutPlatform,
  withShortcutAction,
} from "./shortcuts";

describe("normalizeAccelerator", () => {
  it("canonicalizes aliases, case, and whitespace", () => {
    expect(normalizeAccelerator(" cmd + shift + y ", "macos")).toEqual({
      ok: true,
      value: "Cmd+Shift+Y",
    });
    expect(normalizeAccelerator("SHIFT+ALT+SPACE", "other")).toEqual({
      ok: true,
      value: "Alt+Shift+Space",
    });
    expect(normalizeAccelerator("option+command+digit7", "macos")).toEqual({
      ok: true,
      value: "Cmd+Alt+7",
    });
    expect(normalizeAccelerator("ctrl+arrowup", "macos")).toEqual({
      ok: true,
      value: "Ctrl+Up",
    });
    expect(normalizeAccelerator("Control+KeyN", "other")).toEqual({
      ok: true,
      value: "Ctrl+N",
    });
  });

  it("reorders trailing modifiers into the canonical form", () => {
    expect(normalizeAccelerator("y+shift+cmd", "macos")).toEqual({
      ok: true,
      value: "Cmd+Shift+Y",
    });
  });

  it("resolves CmdOrCtrl per platform", () => {
    expect(normalizeAccelerator("CmdOrCtrl+Shift+Y", "macos")).toEqual({
      ok: true,
      value: "Cmd+Shift+Y",
    });
    expect(normalizeAccelerator("CmdOrCtrl+Shift+Y", "other")).toEqual({
      ok: true,
      value: "Ctrl+Shift+Y",
    });
  });

  it("keeps real Ctrl distinct from Cmd on macOS", () => {
    expect(normalizeAccelerator("Ctrl+Shift+Y", "macos")).toEqual({
      ok: true,
      value: "Ctrl+Shift+Y",
    });
  });

  it("allows bare function keys", () => {
    expect(normalizeAccelerator("F5", "macos")).toEqual({ ok: true, value: "F5" });
    expect(normalizeAccelerator("f24", "other")).toEqual({ ok: true, value: "F24" });
  });

  it("accepts punctuation keys", () => {
    for (const key of ["-", "=", "[", "]", ",", ".", "/", ";", "'", "`", "\\"]) {
      expect(normalizeAccelerator(`Cmd+${key}`, "macos")).toEqual({
        ok: true,
        value: `Cmd+${key}`,
      });
    }
  });

  it("rejects invalid accelerators", () => {
    for (const input of [
      "",
      "   ",
      "++",
      "Cmd+",
      "Cmd",
      "Shift",
      "Y",
      "Cmd+Y+W",
      "Cmd+Bogus",
      "Cmd+Ctrl+Y",
      "Cmd+F25",
      "Cmd+F0",
    ]) {
      expect(normalizeAccelerator(input, "macos").ok).toBe(false);
    }
  });

  it("normalizes the shipped defaults", () => {
    for (const value of [
      DEFAULT_GLOBAL_SHORTCUTS.summon,
      DEFAULT_GLOBAL_SHORTCUTS.approve,
      DEFAULT_GLOBAL_SHORTCUTS.deny,
      DEFAULT_GLOBAL_SHORTCUTS.always,
    ]) {
      expect(normalizeAccelerator(value, "macos").ok).toBe(true);
      expect(normalizeAccelerator(value, "other").ok).toBe(true);
    }
  });
});

describe("formatAccelerator", () => {
  it("resolves CmdOrCtrl for display", () => {
    expect(formatAccelerator("CmdOrCtrl+Shift+Space", "macos")).toBe("Cmd+Shift+Space");
    expect(formatAccelerator("CmdOrCtrl+Shift+Space", "other")).toBe("Ctrl+Shift+Space");
  });

  it("shows Win for stored Cmd bindings off macOS", () => {
    expect(formatAccelerator("Cmd+Shift+Y", "other")).toBe("Win+Shift+Y");
    expect(formatAccelerator("Cmd+Shift+Y", "macos")).toBe("Cmd+Shift+Y");
  });

  it("renders cleared bindings as empty", () => {
    expect(formatAccelerator("", "macos")).toBe("");
  });
});

describe("acceleratorFromKeyboardEvent", () => {
  const base = {
    code: "KeyY",
    key: "y",
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    shiftKey: false,
  };

  it("maps mac Cmd+Shift+letter", () => {
    expect(
      acceleratorFromKeyboardEvent({ ...base, metaKey: true, shiftKey: true }, "macos"),
    ).toBe("Cmd+Shift+Y");
  });

  it("maps Ctrl+Shift on Windows", () => {
    expect(
      acceleratorFromKeyboardEvent({ ...base, ctrlKey: true, shiftKey: true }, "other"),
    ).toBe("Ctrl+Shift+Y");
  });

  it("keeps real Ctrl on macOS and refuses Cmd+Ctrl combos", () => {
    expect(acceleratorFromKeyboardEvent({ ...base, ctrlKey: true }, "macos")).toBe("Ctrl+Y");
    expect(
      acceleratorFromKeyboardEvent({ ...base, metaKey: true, ctrlKey: true }, "macos"),
    ).toBeNull();
  });

  it("maps named and function keys", () => {
    expect(
      acceleratorFromKeyboardEvent(
        { ...base, code: "Space", key: " ", metaKey: true },
        "macos",
      ),
    ).toBe("Cmd+Space");
    expect(acceleratorFromKeyboardEvent({ ...base, code: "F5", key: "F5" }, "other")).toBe("F5");
    expect(
      acceleratorFromKeyboardEvent({ ...base, code: "ArrowUp", key: "Up", altKey: true }, "macos"),
    ).toBe("Alt+Up");
  });

  it("ignores bare keys that would swallow typing", () => {
    expect(acceleratorFromKeyboardEvent(base, "macos")).toBeNull();
    expect(
      acceleratorFromKeyboardEvent({ ...base, code: "Space", key: " " }, "macos"),
    ).toBeNull();
  });

  it("ignores lone modifier presses and the Windows key off macOS", () => {
    expect(
      acceleratorFromKeyboardEvent({ ...base, code: "ShiftLeft", key: "Shift", shiftKey: true }, "macos"),
    ).toBeNull();
    expect(
      acceleratorFromKeyboardEvent({ ...base, metaKey: true }, "other"),
    ).toBeNull();
  });
});

describe("shortcutPlatform", () => {
  it("detects macOS from the platform string", () => {
    expect(shortcutPlatform("MacIntel")).toBe("macos");
    expect(shortcutPlatform("iPhone")).toBe("macos");
    expect(shortcutPlatform("Win32")).toBe("other");
    expect(shortcutPlatform("")).toBe("other");
  });
});

describe("withShortcutAction", () => {
  it("replaces a single action binding", () => {
    const next = withShortcutAction(DEFAULT_GLOBAL_SHORTCUTS, "approve", "Ctrl+Alt+K");
    expect(next.approve).toBe("Ctrl+Alt+K");
    expect(next.summon).toBe(DEFAULT_GLOBAL_SHORTCUTS.summon);
    expect(next.deny).toBe(DEFAULT_GLOBAL_SHORTCUTS.deny);
    expect(DEFAULT_GLOBAL_SHORTCUTS.approve).toBe("CmdOrCtrl+Shift+Y");
  });
});
