import { useEffect, useRef, useState } from "react";
import {
  createApprovalRuleFromRequest,
  deactivateAtoll,
  getSessionRequests,
  resolvePermissionRequest,
  setSessionAutoApprove,
  type ApprovalRuleScope,
  type IslandSnapshot,
  type PermissionRequest,
} from "../tauri";
import { RESOLVE_FEEDBACK_MS } from "../islandPresentation";
import { waitForMotion } from "../animationTiming";
import type { AgentKind, Decision, PanelView } from "../appTypes";
import type { ApprovalMascotFeedback } from "../mascotMotion";

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
  const [mascotFeedback, setMascotFeedback] = useState<ApprovalMascotFeedback | null>(null);
  const feedbackSeq = useRef(0);
  const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const showFeedback = (request: PermissionRequest, decision: Decision) => {
    if (mounted.current) setMascotFeedback({ eventId: `${request.id}:${++feedbackSeq.current}`, requestId: request.id, agent: request.agent, sessionId: request.session, decision });
  };
  const panelRef = useRef(panelView); panelRef.current = panelView;

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (panelRef.current.kind === "reminders" || event.isComposing || (event.target as HTMLElement)?.closest?.(".reminder-alert")) return;
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

  // Feedback belongs to a confirmed decision and has its own expiry. A new
  // request must never cancel the expiry and leave unrelated avatars happy.
  useEffect(() => {
    if (!mascotFeedback) return;
    const timer = window.setTimeout(() => setMascotFeedback(null), 1300);
    return () => window.clearTimeout(timer);
  }, [mascotFeedback]);
  useEffect(() => {
    if (!mascotFeedback) return;
    const requests = snapshot.activeRequest ? [snapshot.activeRequest, ...snapshot.recent] : snapshot.recent;
    if (requests.some(request => request.agent === mascotFeedback.agent && request.session === mascotFeedback.sessionId && request.id !== mascotFeedback.requestId && request.status === "pending")) {
      setMascotFeedback(null);
    }
  }, [snapshot.activeRequest, snapshot.recent, mascotFeedback]);

  async function resolveActive(
    request: PermissionRequest | null,
    decision: Decision,
    alwaysApprove = false,
    note = "",
    ruleScope?: ApprovalRuleScope,
  ) {
    if (!request) return;

    setBusyDecision(decision);
    try {
      const resolveWork = (async () => {
        if (ruleScope) {
          // A failed rule write must not block the approval itself.
          await createApprovalRuleFromRequest(request.id, ruleScope, decision).catch(
            () => undefined,
          );
        }
        if (alwaysApprove) {
          await setSessionAutoApprove(request.session, true);
        }
        return resolvePermissionRequest(request.id, decision, note);
      })();
      // Hold the snapshot update until the resolve feedback animation can play.
      const [, nextSnapshot] = await Promise.all([
        waitForMotion(RESOLVE_FEEDBACK_MS),
        resolveWork,
      ]);
      applySnapshot(nextSnapshot);
      showFeedback(request, decision);
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
      if (resolvedRequest) showFeedback(resolvedRequest, decision);
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

  return { busyDecision, justResolved: mascotFeedback !== null, mascotFeedback, resolveActive, resolveRequest };
}
