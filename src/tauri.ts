import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PermissionStatus = "pending" | "approved" | "denied";
export type AgentKind = "claude" | "codex" | "gemini" | "other";

export interface TokenUsage {
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
}

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
  supportsAlways?: boolean;
}

export interface IslandSnapshot {
  online: boolean;
  pendingCount: number;
  archivedCount: number;
  activeRequest: PermissionRequest | null;
  recent: PermissionRequest[];
  sessions: SessionSummary[];
  dailyTokens: TokenUsage;
}

export interface SessionSummary {
  sessionId: string;
  agent: AgentKind;
  cwd: string;
  pendingCount: number;
  totalCount: number;
  lastActivity: string;
  transcriptPath: string | null;
  pinned?: boolean;
}

export interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
  toolName?: string | null;
}

export interface IslandHoverChanged {
  hovering: boolean;
  clientX?: number;
  clientY?: number;
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
    sessions: [],
    dailyTokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    },
  };
}

export async function getSessionRequests(sessionId: string): Promise<PermissionRequest[]> {
  if (isTauriRuntime) {
    return invoke<PermissionRequest[]>("get_session_requests", { sessionId });
  }

  return localRequests.filter((request) => request.session === sessionId);
}

export async function getSessionTranscript(transcriptPath: string): Promise<ChatMessage[]> {
  if (isTauriRuntime) {
    return invoke<ChatMessage[]>("get_session_transcript", { transcriptPath });
  }

  return [];
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

export async function archiveSession(sessionId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("archive_session", { sessionId });
  }

  localRequests = localRequests.map((request) =>
    request.session === sessionId ? { ...request, archived: true } : request,
  );
  return getSnapshot();
}

export async function pinSession(sessionId: string, pinned: boolean): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return invoke<IslandSnapshot>("pin_session", { sessionId, pinned });
  }

  return getSnapshot();
}

export interface HookStatus {
  installed: boolean;
  scriptFound: boolean;
  settingsPath: string;
  scriptPath: string;
}

export async function getClaudeHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return invoke<HookStatus>("get_claude_hook_status");
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return invoke<HookStatus>("install_claude_hooks");
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return invoke<HookStatus>("uninstall_claude_hooks");
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function quitAtoll() {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("quit_atoll");
}

export async function deactivateAtoll() {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("deactivate_atoll");
}

export async function setIslandPresentation(
  mode: "compact" | "expanded" | "dormant",
  compactWidth?: number,
  expandedIdle?: boolean,
  compactLeftWidth?: number,
) {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_island_presentation", {
    mode,
    compactWidth,
    compactLeftWidth,
    expandedIdle,
  });
}

export interface NotchMetrics {
  hasNotch: boolean;
  width: number;
  height: number;
  leftAreaWidth?: number;
  rightAreaWidth?: number;
}

export async function getNotchMetrics(): Promise<NotchMetrics> {
  if (isTauriRuntime) {
    return invoke<NotchMetrics>("get_notch_metrics");
  }

  return { hasNotch: false, width: 0, height: 0 };
}

export async function getSessionRetention(): Promise<number> {
  if (isTauriRuntime) {
    return invoke<number>("get_session_retention");
  }
  return 900;
}

export async function setSessionRetention(minutes: number): Promise<number> {
  if (isTauriRuntime) {
    return invoke<number>("set_session_retention", { minutes });
  }
  return minutes * 60;
}

export async function openInTerminal(cwd: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("open_in_terminal", { cwd });
}

export async function openUrl(url: string): Promise<void> {
  if (!isTauriRuntime) {
    window.open(url, "_blank");
    return;
  }

  return invoke<void>("open_url", { url });
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
