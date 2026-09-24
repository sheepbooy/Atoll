import { invoke } from "@tauri-apps/api/core";

import { isTauriRuntime } from "./runtime";
import { localDateKey } from "../salaryFormat";

export interface SalaryHistoryDay {
  date: string;
  amount: number;
}

export interface SalaryHistoryResponse {
  timezone: string;
  days: SalaryHistoryDay[];
}

export async function recordSalaryDay(date: string, amount: number): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await invoke("record_salary_day", { date, amount });
  } catch (error) {
    console.error("[Atoll] record_salary_day failed", error);
  }
}

export async function getSalaryHistory(days: number): Promise<SalaryHistoryResponse> {
  if (isTauriRuntime()) {
    return invoke<SalaryHistoryResponse>("get_salary_history", { days });
  }

  return {
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    days: [{ date: localDateKey(new Date()), amount: 0 }],
  };
}
