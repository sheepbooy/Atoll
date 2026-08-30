import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ShortcutSettingsView } from "./SettingsPages";
import { DEFAULT_GLOBAL_SHORTCUTS } from "./shortcuts";
import type { GlobalShortcutErrors } from "./tauri";

const NO_ERRORS: GlobalShortcutErrors = {};

function baseProps() {
  return {
    config: { ...DEFAULT_GLOBAL_SHORTCUTS },
    errors: NO_ERRORS,
    platform: "macos" as const,
    onChangeEnabled: vi.fn(),
    onChangeAccelerator: vi.fn(),
  };
}

describe("ShortcutSettingsView", () => {
  it("renders the three action bindings from the config", () => {
    render(<ShortcutSettingsView {...baseProps()} />);

    expect(screen.getByDisplayValue("Cmd+Shift+Space")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Cmd+Shift+Y")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Cmd+Shift+N")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Global shortcuts" })).toBeChecked();
  });

  it("reports master toggle changes through onChangeEnabled", () => {
    const onChangeEnabled = vi.fn();
    render(<ShortcutSettingsView {...baseProps()} onChangeEnabled={onChangeEnabled} />);

    fireEvent.click(screen.getByRole("switch", { name: "Global shortcuts" }));
    expect(onChangeEnabled).toHaveBeenCalledWith(false);
  });

  it("records a new accelerator on keydown", () => {
    const onChangeAccelerator = vi.fn();
    render(
      <ShortcutSettingsView {...baseProps()} onChangeAccelerator={onChangeAccelerator} />,
    );

    const summon = screen.getByDisplayValue("Cmd+Shift+Space");
    fireEvent.focus(summon);
    fireEvent.keyDown(summon, {
      code: "KeyJ",
      key: "j",
      ctrlKey: false,
      metaKey: true,
      altKey: false,
      shiftKey: true,
    });

    expect(onChangeAccelerator).toHaveBeenCalledWith("summon", "Cmd+Shift+J");
  });

  it("shows the modifier hint for bare keys and does not report them", () => {
    const onChangeAccelerator = vi.fn();
    render(
      <ShortcutSettingsView {...baseProps()} onChangeAccelerator={onChangeAccelerator} />,
    );

    const approve = screen.getByDisplayValue("Cmd+Shift+Y");
    fireEvent.focus(approve);
    fireEvent.keyDown(approve, {
      code: "KeyY",
      key: "y",
      ctrlKey: false,
      metaKey: false,
      altKey: false,
      shiftKey: false,
    });

    expect(onChangeAccelerator).not.toHaveBeenCalled();
    expect(screen.getByText(/add a modifier/i)).toBeInTheDocument();
  });

  it("exits recording on Escape without reporting a binding", () => {
    const onChangeAccelerator = vi.fn();
    render(
      <ShortcutSettingsView {...baseProps()} onChangeAccelerator={onChangeAccelerator} />,
    );

    // Real DOM focus so the later blur() actually clears the recording state.
    const deny = screen.getByDisplayValue("Cmd+Shift+N");
    deny.focus();
    fireEvent.keyDown(deny, { key: "Escape" });

    expect(onChangeAccelerator).not.toHaveBeenCalled();
    expect(screen.queryByText(/press a key combination/i)).not.toBeInTheDocument();
  });

  it("surfaces per-action registration errors", () => {
    render(
      <ShortcutSettingsView
        {...baseProps()}
        errors={{ summon: "register Cmd+Shift+Space: taken by another app" }}
      />,
    );

    expect(
      screen.getByText("register Cmd+Shift+Space: taken by another app"),
    ).toBeInTheDocument();
  });

  it("clears a binding through the clear button", () => {
    const onChangeAccelerator = vi.fn();
    render(
      <ShortcutSettingsView {...baseProps()} onChangeAccelerator={onChangeAccelerator} />,
    );

    fireEvent.click(screen.getAllByRole("button", { name: "Clear" })[0]);
    expect(onChangeAccelerator).toHaveBeenCalledWith("summon", "");
  });

  it("disables the inputs while shortcuts are off", () => {
    render(
      <ShortcutSettingsView
        {...baseProps()}
        config={{ ...DEFAULT_GLOBAL_SHORTCUTS, enabled: false }}
      />,
    );

    expect(screen.getByDisplayValue("Cmd+Shift+Space")).toBeDisabled();
    expect(screen.getByRole("switch", { name: "Global shortcuts" })).not.toBeChecked();
  });
});
