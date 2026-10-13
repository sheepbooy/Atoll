import { useEffect, useState } from "react";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import { onAgentLifecycleChanged } from "../tauri/agentEvents";

/** Live turn evidence only: opening a historical session never claims it is working. */
export function useAgentActivity() {
  const [working, setWorking] = useState<ReadonlySet<string>>(() => new Set());
  useEffect(() => {
    const mountedAt = Date.now();
    const lastEvents = new Map<string, number>();
    return manageAsyncUnlisten(onAgentLifecycleChanged(event => {
      if (event.subagentId !== null || !["started", "turnEnded"].includes(event.kind) || event.occurredAt < mountedAt) return;
      const key = `${event.agent}:${event.sessionId}`;
      if (event.occurredAt < (lastEvents.get(key) ?? 0)) return;
      lastEvents.set(key, event.occurredAt);
      if (lastEvents.size > 256) lastEvents.delete(lastEvents.keys().next().value!);
      setWorking(previous => {
        const next = new Set(previous);
        if (event.kind === "started") next.add(key); else next.delete(key);
        if (next.size > 128) next.delete(next.values().next().value!);
        return next;
      });
    }));
  }, []);
  return working;
}
