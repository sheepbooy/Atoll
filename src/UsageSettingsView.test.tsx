import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UsageSettingsView, type UsageSettingsViewProps } from "./UsageSettingsView";
import { DEFAULT_SALARY_SETTINGS } from "./salarySettings";

function renderView(overrides: Partial<UsageSettingsViewProps> = {}) {
  const props = {
    foldedCounterDisplay: "tokens" as const,
    expandedCounterDisplay: "tokens" as const,
    settingsBadgeDisplay: "tokens" as const,
    heatmapDisplay: "tokens" as const,
    onChangeFoldedCounterDisplay: vi.fn(),
    onChangeExpandedCounterDisplay: vi.fn(),
    onChangeSettingsBadgeDisplay: vi.fn(),
    onChangeHeatmapDisplay: vi.fn(),
    salarySettings: DEFAULT_SALARY_SETTINGS,
    onChangeSalarySettings: vi.fn(),
    pricingModels: [],
    onPricingModelsChange: vi.fn(),
    ...overrides,
  };
  render(<UsageSettingsView {...props} />);
  return props;
}

describe("UsageSettingsView", () => {
  it("offers the salary segment on every display toggle", () => {
    renderView();
    expect(screen.getAllByRole("button", { name: "Salary" })).toHaveLength(4);
  });

  it("switches the folded counter to salary mode", () => {
    const props = renderView();
    fireEvent.click(screen.getAllByRole("button", { name: "Salary" })[0]);
    expect(props.onChangeFoldedCounterDisplay).toHaveBeenCalledWith("salary");
    expect(props.onChangeFoldedCounterDisplay).toHaveBeenCalledTimes(1);
  });

  it("edits the monthly wage through the salary card", () => {
    const props = renderView();
    fireEvent.change(screen.getByLabelText(/monthly wage/i), {
      target: { value: "35000" },
    });
    expect(props.onChangeSalarySettings).toHaveBeenCalledWith({
      ...DEFAULT_SALARY_SETTINGS,
      monthlyWage: 35000,
    });
  });

  it("clamps work day edits into range", () => {
    const props = renderView();
    fireEvent.change(screen.getByLabelText(/work days/i), {
      target: { value: "0" },
    });
    expect(props.onChangeSalarySettings).toHaveBeenCalledWith({
      ...DEFAULT_SALARY_SETTINGS,
      workDaysPerMonth: 1,
    });
  });

  it("switches the salary currency segment", () => {
    const props = renderView();
    fireEvent.click(screen.getByRole("button", { name: "$" }));
    expect(props.onChangeSalarySettings).toHaveBeenCalledWith({
      ...DEFAULT_SALARY_SETTINGS,
      currency: "$",
    });
  });

  it("previews the per-second rate derived from the settings", () => {
    renderView();
    expect(screen.getByText(/0\.0316/)).toBeInTheDocument();
  });
});
