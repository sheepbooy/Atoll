import { invoke } from "@tauri-apps/api/core";

import {
  localRequests,
  setLocalRequests,
} from "./localState";
import { isTauriRuntime } from "./runtime";
import { getSnapshot, normalizeSnapshot } from "./snapshot";
import type {
  IslandSnapshot,
  PermissionRequest,
} from "./types";
export async function resolvePermissionRequest(
  id: string,
  decision: "approved" | "denied",
  note = "",
): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("resolve_permission_request", { id, decision, note }),
    );
  }

  setLocalRequests(localRequests.map((request) =>
    request.id === id ? { ...request, status: decision } : request,
  ));

  return getSnapshot();
}

export async function resolvePermissionWithInput(
  id: string,
  decision: "approved" | "denied",
  note: string,
  updatedInput?: unknown,
): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("resolve_permission_with_input", {
        id,
        decision,
        note,
        updatedInput: updatedInput ?? null,
      }),
    );
  }

  setLocalRequests(localRequests.map((request) =>
    request.id === id ? { ...request, status: decision } : request,
  ));

  return getSnapshot();
}

export async function setSessionAutoApprove(session: string, enabled: boolean) {
  if (!isTauriRuntime()) {
    return;
  }

  return invoke<void>("set_session_auto_approve", { session, enabled });
}

export async function archiveRequest(id: string): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_request", { id }));
  }

  setLocalRequests(localRequests.map((request) =>
    request.id === id ? { ...request, archived: true } : request,
  ));
  return getSnapshot();
}

export async function archiveAllResolved(): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_all_resolved"));
  }

  setLocalRequests(localRequests.map((request) =>
    request.status !== "pending" ? { ...request, archived: true } : request,
  ));
  return getSnapshot();
}

export async function archiveSession(sessionId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_session", { sessionId }));
  }

  setLocalRequests(localRequests.filter((request) => request.session !== sessionId));
  return getSnapshot();
}

export async function archiveSubagent(agentId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_subagent", { agentId }));
  }
  return getSnapshot();
}

export async function archiveCompletedSubagents(sessionId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("archive_completed_subagents", { sessionId }),
    );
  }
  return getSnapshot();
}

export async function getSubagentRetention(): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>("get_subagent_retention");
  }
  return 600;
}

export async function setSubagentRetention(minutes: number): Promise<number> {
  if (isTauriRuntime()) {
    return invoke<number>("set_subagent_retention", { minutes });
  }
  return minutes * 60;
}

export async function pinSession(sessionId: string, pinned: boolean): Promise<IslandSnapshot> {
  if (isTauriRuntime()) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("pin_session", { sessionId, pinned }));
  }

  return getSnapshot();
}

export type ApprovalHistoryStatus =
  | "pending"
  | "approved"
  | "denied"
  | "expired"
  | "answered_elsewhere";

export interface ApprovalHistoryEntry {
  id: string;
  agent: string;
  sessionId: string;
  command: string;
  detail: string;
  cwd: string;
  toolInput?: unknown;
  transcriptPath?: string;
  requestedAt: number;
  decidedAt?: number;
  status: ApprovalHistoryStatus;
  host: string;
}

export interface ApprovalHistoryQuery {
  search?: string;
  agent?: string;
  status?: string;
  sessionId?: string;
  fromSecs?: number;
  toSecs?: number;
  limit?: number;
  offset?: number;
}

export interface ApprovalHistoryPage {
  items: ApprovalHistoryEntry[];
  total: number;
}

export async function getApprovalHistory(
  query: ApprovalHistoryQuery = {},
): Promise<ApprovalHistoryPage> {
  if (!isTauriRuntime()) {
    return { items: [], total: 0 };
  }
  return invoke<ApprovalHistoryPage>("get_approval_history", { query });
}

export async function exportApprovalHistory(
  query: ApprovalHistoryQuery = {},
  format: "json" | "csv",
): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }
  return invoke<string | null>("export_approval_history", { query, format });
}

export async function clearApprovalHistory(): Promise<void> {
  if (!isTauriRuntime()) {
    return;
  }
  return invoke<void>("clear_approval_history");
}
