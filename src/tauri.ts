import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getDemoCodexHookStatus, getDemoHookStatus, getDemoMode, getDemoSnapshot } from "./demoSnapshot";

export type PermissionStatus = "pending" | "approved" | "denied";
export type AgentKind = "claude" | "codex" | "cursor" | "gemini" | "other";

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
  toolInput?: unknown;
}

export interface IslandSnapshot {
  online: boolean;
  pendingCount: number;
  archivedCount: number;
  activeRequest: PermissionRequest | null;
  recent: PermissionRequest[];
  sessions: SessionSummary[];
  dailyTokens: TokenUsage;
  activeSessionTokens: TokenUsage;
  hookHealth: HookHealthSnapshot;
}

export type SessionHost =
  | "unknown"
  | "claudeDesktop"
  | "claudeCli"
  | "codexDesktop"
  | "codexCli"
  | "cursorIde";

export interface SubagentSummary {
  agentId: string;
  agentType: string;
  startedAt: string;
  agentTranscriptPath?: string | null;
  completedAt?: string | null;
  archived?: boolean;
  lastMessage?: string | null;
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
  sessionHost?: SessionHost;
  activeSubagents?: SubagentSummary[];
}

export interface ChatMessage {
  role: "user" | "assistant" | "system";
  content: string;
  toolName?: string | null;
  toolInput?: unknown;
}

export interface IslandHoverChanged {
  hovering: boolean;
  cursorOverWindow: boolean;
  clientX?: number;
  clientY?: number;
}

const isTauriRuntime = "__TAURI_INTERNALS__" in window;

/** Matches `uses_micro_island` in src-tauri (Windows-only micro island). */
export function isWindowsTauriRuntime(): boolean {
  if (!("__TAURI_INTERNALS__" in window)) {
    return false;
  }
  return /Windows/i.test(navigator.userAgent);
}

export function usesMicroIslandSync(): boolean {
  return isWindowsTauriRuntime();
}

let localRequests: PermissionRequest[] = [];

export async function getSnapshot(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("get_snapshot"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoSnapshot(demoMode);
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
    activeSessionTokens: {
      inputTokens: 0,
      outputTokens: 0,
      cacheReadTokens: 0,
      cacheCreationTokens: 0,
    },
    hookHealth: EMPTY_HOOK_HEALTH,
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

export async function getSessionChat(sessionId: string): Promise<ChatMessage[]> {
  if (isTauriRuntime) {
    return invoke<ChatMessage[]>("get_session_chat", { sessionId });
  }

  return [];
}

export async function resolvePermissionRequest(
  id: string,
  decision: "approved" | "denied",
  note = "",
): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("resolve_permission_request", { id, decision, note }),
    );
  }

  localRequests = localRequests.map((request) =>
    request.id === id ? { ...request, status: decision } : request,
  );

  return getSnapshot();
}

export async function resolvePermissionWithInput(
  id: string,
  decision: "approved" | "denied",
  note: string,
  updatedInput?: unknown,
): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("resolve_permission_with_input", {
        id,
        decision,
        note,
        updatedInput: updatedInput ?? null,
      }),
    );
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
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_request", { id }));
  }

  localRequests = localRequests.map((request) =>
    request.id === id ? { ...request, archived: true } : request,
  );
  return getSnapshot();
}

export async function archiveAllResolved(): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_all_resolved"));
  }

  localRequests = localRequests.map((request) =>
    request.status !== "pending" ? { ...request, archived: true } : request,
  );
  return getSnapshot();
}

export async function archiveSession(sessionId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_session", { sessionId }));
  }

  localRequests = localRequests.filter((request) => request.session !== sessionId);
  return getSnapshot();
}

export async function archiveSubagent(agentId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("archive_subagent", { agentId }));
  }
  return getSnapshot();
}

export async function archiveCompletedSubagents(sessionId: string): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(
      await invoke<IslandSnapshot>("archive_completed_subagents", { sessionId }),
    );
  }
  return getSnapshot();
}

export async function getSubagentRetention(): Promise<number> {
  if (isTauriRuntime) {
    return invoke<number>("get_subagent_retention");
  }
  return 600;
}

export async function setSubagentRetention(minutes: number): Promise<number> {
  if (isTauriRuntime) {
    return invoke<number>("set_subagent_retention", { minutes });
  }
  return minutes * 60;
}

export async function pinSession(sessionId: string, pinned: boolean): Promise<IslandSnapshot> {
  if (isTauriRuntime) {
    return normalizeSnapshot(await invoke<IslandSnapshot>("pin_session", { sessionId, pinned }));
  }

  return getSnapshot();
}

export interface TokenHistoryDay {
  date: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  byAgent: Record<string, TokenUsage>;
}

export interface TokenHistoryResponse {
  timezone: string;
  days: TokenHistoryDay[];
}

export interface HookStatus {
  installed: boolean;
  scriptFound: boolean;
  settingsPath: string;
  scriptPath: string;
  nodePath?: string;
  nodeFound?: boolean;
}

export interface HookHealthSnapshot {
  claude: HookStatus;
  codex: HookStatus;
  cursor: HookStatus;
}

export const EMPTY_HOOK_HEALTH: HookHealthSnapshot = {
  claude: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
  },
  codex: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
  },
  cursor: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
  },
};

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as Record<string, unknown>;
}

function readBool(record: Record<string, unknown>, camel: string, snake: string): boolean {
  const value = record[camel] ?? record[snake];
  return Boolean(value);
}

function readString(record: Record<string, unknown>, camel: string, snake: string): string {
  const value = record[camel] ?? record[snake];
  return typeof value === "string" ? value : "";
}

