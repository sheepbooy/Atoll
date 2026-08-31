import { useEffect, useRef, useState } from "react";
import {
  deactivateAtoll,
  getSessionRequests,
  resolvePermissionRequest,
  setSessionAutoApprove,
  type IslandSnapshot,
  type PermissionRequest,
} from "../tauri";
import { RESOLVE_FEEDBACK_MS } from "../islandPresentation";
import type { AgentKind, Decision, PanelView } from "../appTypes";

interface UseApprovalsOptions {
  snapshot: IslandSnapshot;
  snapshotRef: { current: IslandSnapshot };
  panelView: PanelView;
  selectedAgentRef: { current: AgentKind | null };
  menuOpenRef: { current: boolean };
  navigationSeqRef: { current: number };
  applySnapshot: (snapshot: IslandSnapshot, options?: { mergeHookHealth?: boolean }) => void;
  collapseIsland: (skipAnimation?: boolean) => void;
  scheduleIdleCollapse: () => void;
  setSessionRequests: (requests: PermissionRequest[]) => void;
}

export function useApprovals({
  snapshot,
  snapshotRef,
  panelView,
  selectedAgentRef,
  menuOpenRef,
  navigationSeqRef,
  applySnapshot,
  collapseIsland,
  scheduleIdleCollapse,
  setSessionRequests,
}: UseApprovalsOptions) {
  const [busyDecision, setBusyDecision] = useState<Decision | null>(null);
  const busyRef = useRef<Decision | null>(null);
  busyRef.current = busyDecision;
  const [justResolved, setJustResolved] = useState(false);
  const prevPendingRef = useRef(0);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (busyRef.current) return;
      if (menuOpenRef.current) return;
      if ((event.target as HTMLElement).tagName === "INPUT" || (event.target as HTMLElement).tagName === "TEXTAREA") return;

      const snapshot = snapshotRef.current;
      const agent = selectedAgentRef.current;
      const targetRequest = agent
        ? snapshot.recent.find(
            (request) => request.status === "pending" && request.agent === agent,
          ) ?? (snapshot.activeRequest?.agent === agent ? snapshot.activeRequest : null)
        : snapshot.activeRequest;
      if (!targetRequest) return;

      if (event.key === "Enter" && event.shiftKey) {
        event.preventDefault();
        resolveActive(targetRequest, "approved", true);
      } else if (event.key === "Enter") {
        event.preventDefault();
        resolveActive(targetRequest, "approved");
      } else if (event.key === "Backspace" || event.key === "Delete") {
        event.preventDefault();
        resolveActive(targetRequest, "denied");
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Approval flash: when the pending queue drains, show the resolved tick.
  useEffect(() => {
    const prev = prevPendingRef.current;
    prevPendingRef.current = snapshot.pendingCount;
    if (prev > 0 && snapshot.pendingCount === 0) {
      setJustResolved(true);
      const timer = window.setTimeout(() => setJustResolved(false), 1300);
      return () => window.clearTimeout(timer);
    }
  }, [snapshot.pendingCount]);

  async function resolveActive(
    request: PermissionRequest | null,
    decision: Decision,
    alwaysApprove = false,
    note = "",
  ) {
    if (!request) return;

    setBusyDecision(decision);
    try {
      const resolveWork = (async () => {
        if (alwaysApprove) {
          await setSessionAutoApprove(request.session, true);
        }
        return resolvePermissionRequest(request.id, decision, note);
      })();
      // Hold the snapshot update until the resolve feedback animation can play.
      const [, nextSnapshot] = await Promise.all([
        new Promise<void>((resolve) => {
          window.setTimeout(resolve, RESOLVE_FEEDBACK_MS);
        }),
        resolveWork,
      ]);
      applySnapshot(nextSnapshot);
      if (nextSnapshot.pendingCount === 0) {
        collapseIsland(true);
        deactivateAtoll(request.agent, request.session, request.cwd).catch(
          () => undefined,
        );
      }
    } finally {
      setBusyDecision(null);
    }
  }

  async function resolveRequest(id: string, decision: Decision, note = "") {
    const resolvedRequest =
      snapshotRef.current.activeRequest?.id === id
        ? snapshotRef.current.activeRequest
        : snapshotRef.current.recent.find((item) => item.id === id);
    setBusyDecision(decision);
    try {
      const seq = ++navigationSeqRef.current;
      const nextSnapshot = await resolvePermissionRequest(id, decision, note);
      applySnapshot(nextSnapshot);
      if (panelView.kind === "session" && navigationSeqRef.current === seq) {
        const requests = await getSessionRequests(panelView.sessionId).catch(() => []);
        if (navigationSeqRef.current === seq) {
          setSessionRequests(requests);
        }
      }
      if (nextSnapshot.pendingCount === 0) {
        scheduleIdleCollapse();
        deactivateAtoll(
          resolvedRequest?.agent,
          resolvedRequest?.session,
          resolvedRequest?.cwd,
        ).catch(() => undefined);
      }
    } finally {
      setBusyDecision(null);
    }
  }

  return { busyDecision, justResolved, resolveActive, resolveRequest };
}
