export type PermissionStatus = "pending" | "approved" | "denied";
export type AgentKind =
  | "claude"
  | "codex"
  | "cursor"
  | "zcode"
  | "gemini"
  | "opencode"
  | "other";

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
  /** Raw tool name from the hook payload ("Bash", "WebFetch", ...). */
  toolName?: string;
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

export type ApprovalRuleDecision = "allow" | "deny";

/** A persistent auto-approve/deny rule (~/.atoll/rules.json). Matchers are
 * ANDed; unset matchers match anything. `pattern`/`tool` are globs
 * (`*` / `?`, case-insensitive); `projectPath` scopes the rule to a cwd. */
export interface ApprovalRule {
  id: string;
  name: string;
  enabled: boolean;
  decision: ApprovalRuleDecision;
  note: string;
  /** Agent kind key; undefined = any agent. */
  agent?: string;
  tool?: string;
  pattern?: string;
  projectPath?: string;
  createdAt: number;
  matchCount: number;
  lastMatchedAt?: number | null;
}

/** Quick-pick scopes for rules created from an approval card. */
export type ApprovalRuleScope = "command_project" | "command_global" | "all_project";

export interface IslandSnapshot {
  online: boolean;
  pendingCount: number;
  archivedCount: number;
  activeRequest: PermissionRequest | null;
  recent: PermissionRequest[];
  sessions: SessionSummary[];
  dailyTokens: TokenUsage;
  activeSessionTokens: TokenUsage;
  dailyTokensByModel?: Record<string, TokenUsage>;
  activeSessionTokensByModel?: Record<string, TokenUsage>;
  hookHealth: HookHealthSnapshot;
}

export type SessionHost =
  | "unknown"
  | "claudeDesktop"
  | "claudeCli"
  | "codexDesktop"
  | "codexCli"
  | "cursorIde"
  | "zcodeDesktop"
  | "zcodeCli";

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
  toolOutput?: string | null;
}

export interface IslandHoverChanged {
  hovering: boolean;
  cursorOverWindow: boolean;
  clientX?: number;
  clientY?: number;
}

export interface CompetingHook {
  event: string;
  command: string;
  binaryExists: boolean;
}

export interface HookStatus {
  installed: boolean;
  scriptFound: boolean;
  settingsPath: string;
  scriptPath: string;
  nodePath?: string;
  nodeFound?: boolean;
  /** Hook script content changed since this agent last trusted it (e.g. an Atoll
   * update overwrote the script in place). The agent may be silently ignoring the
   * hook until the user re-confirms trust for it. */
  needsRetrust?: boolean;
  /** Non-Atoll hooks registered for Claude events. Empty for codex/cursor. Dead
   * competitor hooks (binary missing or app not running) can veto Atoll's
   * permission decisions under Claude Code's most-restrictive-wins merge. */
  competingHooks?: CompetingHook[];
}

export interface HookHealthSnapshot {
  claude: HookStatus;
  codex: HookStatus;
  cursor: HookStatus;
  zcode: HookStatus;
  gemini: HookStatus;
  opencode: HookStatus;
}

export const EMPTY_HOOK_HEALTH: HookHealthSnapshot = {
  claude: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
  codex: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
  cursor: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
  zcode: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
  gemini: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
  opencode: {
    installed: false,
    scriptFound: false,
    settingsPath: "",
    scriptPath: "",
    nodePath: "",
    nodeFound: true,
    needsRetrust: false,
  },
};

