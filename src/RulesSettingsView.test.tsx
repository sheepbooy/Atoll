import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RulesSettingsView } from "./RulesSettingsView";
import type { ApprovalRule } from "./tauri";
import {
  getApprovalRules,
  getRiskGuardEnabled,
  saveApprovalRules,
  setRiskGuardEnabled,
} from "./tauri";

vi.mock("./tauri", async (importOriginal) => {
  const actual = await importOriginal<typeof import("./tauri")>();
  return {
    ...actual,
    getApprovalRules: vi.fn(),
    getRiskGuardEnabled: vi.fn(),
    saveApprovalRules: vi.fn(),
    setRiskGuardEnabled: vi.fn(),
  };
});

const mockGetRules = vi.mocked(getApprovalRules);
const mockGetGuard = vi.mocked(getRiskGuardEnabled);
const mockSaveRules = vi.mocked(saveApprovalRules);
const mockSetGuard = vi.mocked(setRiskGuardEnabled);

function makeRule(overrides: Partial<ApprovalRule> = {}): ApprovalRule {
  return {
    id: "rule-1",
    name: "npm allow",
    enabled: true,
    decision: "allow",
    note: "",
    agent: "claude",
    tool: undefined,
    pattern: "Bash: npm *",
    projectPath: undefined,
    createdAt: 0,
    matchCount: 0,
    lastMatchedAt: null,
    ...overrides,
  };
}

describe("RulesSettingsView", () => {
  beforeEach(() => {
    mockGetRules.mockResolvedValue([]);
    mockGetGuard.mockResolvedValue(true);
    mockSaveRules.mockImplementation(async (rules) => rules);
    mockSetGuard.mockResolvedValue(true);
  });

  it("lists rules with decision badges and match stats", async () => {
    mockGetRules.mockResolvedValue([
      makeRule({ matchCount: 3, lastMatchedAt: Math.floor(Date.now() / 1000) - 120 }),
      makeRule({
        id: "rule-2",
        name: "block force push",
        decision: "deny",
        pattern: "Bash: git push --force*",
        enabled: false,
      }),
    ]);
    render(<RulesSettingsView />);

    expect(await screen.findByText("npm allow")).toBeInTheDocument();
    expect(screen.getByText("block force push")).toBeInTheDocument();
    expect(screen.getByText("Bash: npm * · Claude Code")).toBeInTheDocument();
    expect(screen.getByText(/Matched 3×/)).toBeInTheDocument();
    expect(screen.getByText(/Never matched/)).toBeInTheDocument();
    // Decision badges appear once per rule (Allow twice with the editor hidden).
    expect(screen.getAllByText("Allow").length).toBe(1);
    expect(screen.getAllByText("Deny").length).toBe(1);
    // The disabled rule still offers Enable instead of Disable.
    expect(screen.getByRole("button", { name: "Enable" })).toBeInTheDocument();
  });

  it("toggles the risk guard through the backend", async () => {
    render(<RulesSettingsView />);
    const toggle = await screen.findByRole("switch");
    fireEvent.click(toggle);

    await waitFor(() => expect(mockSetGuard).toHaveBeenCalledWith(false));
  });

  it("creates a rule from the add form", async () => {
    render(<RulesSettingsView />);
    fireEvent.click(await screen.findByRole("button", { name: "Add rule" }));

    fireEvent.change(screen.getByPlaceholderText("e.g. npm commands"), {
      target: { value: "npm tools" },
    });
    fireEvent.change(screen.getByPlaceholderText("Bash: npm *"), {
      target: { value: "Bash: npm install*" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(mockSaveRules).toHaveBeenCalledTimes(1));
    const saved = mockSaveRules.mock.calls[0][0];
    expect(saved).toHaveLength(1);
    expect(saved[0]).toMatchObject({
      name: "npm tools",
      decision: "allow",
      pattern: "Bash: npm install*",
      enabled: true,
    });
  });

  it("refuses to save a rule without any matcher", async () => {
    render(<RulesSettingsView />);
    fireEvent.click(await screen.findByRole("button", { name: "Add rule" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByText(/Set at least one matcher/)).toBeInTheDocument();
    expect(mockSaveRules).not.toHaveBeenCalled();
  });

  it("deletes a rule only after a second confirming click", async () => {
    mockGetRules.mockResolvedValue([makeRule()]);
    render(<RulesSettingsView />);
    const deleteButton = await screen.findByRole("button", { name: "Delete" });

    fireEvent.click(deleteButton);
    expect(mockSaveRules).not.toHaveBeenCalled();
    expect(
      await screen.findByRole("button", { name: "Click again to confirm" }),
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Click again to confirm" }));
    await waitFor(() => expect(mockSaveRules).toHaveBeenCalledWith([]));
  });
});
