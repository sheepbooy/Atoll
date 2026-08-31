import {
  EMPTY_HOOK_HEALTH,
  type IslandSnapshot,
  type TokenUsage,
  type NotchMetrics,
} from "./tauri";

export const ZERO_TOKEN_USAGE: TokenUsage = {
  inputTokens: 0,
  outputTokens: 0,
  cacheReadTokens: 0,
  cacheCreationTokens: 0,
};
export const EMPTY_NOTCH_METRICS: NotchMetrics = {
  hasNotch: false,
  width: 0,
  height: 0,
};

export const initialSnapshot: IslandSnapshot = {
  online: false,
  pendingCount: 0,
  archivedCount: 0,
  activeRequest: null,
  recent: [],
  sessions: [],
  dailyTokens: ZERO_TOKEN_USAGE,
  activeSessionTokens: ZERO_TOKEN_USAGE,
  hookHealth: EMPTY_HOOK_HEALTH,
};
