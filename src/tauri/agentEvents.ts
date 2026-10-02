import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauriRuntime } from "./runtime";
import type { AgentKind } from "./types";

export interface HookObservation {
  generation: number;
  lastEventAt: number | null;
  lastEventName: string | null;
  lastPermissionAt: number | null;
}
export interface LifecycleEvent {
  eventId: string;
  agent: AgentKind;
  sessionId: string;
  subagentId: string | null;
  kind: "started" | "subagentStarted" | "subagentEnded" | "turnEnded";
  occurredAt: number;
}
export async function getHookObservations(): Promise<Record<string, HookObservation>> {
  return isTauriRuntime() ? invoke("get_hook_observations") : {};
}
export async function onHookObserved(callback: (event: { agent: string; observation: HookObservation }) => void) {
  return isTauriRuntime() ? listen<{ agent: string; observation: HookObservation }>("hook-observed", event => callback(event.payload)) : () => undefined;
}
export async function onAgentLifecycleChanged(callback: (event: LifecycleEvent) => void) {
  return isTauriRuntime() ? listen<LifecycleEvent>("agent-lifecycle-changed", event => callback(event.payload)) : () => undefined;
}
