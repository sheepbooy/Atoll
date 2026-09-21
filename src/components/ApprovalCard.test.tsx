import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ApprovalCard } from "./ApprovalCard";
import type { PermissionRequest } from "../tauri";

function makeRequest(overrides: Partial<PermissionRequest> = {}): PermissionRequest {
  return {
    id: "req-1",
    agent: "claude",
    session: "session-1",
    command: "Bash: npm test",
    detail: "",
    cwd: "/work/atoll",
    requestedAt: "2026-09-21T00:00:00Z",
    status: "pending",
    supportsAlways: true,
    ...overrides,
  };
}

function renderCard(props: { onCreateRule?: (scope: string) => void } = {}) {
  return render(
    <ApprovalCard
      request={makeRequest()}
      busyDecision={null}
      sessions={[]}
      onApprove={vi.fn()}
      onDeny={vi.fn()}
      onAlwaysApprove={vi.fn()}
      onCreateRule={props.onCreateRule}
      onViewSession={vi.fn()}
    />,
  );
}

describe("ApprovalCard rule scope menu", () => {
  it("keeps the always button a direct session-level action", () => {
    const onAlwaysApprove = vi.fn();
    render(
      <ApprovalCard
        request={makeRequest()}
        busyDecision={null}
        sessions={[]}
        onApprove={vi.fn()}
        onDeny={vi.fn()}
        onAlwaysApprove={onAlwaysApprove}
        onViewSession={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /Always/ }));
    expect(onAlwaysApprove).toHaveBeenCalledTimes(1);
  });

  it("opens the scope menu from the chevron and creates a rule for the chosen scope", () => {
    const onCreateRule = vi.fn();
    renderCard({ onCreateRule });

    fireEvent.click(screen.getByRole("button", { name: "Always allow with a rule" }));
    expect(screen.getByText("This command · this project")).toBeInTheDocument();
    expect(screen.getByText("Everything in this project")).toBeInTheDocument();

    fireEvent.click(screen.getByText("This command · this project"));
    expect(onCreateRule).toHaveBeenCalledWith("command_project");
    // The menu closes after picking a scope.
    expect(screen.queryByText("This command · anywhere")).not.toBeInTheDocument();
  });

  it("falls back to session scope from the menu", () => {
    const onAlwaysApprove = vi.fn();
    render(
      <ApprovalCard
        request={makeRequest()}
        busyDecision={null}
        sessions={[]}
        onApprove={vi.fn()}
        onDeny={vi.fn()}
        onAlwaysApprove={onAlwaysApprove}
        onCreateRule={vi.fn()}
        onViewSession={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Always allow with a rule" }));
    fireEvent.click(screen.getByText("This session"));
    expect(onAlwaysApprove).toHaveBeenCalledTimes(1);
  });

  it("hides the scope chevron when rule creation is unavailable", () => {
    renderCard();

    expect(
      screen.queryByRole("button", { name: "Always allow with a rule" }),
    ).not.toBeInTheDocument();
  });
});
