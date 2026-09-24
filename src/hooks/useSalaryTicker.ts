import { useEffect, useState } from "react";

import { salaryEarnedToday } from "../salaryFormat";
import type { SalarySettings } from "../salarySettings";

const TICK_INTERVAL_MS = 250;

/**
 * Live "earned today" amount for the salary display mode. Earnings are a pure
 * function of the wall clock, so the interval only re-renders the caller —
 * keep it inside the displaying component to avoid ticking the whole app.
 */
export function useSalaryTicker(
  settings: SalarySettings | undefined,
  enabled: boolean,
): number {
  const [earned, setEarned] = useState(0);

  useEffect(() => {
    if (!enabled || !settings) {
      setEarned(0);
      return;
    }
    const update = () => setEarned(salaryEarnedToday(settings, new Date()));
    update();
    const timer = window.setInterval(update, TICK_INTERVAL_MS);
    return () => window.clearInterval(timer);
  }, [enabled, settings]);

  return earned;
}
