import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { NotificationSettingsView } from "./SettingsPages";

describe("NotificationSettingsView", () => {
  it("marks interrupt as the active segment by default", () => {
    render(
      <NotificationSettingsView mode="interrupt" onChangeMode={vi.fn()} />,
    );

    expect(screen.getByRole("button", { name: "Interrupt" })).toHaveClass("is-active");
    expect(screen.getByRole("button", { name: "Notify only" })).not.toHaveClass("is-active");
    expect(screen.getByText(/expand and focus the island/i)).toBeInTheDocument();
  });

  it("describes notify-only mode when it is selected", () => {
    render(<NotificationSettingsView mode="notify" onChangeMode={vi.fn()} />);

    expect(screen.getByRole("button", { name: "Notify only" })).toHaveClass("is-active");
    expect(screen.getByText(/system notification/i)).toBeInTheDocument();
  });

  it("reports the selected mode back through onChangeMode", () => {
    const onChangeMode = vi.fn();
    render(
      <NotificationSettingsView mode="interrupt" onChangeMode={onChangeMode} />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Notify only" }));
    expect(onChangeMode).toHaveBeenCalledWith("notify");
  });
});
