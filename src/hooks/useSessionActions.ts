// Session-level actions from the island UI: quit, archive-all, archive/pin a
// session, and archive-completed-subagents. All adopt the backend snapshot
// they receive. Extracted from App.tsx; behavior unchanged.
import {
  archiveAllResolved,
  archiveCompletedSubagents,
  archiveSession,
  pinSession,
  quitAtoll,
  type IslandSnapshot,
} from "../tauri";
import type { PanelView } from "../appTypes";

interface UseSessionActionsOptions {
  setMenuOpen: (open: boolean) => void;
  applySnapshot: (snapshot: IslandSnapshot, options?: { mergeHookHealth?: boolean }) => void;
  panelViewRef: { current: PanelView };
  navigationSeqRef: { current: number };
  setPanelView: (view: PanelView) => void;
}

export function useSessionActions({
  setMenuOpen,
  applySnapshot,
  panelViewRef,
  navigationSeqRef,
  setPanelView,
}: UseSessionActionsOptions) {
  async function handleQuit() {
    setMenuOpen(false);
    await quitAtoll().catch(() => undefined);
  }

  async function handleArchiveAll() {
    setMenuOpen(false);
    const nextSnapshot = await archiveAllResolved().catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handleArchiveSession(sessionId: string) {
    const nextSnapshot = await archiveSession(sessionId).catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handlePinSession(sessionId: string, pinned: boolean) {
    const nextSnapshot = await pinSession(sessionId, pinned).catch(() => null);
    if (nextSnapshot) {
      applySnapshot(nextSnapshot);
    }
  }

  async function handleArchiveCompletedSubagents(sessionId: string) {
    const nextSnapshot = await archiveCompletedSubagents(sessionId).catch(() => null);
    if (!nextSnapshot) {
      return;
    }
    applySnapshot(nextSnapshot);
    const currentView = panelViewRef.current;
    if (
      currentView.kind === "subagent"
      && currentView.sessionId === sessionId
      && !nextSnapshot.sessions
        .find((session) => session.sessionId === sessionId)
        ?.activeSubagents?.some((sub) => sub.agentId === currentView.agentId)
    ) {
      ++navigationSeqRef.current;
      setPanelView({ kind: "home" });
    }
  }

  return {
    handleQuit,
    handleArchiveAll,
    handleArchiveSession,
    handlePinSession,
    handleArchiveCompletedSubagents,
  };
}
