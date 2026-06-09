import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PermissionStatus = "pending" | "approved" | "denied";
export type AgentKind = "claude" | "codex" | "gemini" | "other";

export interface PermissionRequest {
  id: string;
  agent: AgentKind;
  session: string;
  command: string;
  detail: string;
  cwd: string;
  requestedAt: string;
  status: PermissionStatus;
}

export interface IslandSnapshot {
  online: boolean;
  pendingCount: number;
  activeRequest: PermissionRequest | null;
  recent: PermissionRequest[];
}

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

const fallbackRequests: PermissionRequest[] = [
  {
    id: "demo-claude-shell",
    agent: "claude",
    session: "marketing-site-fix",
    command: "Bash: npm install",
    detail: "Claude wants to install packages for the local project.",
    cwd: "/Users/yangshuai/Documents/Atoll",
    requestedAt: new Date(Date.now() - 1000 * 75).toISOString(),
    status: "pending",
  },
  {
    id: "demo-codex-write",
    agent: "codex",
    session: "agent-permission-bridge",
    command: "Edit: src-tauri/src/main.rs",
    detail: "Codex is waiting before changing the Rust event adapter.",
    cwd: "/Users/yangshuai/Documents/Atoll",
    requestedAt: new Date(Date.now() - 1000 * 240).toISOString(),
    status: "pending",
  },
];

let localRequests = [...fallbackRequests];

export async function getSnapshot(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("get_snapshot");
  }

  return {
    online: true,
    pendingCount: localRequests.filter((request) => request.status === "pending").length,
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

export async function simulatePermissionRequest(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("simulate_permission_request");
  }

  const createdAt = new Date();
  localRequests = [
    {
      id: `demo-${createdAt.getTime()}`,
      agent: "claude",
      session: "local-demo",
      command: "Bash: git status --short",
      detail: "A local demo request is waiting for confirmation.",
      cwd: "/Users/yangshuai/Documents/Atoll",
      requestedAt: createdAt.toISOString(),
      status: "pending",
    },
    ...localRequests,
  ];

  return getSnapshot();
}

export async function onSnapshotChanged(callback: (snapshot: IslandSnapshot) => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<IslandSnapshot>("snapshot-changed", (event) => callback(event.payload));
}
