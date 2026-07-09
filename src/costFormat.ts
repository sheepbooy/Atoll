export function formatCompactCost(
  value: number,
  compact = 0,
  formatHint = value,
): string {
  const abs = Math.abs(value);
  const hintAbs = Math.abs(formatHint);
  const sign = value < 0 ? "-" : "";

  // Abbreviated forms only when compact pressure is high and the amount is large.
  if (compact >= 1 && hintAbs >= 1_000_000) {
    const fractionDigits = compact >= 2 ? 0 : 2;
    return `${sign}$${(abs / 1_000_000).toFixed(fractionDigits)}M`;
  }
  if (compact >= 1 && hintAbs >= 1_000) {
    const fractionDigits = compact >= 2 ? 0 : 2;
    return `${sign}$${(abs / 1_000).toFixed(fractionDigits)}K`;
  }

  if (abs < 0.01 && value === 0) return "$0.00";
  return `${sign}$${abs.toFixed(2)}`;
}

export function costDisplayCompactLevel(
  value: number,
  variant: "compact" | "expanded" | "micro",
  sessionCount: number,
  maxCompactIcons: number,
  maxDisplayChars = variant === "expanded" ? 11 : 999,
): number {
  const fullText = formatCompactCost(value, 0, value);
  if (variant === "compact" || variant === "micro") {
    const iconPressure =
      maxCompactIcons > 0 ? Math.min(sessionCount, maxCompactIcons) / maxCompactIcons : 0;
    const sessionLevel = iconPressure < 0.92 ? 0 : sessionCount <= maxCompactIcons + 2 ? 1 : 2;
    if (fullText.length <= maxDisplayChars) return sessionLevel;
    return Math.max(sessionLevel, fullText.length > 9 ? 2 : 1);
  }

  if (fullText.length <= maxDisplayChars && value < 1000) return 0;
  if (value >= 1_000_000 || fullText.length > 10) return 2;
  if (value >= 1000 || fullText.length > maxDisplayChars) return 1;
  return 0;
}

export function estimateCostDisplayWidth(text: string): number {
  return Math.ceil([...text].reduce((width, char) => width + (char === "$" ? 7 : char === "." ? 4 : 7), 0));
}
