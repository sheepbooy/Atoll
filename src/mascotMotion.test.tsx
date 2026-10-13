import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AgentMascot } from "./AgentMascot";
import { MASCOT_ACTION_MS, MascotEnvironmentProvider } from "./mascotMotion";
import { mockMotionPreference } from "./test-utils/motionPreference";
import type { AgentKind } from "./tauri";

const agents: AgentKind[] = ["claude", "codex", "cursor", "gemini", "opencode", "zcode", "other"];
beforeEach(() => { vi.useFakeTimers(); mockMotionPreference(); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); Reflect.deleteProperty(HTMLElement.prototype, "getAnimations"); });

describe("character motion lifecycle", () => {
  it.each(agents)("%s has articulated parts and no motion timers when static", agent => {
    const { container } = render(<AgentMascot agent={agent} mood="alert" animated={false} />);
    expect(container.querySelector("[data-part]")).not.toBeNull();
    expect(container.querySelector(".agent-mascot")).toHaveClass("is-motion-disabled");
    expect(vi.getTimerCount()).toBe(0);
  });

  it("restarts repeated actions without remounting parts or restarting ambient animations", () => {
    const shot = { animationName: "agent-shot-petal-open", currentTime: 250 };
    const loop = { animationName: "agent-work-petal", currentTime: 450 };
    Object.defineProperty(HTMLElement.prototype, "getAnimations", { configurable: true, value: () => [shot, loop] });
    const { container, rerender } = render(<AgentMascot agent="codex" mood="calm" action="approved" actionKey={1} />);
    const svg = container.querySelector("svg");
    const petal = container.querySelector('[data-part="petal-0"]');
    shot.currentTime = 350;
    rerender(<AgentMascot agent="codex" mood="calm" action="approved" actionKey={2} />);
    expect(shot.currentTime).toBe(0);
    expect(loop.currentTime).toBe(450);
    expect(container.querySelector("svg")).toBe(svg);
    expect(container.querySelector('[data-part="petal-0"]')).toBe(petal);
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    expect(container.querySelector(".has-action")).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("cancels a shot on live reduced-motion changes and never replays it on recovery", () => {
    const reduce = mockMotionPreference();
    const { container } = render(<AgentMascot agent="gemini" mood="calm" action="approved" actionKey={1} />);
    expect(container.querySelector(".has-action")).not.toBeNull();
    act(() => reduce(true));
    expect(container.querySelector(".has-action")).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
    act(() => reduce(false));
    expect(container.querySelector(".has-action")).toBeNull();
  });

  it("pauses ambient motion while finite feedback completes", () => {
    const { container } = render(<MascotEnvironmentProvider ambientPaused feedback={null}>
      <AgentMascot agent="opencode" mood="calm" activity="working" action="started" />
      <AgentMascot agent="claude" mood="calm" />
    </MascotEnvironmentProvider>);
    expect(container.querySelector(".is-loop-working, .is-loop-idle")).toBeNull();
    expect(container.querySelector(".opencode")).toHaveClass("is-action-started");
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    expect(container.querySelector(".has-action")).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps slowed feedback alive for the entire slower timeline", () => {
    const { container } = render(<MascotEnvironmentProvider ambientPaused={false} feedback={null} rate={.5}>
      <AgentMascot agent="cursor" mood="calm" action="approved" />
    </MascotEnvironmentProvider>);
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    expect(container.querySelector(".has-action")).not.toBeNull();
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    expect(container.querySelector(".has-action")).toBeNull();
  });

  it("gives a raised-hand reminder time to wait and resumes at the next four-second beat", () => {
    const { container } = render(<AgentMascot agent="gemini" mood="alert" />);
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    expect(container.querySelector(".gemini")).toHaveClass("is-action-attention");
    act(() => vi.advanceTimersByTime(700));
    const root = container.querySelector(".gemini") as HTMLElement;
    expect(root).not.toHaveClass("has-action");
    expect(root.style.getPropertyValue("--agent-remind-phase")).toBe("2400ms");
    expect(container.querySelector("animate")?.getAttribute("repeatCount")).toBe("indefinite");
  });

  it("restarts Gemini's contour feedback on the same SVG and removes it when cancelled", () => {
    const begin = vi.fn();
    Object.defineProperty(SVGElement.prototype,"beginElementAt",{configurable:true,value:begin});
    try {
      const { container,rerender } = render(<AgentMascot agent="gemini" mood="calm" activity="rest" action="approved" actionKey={1} />);
      const svg = container.querySelector("svg");
      expect(begin).toHaveBeenCalledTimes(5);
      rerender(<AgentMascot agent="gemini" mood="calm" activity="rest" action="approved" actionKey={2} />);
      expect(begin).toHaveBeenCalledTimes(10);
      expect(container.querySelector("svg")).toBe(svg);
      rerender(<AgentMascot agent="gemini" mood="calm" activity="rest" action={null} />);
      expect(container.querySelector("animate")).toBeNull();
      expect(vi.getTimerCount()).toBe(0);
    } finally { Reflect.deleteProperty(SVGElement.prototype,"beginElementAt"); }
  });

  it("keeps the ambient phase stable when a resolved approval leaves the feedback context", () => {
    const { container,rerender } = render(<AgentMascot agent="codex" mood="calm" activity="working" instanceKey="phase-sensitive-session" action="approved" actionKey={1} />);
    act(() => vi.advanceTimersByTime(MASCOT_ACTION_MS));
    const root = container.querySelector(".codex") as HTMLElement;
    const phase = root.style.getPropertyValue("--agent-phase");
    rerender(<AgentMascot agent="codex" mood="calm" activity="working" instanceKey="phase-sensitive-session" action={null} />);
    expect(root.style.getPropertyValue("--agent-phase")).toBe(phase);
    expect(root).toHaveClass("is-loop-working");
  });

  it("restricts decision feedback to its session and lets a pending request take priority", () => {
    const { container } = render(<MascotEnvironmentProvider ambientPaused={false} feedback={{ eventId: "r:1", requestId: "r", agent: "codex", sessionId: "one", decision: "denied" }}>
      <AgentMascot agent="codex" mood="calm" instanceKey="one" />
      <AgentMascot agent="codex" mood="calm" instanceKey="two" />
      <AgentMascot agent="cursor" mood="alert" instanceKey="one" />
      <AgentMascot agent="cursor" mood="calm" instanceKey="one" />
    </MascotEnvironmentProvider>);
    expect(container.querySelectorAll(".is-action-denied")).toHaveLength(1);
    expect(container.querySelector(".cursor-mascot")).toHaveClass("is-action-attention");
    expect(container.querySelector(".is-action-approved")).toBeNull();
  });

  it("does not share mask or clip ids across simultaneously rendered characters", () => {
    const { container } = render(<>{agents.flatMap(agent => ["one", "two"].map(instanceKey =>
      <AgentMascot key={`${agent}:${instanceKey}`} agent={agent} mood="calm" instanceKey={instanceKey} />))}</>);
    const ids = [...container.querySelectorAll("[id]")].map(n => n.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it.each(["approved", "denied"] as const)("routes %s to the matching brand tab without notifying unrelated session avatars", decision => {
    const { container } = render(<MascotEnvironmentProvider ambientPaused={false} feedback={{ eventId:"decision:1",requestId:"request",agent:"gemini",sessionId:"one",decision }}>
      <AgentMascot agent="gemini" instanceKey="gemini" feedbackScope="agent" mood="calm" />
      <AgentMascot agent="codex" instanceKey="codex" feedbackScope="agent" mood="calm" />
      <AgentMascot agent="gemini" instanceKey="one" mood="calm" />
      <AgentMascot agent="gemini" instanceKey="two" mood="calm" />
      <AgentMascot agent="gemini" instanceKey="pending-tab" feedbackScope="agent" mood="alert" />
    </MascotEnvironmentProvider>);
    expect(container.querySelectorAll(`.is-action-${decision}`)).toHaveLength(2);
    expect(container.querySelectorAll(".gemini")[3]).toHaveClass("is-action-attention");
  });

  it("uses live work evidence only for the matching agent and session", () => {
    const { container } = render(<MascotEnvironmentProvider ambientPaused={false} feedback={null} workingSessions={new Set(["codex:one"])}>
      <AgentMascot agent="codex" mood="calm" instanceKey="one" />
      <AgentMascot agent="claude" mood="calm" instanceKey="one" />
      <AgentMascot agent="codex" mood="calm" instanceKey="two" />
    </MascotEnvironmentProvider>);
    expect(container.querySelectorAll(".is-loop-working")).toHaveLength(1);
    expect(container.querySelectorAll(".is-loop-idle")).toHaveLength(2);
  });
});
