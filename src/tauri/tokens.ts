import { invoke } from "@tauri-apps/api/core";

import type { TokenUsage } from "./types";
import { isTauriRuntime } from "./runtime";
export interface TokenHistoryDay {
  date: string;
  inputTokens: number;
  outputTokens: number;
  cacheReadTokens: number;
  cacheCreationTokens: number;
  byAgent: Record<string, TokenUsage>;
  byModel?: Record<string, TokenUsage>;
}

export interface TokenHistoryResponse {
  timezone: string;
  days: TokenHistoryDay[];
}


export async function getTokenHistory(days: number): Promise<TokenHistoryResponse> {
  if (isTauriRuntime()) {
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
        byModel: {},
      },
    ],
  };
}
