import { afterEach, describe, expect, it } from "vitest";
import {
  formatSalaryEarnings,
  formatSalaryRate,
  formatSalaryWage,
  localDateKey,
  salaryEarnedToday,
  salaryPerSecond,
} from "./salaryFormat";
import {
  clampMonthlyWage,
  clampWorkDays,
  clampWorkHours,
  readSalarySettings,
  writeSalarySettings,
  DEFAULT_SALARY_SETTINGS,
  type SalarySettings,
} from "./salarySettings";
import { readDisplayMode, FOLDED_COUNTER_DISPLAY_KEY } from "./displayPrefs";

const SETTINGS: SalarySettings = {
  monthlyWage: 20000,
  workDaysPerMonth: 22,
  workHoursPerDay: 8,
  currency: "¥",
};

describe("salaryFormat", () => {
  it("computes the per-second rate from monthly wage and work volume", () => {
    // 20000 / (22 * 8 * 3600) = 20000 / 633600
    expect(salaryPerSecond(SETTINGS)).toBeCloseTo(0.0315657, 6);
  });

  it("returns zero rate for a zero wage", () => {
    expect(salaryPerSecond({ ...SETTINGS, monthlyWage: 0 })).toBe(0);
  });

  it("computes earnings from local midnight", () => {
    const noon = new Date(2026, 8, 24, 12, 0, 0);
    expect(salaryEarnedToday(SETTINGS, noon)).toBeCloseTo(
      12 * 3600 * salaryPerSecond(SETTINGS),
      4,
    );
  });

  it("rolls over to zero at midnight", () => {
    const justAfterMidnight = new Date(2026, 8, 25, 0, 0, 3);
    expect(salaryEarnedToday(SETTINGS, justAfterMidnight)).toBeCloseTo(
      3 * salaryPerSecond(SETTINGS),
      6,
    );
  });

  it("formats earnings with two decimals by default", () => {
    expect(formatSalaryEarnings(123.456, "¥")).toBe("¥123.46");
    expect(formatSalaryEarnings(0, "$")).toBe("$0.00");
  });

  it("reduces precision and abbreviates under compact pressure", () => {
    expect(formatSalaryEarnings(1234.56, "¥", 1)).toBe("¥1234.6");
    expect(formatSalaryEarnings(1234.56, "¥", 2)).toBe("¥1235");
    expect(formatSalaryEarnings(123_456, "€", 1, 123_456)).toBe("€123K");
    expect(formatSalaryEarnings(1_234_567, "$", 1, 1_234_567)).toBe("$1.23M");
    expect(formatSalaryEarnings(1_234_567, "$", 2, 1_234_567)).toBe("$1M");
  });

  it("formats the rate and wage for tooltips", () => {
    expect(formatSalaryRate(0.0315696, "¥")).toBe("¥0.0316");
    expect(formatSalaryWage(20000, "¥")).toBe("¥20,000");
  });

  it("formats a local date key", () => {
    expect(localDateKey(new Date(2026, 0, 5, 8, 30))).toBe("2026-01-05");
  });
});

describe("salarySettings", () => {
  afterEach(() => {
    window.localStorage.clear();
  });

  it("returns defaults when nothing is stored", () => {
    expect(readSalarySettings()).toEqual(DEFAULT_SALARY_SETTINGS);
  });

  it("clamps stored values into range", () => {
    window.localStorage.setItem("atoll.salary.monthlyWage", "99999999");
    window.localStorage.setItem("atoll.salary.workDaysPerMonth", "0");
    window.localStorage.setItem("atoll.salary.workHoursPerDay", "99");
    const settings = readSalarySettings();
    expect(settings.monthlyWage).toBe(10_000_000);
    expect(settings.workDaysPerMonth).toBe(1);
    expect(settings.workHoursPerDay).toBe(24);
  });

  it("round-trips written settings", () => {
    const next: SalarySettings = {
      monthlyWage: 35000,
      workDaysPerMonth: 20,
      workHoursPerDay: 7.5,
      currency: "$",
    };
    writeSalarySettings(next);
    expect(readSalarySettings()).toEqual(next);
  });

  it("clamps direct value edits", () => {
    expect(clampMonthlyWage(-5)).toBe(0);
    expect(clampWorkDays(2.7)).toBe(3);
    expect(clampWorkHours(0.5)).toBe(1);
    expect(clampWorkHours(30)).toBe(24);
  });

  it("reads the salary display mode from localStorage", () => {
    window.localStorage.setItem(FOLDED_COUNTER_DISPLAY_KEY, "salary");
    expect(readDisplayMode(FOLDED_COUNTER_DISPLAY_KEY)).toBe("salary");
    window.localStorage.setItem(FOLDED_COUNTER_DISPLAY_KEY, "nonsense");
    expect(readDisplayMode(FOLDED_COUNTER_DISPLAY_KEY)).toBe("tokens");
  });
});
