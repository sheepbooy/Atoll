// Salary-mode math and formatting. Earnings are a pure function of the local
// wall clock (seconds since local midnight × per-second rate), so the counter
// needs no persistence and rolls over to zero automatically at midnight.
import { resolveIntlLocale } from "./i18n";
import type { SalaryCurrency, SalarySettings } from "./salarySettings";

export function salaryPerSecond(settings: SalarySettings): number {
  const secondsPerMonth =
    settings.workDaysPerMonth * settings.workHoursPerDay * 3600;
  if (secondsPerMonth <= 0) return 0;
  return settings.monthlyWage / secondsPerMonth;
}

export function localDateKey(now: Date): string {
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function secondsSinceLocalMidnight(now: Date): number {
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  return Math.max(0, (now.getTime() - start.getTime()) / 1000);
}

export function salaryEarnedToday(settings: SalarySettings, now: Date): number {
  return secondsSinceLocalMidnight(now) * salaryPerSecond(settings);
}

export function formatSalaryEarnings(
  value: number,
  currency: SalaryCurrency,
  compact = 0,
  formatHint = value,
): string {
  const abs = Math.abs(value);
  const hintAbs = Math.abs(formatHint);
  const sign = value < 0 ? "-" : "";

  // Abbreviated forms only when compact pressure is high and the amount is large.
  if (compact >= 1 && hintAbs >= 1_000_000) {
    return `${sign}${currency}${(abs / 1_000_000).toFixed(compact >= 2 ? 0 : 2)}M`;
  }
  if (compact >= 1 && hintAbs >= 100_000) {
    return `${sign}${currency}${Math.round(abs / 1000)}K`;
  }
  const fractionDigits =
    compact >= 2 ? 0 : compact >= 1 && hintAbs >= 1000 ? 1 : 2;
  return `${sign}${currency}${abs.toFixed(fractionDigits)}`;
}

export function formatSalaryRate(rate: number, currency: SalaryCurrency): string {
  return `${currency}${rate.toFixed(4)}`;
}

export function formatSalaryWage(wage: number, currency: SalaryCurrency): string {
  return `${currency}${Math.round(wage).toLocaleString(resolveIntlLocale())}`;
}
