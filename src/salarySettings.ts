// Salary-counter settings for the "salary" display mode: monthly wage, work
// volume and currency. Persisted in localStorage alongside the other display
// preferences; the Rust side only stores the derived daily earnings history.
import { readStoredSetting } from "./settingsStorage";

export type SalaryCurrency = "¥" | "$" | "€";

export interface SalarySettings {
  monthlyWage: number;
  workDaysPerMonth: number;
  workHoursPerDay: number;
  currency: SalaryCurrency;
}

export const SALARY_MONTHLY_WAGE_KEY = "atoll.salary.monthlyWage";
export const SALARY_WORK_DAYS_KEY = "atoll.salary.workDaysPerMonth";
export const SALARY_WORK_HOURS_KEY = "atoll.salary.workHoursPerDay";
export const SALARY_CURRENCY_KEY = "atoll.salary.currency";

export const DEFAULT_SALARY_MONTHLY_WAGE = 20000;
export const DEFAULT_SALARY_WORK_DAYS = 22;
export const DEFAULT_SALARY_WORK_HOURS = 8;
export const DEFAULT_SALARY_CURRENCY: SalaryCurrency = "¥";

export const DEFAULT_SALARY_SETTINGS: SalarySettings = {
  monthlyWage: DEFAULT_SALARY_MONTHLY_WAGE,
  workDaysPerMonth: DEFAULT_SALARY_WORK_DAYS,
  workHoursPerDay: DEFAULT_SALARY_WORK_HOURS,
  currency: DEFAULT_SALARY_CURRENCY,
};

export const MIN_SALARY_MONTHLY_WAGE = 0;
export const MAX_SALARY_MONTHLY_WAGE = 10_000_000;

export function clampMonthlyWage(value: number) {
  return Math.min(MAX_SALARY_MONTHLY_WAGE, Math.max(MIN_SALARY_MONTHLY_WAGE, value));
}

export function clampWorkDays(value: number) {
  return Math.min(31, Math.max(1, Math.round(value)));
}

export function clampWorkHours(value: number) {
  return Math.min(24, Math.max(1, value));
}

export function readSalarySettings(): SalarySettings {
  return {
    monthlyWage: readStoredSetting(
      SALARY_MONTHLY_WAGE_KEY,
      DEFAULT_SALARY_MONTHLY_WAGE,
      clampMonthlyWage,
    ),
    workDaysPerMonth: readStoredSetting(
      SALARY_WORK_DAYS_KEY,
      DEFAULT_SALARY_WORK_DAYS,
      clampWorkDays,
    ),
    workHoursPerDay: readStoredSetting(
      SALARY_WORK_HOURS_KEY,
      DEFAULT_SALARY_WORK_HOURS,
      clampWorkHours,
    ),
    currency: readSalaryCurrency(),
  };
}

export function writeSalarySettings(settings: SalarySettings) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(SALARY_MONTHLY_WAGE_KEY, String(settings.monthlyWage));
    window.localStorage.setItem(SALARY_WORK_DAYS_KEY, String(settings.workDaysPerMonth));
    window.localStorage.setItem(SALARY_WORK_HOURS_KEY, String(settings.workHoursPerDay));
    window.localStorage.setItem(SALARY_CURRENCY_KEY, settings.currency);
  } catch {
    // ignore local storage errors
  }
}

function readSalaryCurrency(): SalaryCurrency {
  if (typeof window === "undefined") return DEFAULT_SALARY_CURRENCY;
  try {
    const stored = window.localStorage.getItem(SALARY_CURRENCY_KEY);
    if (stored === "¥" || stored === "$" || stored === "€") return stored;
  } catch {
    // ignore
  }
  return DEFAULT_SALARY_CURRENCY;
}
