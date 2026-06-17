import { describe, expect, it } from "vitest";
import {
  buildDigitReelStrip,
  buildTokenOdometerCells,
  formatCompactTokenCount,
  tokenCompactLevel,
  tokenDisplayCompactLevel,
  tokenDisplayFormatsCompatible,
} from "./tokenCounterFormat";

describe("tokenCounter", () => {
  it("rolls digits through a slot strip", () => {
    expect(buildDigitReelStrip("8", "2")).toEqual(["8", "9", "0", "1", "2"]);
  });

  it("aligns digit rolls from the right when commas are inserted", () => {
    const cells = buildTokenOdometerCells("1,000", "999");
    const digits = cells.filter((cell) => cell.kind === "digit");
    expect(digits.map((cell) => cell.char).join("")).toBe("1000");
    expect(digits.filter((cell) => cell.changed).length).toBeGreaterThan(0);
    expect(digits[digits.length - 1]?.prevChar).toBe("9");
    expect(digits[digits.length - 1]?.char).toBe("0");
  });

  it("skips roll animation when display format changes abruptly", () => {
    expect(tokenDisplayFormatsCompatible("1.2K", "1,234")).toBe(false);
    const cells = buildTokenOdometerCells("1.2K", "1,234");
    expect(cells.every((cell) => !cell.changed)).toBe(true);
  });

  it("keeps full locale formatting longer under mild compact pressure", () => {
    expect(tokenCompactLevel(3, 4)).toBe(0);
    expect(formatCompactTokenCount(12_345, 1, 12_345)).toBe("12,345");
    expect(formatCompactTokenCount(850, 1, 900)).toBe("850");
  });

  it("abbreviates very large counts for expanded display", () => {
    expect(tokenDisplayCompactLevel(850_000, "expanded", 0, 4)).toBe(0);
    expect(tokenDisplayCompactLevel(2_500_000, "expanded", 0, 4)).toBe(1);
    expect(formatCompactTokenCount(2_500_000, 1, 2_500_000)).toBe("2.5M");
    expect(tokenDisplayCompactLevel(3_400_000_000, "expanded", 0, 4)).toBe(2);
    expect(formatCompactTokenCount(3_400_000_000, 2, 3_400_000_000)).toBe("3B");
  });

  it("supports billion suffix in compact formatting", () => {
    expect(tokenDisplayFormatsCompatible("3.4B", "3.3B")).toBe(true);
    expect(formatCompactTokenCount(1_250_000_000, 1, 1_250_000_000)).toBe("1.3B");
  });

  it("marks newly carried digits as entering", () => {
    const cells = buildTokenOdometerCells("1,000", "999");
    expect(cells[0]?.entering).toBe(true);
    expect(cells[0]?.char).toBe("1");
  });

  it("staggers roll delays from the right", () => {
    const cells = buildTokenOdometerCells("1,234", "1,229");
    const rolling = cells.filter((cell) => cell.changed);
    expect(rolling.length).toBeGreaterThan(0);
    const delays = rolling.map((cell) => cell.rollDelayMs);
    expect(delays.some((delay) => delay > 0)).toBe(true);
  });
});
