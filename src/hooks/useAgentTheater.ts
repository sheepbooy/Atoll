import { useEffect, useRef, useState } from "react";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { onAgentLifecycleChanged, type LifecycleEvent } from "../tauri/agentEvents";

export const THEATER_DURATION_MS = 2_400;
export interface TheaterScene {
  event: LifecycleEvent;
  actors: (string | null)[];
}

// Event-only: mounting never asks for past events, and a newer event replaces
// the current scene. Events hidden by approval or drag are never queued.
export function useAgentTheater(blocked: boolean) {
  const [scene, setScene] = useState<TheaterScene | null>(null);
  const blockedRef = useRef(blocked);
  blockedRef.current = blocked;
  useEffect(() => {
    const mountedAt = Date.now();
    const seen = new Set<string>();
    const casts = new Map<string, Set<string>>();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const unsubscribe = manageAsyncUnlisten(onAgentLifecycleChanged(event => {
      if (event.occurredAt < mountedAt || seen.has(event.eventId)) return;
      seen.add(event.eventId);
      if (seen.size > 256) seen.delete(seen.values().next().value!);
      const key = `${event.agent}:${event.sessionId}`;
      const cast = casts.get(key) ?? new Set<string>();
      if (event.kind === "subagentStarted" && event.subagentId) cast.add(event.subagentId);
      const actors: (string | null)[] = [null, ...cast];
      if (event.kind === "subagentEnded" && event.subagentId) {
        if (!cast.has(event.subagentId)) actors.push(event.subagentId);
        cast.delete(event.subagentId);
      }
      // Keep memory bounded even if an agent never sends completion events.
      while (cast.size > 128) cast.delete(cast.values().next().value!);
      casts.set(key, cast);
      if (casts.size > 64) casts.delete(casts.keys().next().value!);
      clearTimeout(timer);
      if (blockedRef.current) { setScene(null); return; }
      setScene({ event, actors });
      timer = setTimeout(() => setScene(null), THEATER_DURATION_MS);
    }));
    return () => { clearTimeout(timer); unsubscribe(); };
  }, []);
  useEffect(() => { if (blocked) setScene(null); }, [blocked]);
  return blocked ? null : scene;
}
