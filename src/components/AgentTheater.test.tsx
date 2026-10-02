import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { AgentTheater } from "./AgentTheater";
import type { TheaterScene } from "../hooks/useAgentTheater";
const scene: TheaterScene = { event: { eventId: "one", agent: "codex", sessionId: "s", subagentId: "d", kind: "subagentStarted", occurredAt: Date.now() }, actors: [null, "a", "b", "c", "d"] };
it("caps the cast at three, keeps the new teammate visible and displays overflow", () => {
  const { container } = render(<AgentTheater scene={scene} />);
  expect(container.querySelectorAll(".agent-theater-actor")).toHaveLength(3);
  expect(container.querySelector(".is-subagentStarted")).toBeInTheDocument();
  expect(screen.getByText("+2")).toBeInTheDocument();
});
it("uses an existing single mascot slot on narrow islands", () => {
  const { container } = render(<AgentTheater scene={scene} capacity={1} />);
  expect(container.querySelectorAll(".agent-theater-actor")).toHaveLength(1);
  expect(container.querySelector(".is-subagentStarted")).toBeInTheDocument();
});
it("calls completion a turn ending without claiming task success", () => {
  render(<AgentTheater scene={{ ...scene, event: { ...scene.event, kind: "turnEnded", subagentId: null } }} />);
  expect(screen.getByRole("status", { name: "Turn ended" })).toBeInTheDocument();
});