export function normalizeHookStatus(raw: unknown): HookStatus {
  const record = asRecord(raw);
  if (!record) {
    return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
  }
  const nodeFoundRaw = record.nodeFound ?? record.node_found;
  return {
    installed: readBool(record, "installed", "installed"),
    scriptFound: readBool(record, "scriptFound", "script_found"),
    settingsPath: readString(record, "settingsPath", "settings_path"),
    scriptPath: readString(record, "scriptPath", "script_path"),
    nodePath: readString(record, "nodePath", "node_path"),
    nodeFound: nodeFoundRaw === undefined ? true : Boolean(nodeFoundRaw),
  };
}

export function normalizeHookHealth(raw: unknown): HookHealthSnapshot {
  const record = asRecord(raw);
  if (!record) {
    return EMPTY_HOOK_HEALTH;
  }
  return {
    claude: normalizeHookStatus(record.claude),
    codex: normalizeHookStatus(record.codex),
    cursor: normalizeHookStatus(record.cursor ?? EMPTY_HOOK_HEALTH.cursor),
  };
}

export function normalizeSnapshot(raw: IslandSnapshot): IslandSnapshot {
  const record = asRecord(raw);
  const hookHealthRaw = record?.hookHealth ?? record?.hook_health;
  return {
    ...raw,
    hookHealth: hookHealthRaw ? normalizeHookHealth(hookHealthRaw) : EMPTY_HOOK_HEALTH,
  };
}

export async function getClaudeHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("get_claude_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("install_claude_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallClaudeHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_claude_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getCodexHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("get_codex_hook_status"));
  }

  const demoMode = getDemoMode();
  if (demoMode) {
    return getDemoCodexHookStatus(demoMode);
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installCodexHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("install_codex_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallCodexHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_codex_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function getCursorHookStatus(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("get_cursor_hook_status"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function installCursorHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("install_cursor_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function uninstallCursorHooks(): Promise<HookStatus> {
  if (isTauriRuntime) {
    return normalizeHookStatus(await invoke<HookStatus>("uninstall_cursor_hooks"));
  }

  return { installed: false, scriptFound: false, settingsPath: "", scriptPath: "" };
}

export async function quitAtoll() {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("quit_atoll");
}

export async function deactivateAtoll(
  agent?: AgentKind,
  session?: string,
  cwd?: string,
) {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("deactivate_atoll", {
    agent: agent ?? null,
    session: session ?? null,
    cwd: cwd ?? null,
  });
}

export async function openAgentApp(
  agent: AgentKind,
  cwd: string,
  session?: string,
): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("open_agent_app", { agent, cwd, session: session ?? null });
}

export async function setIslandPresentation(
  mode: "micro" | "compact" | "expanded" | "dormant",
  compactWidth?: number,
  expandedIdle?: boolean,
  compactLeftWidth?: number,
  animate = true,
  snap = false,
) {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_island_presentation", {
    mode,
    compactWidth,
    compactLeftWidth,
    expandedIdle,
    animate,
    snap,
  });
}

export async function usesMicroIsland(): Promise<boolean> {
  if (!isTauriRuntime) {
    return false;
  }

  return invoke<boolean>("uses_micro_island");
}

/** Persist compact layout metrics without triggering a native window animation. */
export async function setCompactLayout(
  compactWidth: number,
  compactLeftWidth: number,
) {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_island_presentation", {
    mode: "compact",
    compactWidth,
    compactLeftWidth,
    animate: false,
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

  if (getDemoMode() === "compact" || getDemoMode() === "gif") {
    return { hasNotch: true, width: 180, height: 32, leftAreaWidth: 120, rightAreaWidth: 120 };
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

export async function getTokenHistory(days: number): Promise<TokenHistoryResponse> {
  if (isTauriRuntime) {
    return invoke<TokenHistoryResponse>("get_token_history", { days });
  }

  const todayKey = new Date();
  const year = todayKey.getFullYear();
  const month = String(todayKey.getMonth() + 1).padStart(2, "0");
  const day = String(todayKey.getDate()).padStart(2, "0");

  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    days: [
      {
        date: `${year}-${month}-${day}`,
        inputTokens: 0,
        outputTokens: 0,
        cacheReadTokens: 0,
        cacheCreationTokens: 0,
        byAgent: {},
      },
    ],
  };
}

export async function openInTerminal(cwd: string): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("open_in_terminal", { cwd });
}

export async function focusClaudeApp(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("focus_claude_app");
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

  return listen<IslandSnapshot>("snapshot-changed", (event) =>
    callback(normalizeSnapshot(event.payload)),
  );
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

export async function onCaptureCollapseRequested(callback: () => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<void>("capture-collapse", () => callback());
}

export async function onCaptureOpenHooksRequested(callback: () => void) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<void>("capture-open-hooks", () => callback());
}

export async function onCaptureScreenshotRequested(callback: () => void | Promise<void>) {
  if (!isTauriRuntime) {
    return () => undefined;
  }

  return listen<void>("capture-screenshot-requested", () => callback());
}

export async function captureProvideScreenshot(pngBase64: string) {
  if (!isTauriRuntime) {
    return;
  }

  await invoke("capture_provide_screenshot", { pngBase64 });
}

export async function isAutostartEnabled(): Promise<boolean> {
  if (!isTauriRuntime) {
    return false;
  }

  return invoke<boolean>("is_autostart_enabled");
}

export async function enableAutostart(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_autostart_enabled", { enabled: true });
}

export async function disableAutostart(): Promise<void> {
  if (!isTauriRuntime) {
    return;
  }

  return invoke<void>("set_autostart_enabled", { enabled: false });
}
