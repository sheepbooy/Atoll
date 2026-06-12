import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PermissionStatus = "pending" | "approved" | "denied";
export type AgentKind = "claude" | "codex" | "gemini" | "other";

export interface PermissionRequest {
  id: string;
  toolUseId?: string | null;
  agent: AgentKind;
  session: string;
  command: string;
  detail: string;
  cwd: string;
  requestedAt: string;
  status: PermissionStatus;
  archived?: boolean;
}

export interface IslandSnapshot {
  online: boolean;
  pendingCount: number;
  archivedCount: number;
  activeRequest: PermissionRequest | null;
  recent: PermissionRequest[];
}

export interface IslandHoverChanged {
  hovering: boolean;
}

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

let localRequests: PermissionRequest[] = [];

export async function getSnapshot(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("get_snapshot");
  }

  return {
    online: true,
    pendingCount: localRequests.filter((request) => request.status === "pending").length,
    archivedCount: localRequests.filter((request) => request.archived).length,
    activeRequest: localRequests.find((request) => request.status === "pending") ?? null,
    recent: localRequests,
  };
}

export async function resolvePermissionRequest(
  id: string,
  decision: "approved" | "denied",
  note = "",
): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("resolve_permission_request", { id, decision, note });
  }

  localRequests = localRequests.map((request) =>
    request.id === id ? { ...request, status: decision } : request,
  );

  return getSnapshot();
}

export async function setSessionAutoApprove(session: string, enabled: boolean) {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_session_auto_approve", { session, enabled });
}

export async function archiveRequest(id: string): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("archive_request", { id });
  }

  localRequests = localRequests.map((request) =>
    request.id === id ? { ...request, archived: true } : request,
  );
  return getSnapshot();
}

export async function archiveAllResolved(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("archive_all_resolved");
  }

  localRequests = localRequests.map((request) =>
    request.status !== "pending" ? { ...request, archived: true } : request,
  );
  return getSnapshot();
}

export async function quitAtoll() {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("quit_atoll");
}

export async function setIslandPresentation(mode: "compact" | "expanded") {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_island_presentation", { mode });
}

export async function onSnapshotChanged(callback: (snapshot: IslandSnapshot) => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<IslandSnapshot>("snapshot-changed", (event) => callback(event.payload));
}

export async function onIslandHoverChanged(callback: (state: IslandHoverChanged) => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<IslandHoverChanged>("island-hover-changed", (event) => callback(event.payload));
}

export async function onIslandOpenRequested(callback: () => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<void>("island-open-requested", () => callback());
}
