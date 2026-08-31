import { invoke } from "@tauri-apps/api/core";

import {
  normalizeHookHealth,
} from "./hooks";
import {
  localRequests,
  setLocalRequests,
  setSnapshotInFlight,
  snapshotInFlight,
} from "./localState";
import { asRecord } from "./hooks";
import { EMPTY_HOOK_HEALTH } from "./types";
import { isTauriRuntime } from "./runtime";
import type {
  ChatMessage,
  IslandSnapshot,
  PermissionRequest,
} from "./types";
import { getDemoMode, getDemoSnapshot } from "../demoSnapshot";
export function normalizeSnapshot(raw: IslandSnapshot): IslandSnapshot {
  const record = asRecord(raw);
  const hookHealthRaw = record?.hookHealth ?? record?.hook_health;
  return {
    ...raw,
    hookHealth: hookHealthRaw ? normalizeHookHealth(hookHealthRaw) : EMPTY_HOOK_HEALTH,
  };
}

export async function getSnapshot(): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    let inFlight = snapshotInFlight;
    if (!inFlight) {
      inFlight = invoke<IslandSnapshot>("get_snapshot")
        .then(normalizeSnapshot)
        .finally(() => {
          setSnapshotInFlight(null);
        });
      setSnapshotInFlight(inFlight);
    }
    return inFlight;
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    const demo = getDemoSnapshot(demoMode);
    if (demo) return demo;
  }

  return {
    online: true,
    pendingCount: localRequests.filter((request) => request.status === "pending").length,
    archivedCount: localRequests.filter((request) => request.archived).length,
    activeRequest: localRequests.find((request) => request.status === "pending") ?? null,
    recent: [...localRequests],
    sessions: [],
    dailyTokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    },
    activeSessionTokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    },
    dailyTokensByModel: {},
    activeSessionTokensByModel: {},
    hookHealth: EMPTY_HOOK_HEALTH,
  };
}

export async function getSessionRequests(sessionId: string): Promise<PermissionRequest[]> {
  if (isTauriRuntime()) {
    return invoke<PermissionRequest[]>("get_session_requests", { sessionId });
  }

  return localRequests.filter((request) => request.session === sessionId);
}

export async function getSessionTranscript(transcriptPath: string): Promise<ChatMessage[]> {
  if (isTauriRuntime()) {
    return invoke<ChatMessage[]>("get_session_transcript", { transcriptPath });
  }

  return [];
}

export async function getSessionChat(sessionId: string): Promise<ChatMessage[]> {
  if (isTauriRuntime()) {
    return invoke<ChatMessage[]>("get_session_chat", { sessionId });
  }

  return [];
}

