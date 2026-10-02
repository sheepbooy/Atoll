import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, afterEach, expect, it, vi } from "vitest";
import { HookConnectionCheck, HOOK_VERIFY_TIMEOUT_MS } from "./HookConnectionCheck";
import type { HookObservation } from "../tauri/agentEvents";
const mocks = vi.hoisted(() => ({ listener: null as null | ((e: { agent: string; observation: HookObservation }) => void), initial: {} as Record<string, HookObservation> }));
vi.mock("../tauri/agentEvents", () => ({ getHookObservations: vi.fn(async () => mocks.initial), onHookObserved: vi.fn(async cb => { mocks.listener = cb; return () => undefined; }) }));
beforeEach(() => { vi.useFakeTimers(); mocks.initial = {}; });
afterEach(() => vi.useRealTimers());
const evidence = (overrides: Partial<HookObservation> = {}): HookObservation => ({ generation: 0, lastEventAt: Date.now(), lastEventName: "Stop", lastPermissionAt: null, ...overrides });
it("ignores historical evidence and events from another agent; times out", async () => {
  mocks.initial = { codex: evidence({ lastEventAt: Date.now() - 1_000 }) };
  render(<HookConnectionCheck agent="codex" installed />);
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "Verify connection" }));
  act(() => mocks.listener?.({ agent: "claude", observation: evidence() }));
  expect(screen.queryByText("Connection verified")).not.toBeInTheDocument();
  await act(async () => { await vi.advanceTimersByTimeAsync(HOOK_VERIFY_TIMEOUT_MS); });
  expect(screen.getByText("No new event received in 60 seconds")).toBeInTheDocument();
});
it("requires a new real event and resets verification after reinstallation", async () => {
  render(<HookConnectionCheck agent="codex" installed />);
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "Verify connection" }));
  act(() => mocks.listener?.({ agent: "codex", observation: evidence({ lastPermissionAt: Date.now() }) }));
  expect(screen.getByText("Connection verified")).toBeInTheDocument();
  expect(screen.getByText("Permission request received")).toBeInTheDocument();
  act(() => mocks.listener?.({ agent: "codex", observation: evidence({ generation: 1, lastEventAt: null, lastPermissionAt: null }) }));
  expect(screen.getByText("Configuration installed")).toBeInTheDocument();
});
it("describes Cursor as an observer", async () => {
  render(<HookConnectionCheck agent="cursor" installed />);
  await act(async () => {});
  fireEvent.click(screen.getByRole("button", { name: "Verify connection" }));
  act(() => mocks.listener?.({ agent: "cursor", observation: evidence() }));
  expect(screen.getByText("Observer connection verified")).toBeInTheDocument();
  expect(screen.queryByText("Permission request received")).not.toBeInTheDocument();
});
