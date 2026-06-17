export function formatCompactTokenCount(
  value: number,
  compact = 0,
  formatHint = value,
): string {
  const abs = Math.abs(value);
  const hintAbs = Math.abs(formatHint);
  const sign = value < 0 ? "-" : "";

  if (compact === 0) {
    return value.toLocaleString();
  }

  if (hintAbs < 1_000) {
    return value.toLocaleString();
  }

  if (hintAbs >= 1_000_000_000) {
    const fractionDigits = compact >= 2 ? 0 : 1;
    return `${sign}${(abs / 1_000_000_000).toFixed(fractionDigits)}B`;
  }

  if (hintAbs >= 1_000_000) {
    const fractionDigits = compact >= 2 ? 0 : 1;
    return `${sign}${(abs / 1_000_000).toFixed(fractionDigits)}M`;
  }

  if (hintAbs >= 1_000) {
    if (compact === 1 && hintAbs < 100_000) {
      return value.toLocaleString();
    }
    const fractionDigits = compact >= 2 ? 0 : 1;
    return `${sign}${(abs / 1_000).toFixed(fractionDigits)}K`;
  }

  return value.toLocaleString();
}

/** Session pressure for collapsed slot display. */
export function tokenCompactLevel(sessionCount: number, maxCompactIcons: number): number {
  const visibleIcons = Math.min(sessionCount, maxCompactIcons);
  const iconPressure = maxCompactIcons > 0 ? visibleIcons / maxCompactIcons : 0;
  if (iconPressure < 0.92) return 0;
  if (sessionCount <= maxCompactIcons + 2) return 1;
  return 2;
}

/**
 * Pick formatting tier from token magnitude (and optional display width budget).
 * Expanded mode prefers full digits until ~1M, then K/M/B abbreviations.
 */
export function tokenDisplayCompactLevel(
  value: number,
  variant: "compact" | "expanded",
  sessionCount: number,
  maxCompactIcons: number,
  maxDisplayChars = variant === "expanded" ? 11 : 999,
): number {
  if (variant === "compact") {
    const sessionLevel = tokenCompactLevel(sessionCount, maxCompactIcons);
    const fullText = formatCompactTokenCount(value, 0, value);
    if (fullText.length <= maxDisplayChars) return sessionLevel;
    return Math.max(sessionLevel, fullText.length > 13 ? 2 : 1);
  }

  const abs = Math.abs(value);
  const fullText = formatCompactTokenCount(value, 0, value);
  if (fullText.length <= maxDisplayChars && abs < 1_000_000) return 0;
  if (abs >= 1_000_000_000 || fullText.length > 14) return 2;
  if (abs >= 1_000_000 || fullText.length > maxDisplayChars) return 1;
  return 0;
}

const TOKEN_ODO_FONT_SIZE = 12;
const TOKEN_ODO_CHAR_EM = {
  digit: 0.64,
  sep: 0.36,
  other: 0.48,
} as const;

function tokenOdoCharWidth(char: string): number {
  if (char >= "0" && char <= "9") {
    return TOKEN_ODO_CHAR_EM.digit * TOKEN_ODO_FONT_SIZE;
  }
  if (char === "," || char === "." || char === "\u202f" || char === "\u00a0") {
    return TOKEN_ODO_CHAR_EM.sep * TOKEN_ODO_FONT_SIZE;
  }
  return TOKEN_ODO_CHAR_EM.other * TOKEN_ODO_FONT_SIZE;
}

export function estimateTokenDisplayWidth(text: string): number {
  return Math.ceil(
    [...text].reduce((width, char) => width + tokenOdoCharWidth(char), 0),
  );
}

export function digitReelSteps(fromChar: string, toChar: string): number {
  const from = Number(fromChar);
  const to = Number(toChar);
  if (!Number.isInteger(from) || !Number.isInteger(to)) return 0;
  const steps = (to - from + 10) % 10;
  return steps === 0 ? 10 : steps;
}

export function buildDigitReelStrip(fromChar: string, toChar: string): string[] {
  const from = Number(fromChar);
  const steps = digitReelSteps(fromChar, toChar);
  return Array.from({ length: steps + 1 }, (_, index) => String((from + index) % 10));
}

export function isNumericTokenChar(char: string): boolean {
  return char >= "0" && char <= "9";
}

export function isTokenSeparator(char: string): boolean {
  return char === "," || char === "." || char === "\u202f" || char === "\u00a0";
}

export function tokenDisplayFormatsCompatible(next: string, prev: string): boolean {
  const suffix = (text: string) => text.match(/[KMB]$/)?.[0] ?? "";
  if (suffix(next) !== suffix(prev)) return false;
  const separators = (text: string) =>
    [...text].filter((char) => isTokenSeparator(char)).length;
  return Math.abs(separators(next) - separators(prev)) <= 1;
}

export interface TokenOdometerCell {
  char: string;
  prevChar: string | null;
  kind: "digit" | "sep" | "other";
  changed: boolean;
  entering: boolean;
  rollDelayMs: number;
}

export function buildTokenOdometerCells(next: string, prev: string): TokenOdometerCell[] {
  if (!tokenDisplayFormatsCompatible(next, prev)) {
    return [...next].map((char) => ({
      char,
      prevChar: char,
      kind: isNumericTokenChar(char)
        ? "digit"
        : isTokenSeparator(char)
          ? "sep"
          : "other",
      changed: false,
      entering: false,
      rollDelayMs: 0,
    }));
  }

  const nextChars = [...next];
  const prevDigits = [...prev].filter(isNumericTokenChar);
  const nextDigitIndices: number[] = [];
  nextChars.forEach((char, index) => {
    if (isNumericTokenChar(char)) nextDigitIndices.push(index);
  });

  const prevDigitByNextIndex = new Map<number, string>();
  const digitOffset = prevDigits.length - nextDigitIndices.length;
  nextDigitIndices.forEach((index, digitIndex) => {
    const prevDigitIndex = digitOffset + digitIndex;
    if (prevDigitIndex >= 0 && prevDigitIndex < prevDigits.length) {
      prevDigitByNextIndex.set(index, prevDigits[prevDigitIndex]!);
    }
  });

  const cells = nextChars.map((char, index) => {
    const kind = isNumericTokenChar(char)
      ? "digit"
      : isTokenSeparator(char)
        ? "sep"
        : "other";

    if (kind === "digit") {
      const prevChar = prevDigitByNextIndex.get(index) ?? null;
      const entering = prevChar === null && prev.length > 0;
      const changed = prevChar !== null && prevChar !== char;
      return {
        char,
        prevChar,
        kind,
        changed,
        entering,
        rollDelayMs: 0,
      };
    }

    return {
      char,
      prevChar: null,
      kind,
      changed: false,
      entering: false,
      rollDelayMs: 0,
    };
  });

  nextDigitIndices.forEach((index, digitIndex) => {
    const cell = cells[index];
    if (!cell || cell.kind !== "digit") return;
    if (!cell.changed && !cell.entering) return;
    const fromRight = nextDigitIndices.length - 1 - digitIndex;
    cell.rollDelayMs = fromRight * 22;
  });

  return cells;
}

export function stepAnimatedTokenValue(current: number, target: number, dt: number): number {
  const diff = target - current;
  const distance = Math.abs(diff);
  if (distance < 0.5) return target;
  const tau = Math.min(0.5, 0.12 + Math.log10(Math.max(10, distance)) * 0.065);
  const alpha = 1 - Math.exp(-dt / tau);
  return current + diff * alpha;
}
