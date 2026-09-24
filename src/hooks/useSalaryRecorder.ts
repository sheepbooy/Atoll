import { useEffect, useRef } from "react";

import { recordSalaryDay } from "../tauri/salary";
import { localDateKey, salaryEarnedToday } from "../salaryFormat";
import type { SalarySettings } from "../salarySettings";

const RECORD_INTERVAL_MS = 30_000;

/**
 * Reports the observed "earned today" amount to the Rust salary history so the
 * heatmap can look back. Runs only while some surface displays salary mode;
 * crossing midnight naturally starts a new day key, and a hidden page or
 * unmount flushes the latest observation.
 */
export function useSalaryRecorder(settings: SalarySettings, enabled: boolean): void {
  const settingsRef = useRef(settings);
  settingsRef.current = settings;

  useEffect(() => {
    if (!enabled) return;

    const report = () => {
      const now = new Date();
      void recordSalaryDay(localDateKey(now), salaryEarnedToday(settingsRef.current, now));
    };

    report();
    const timer = window.setInterval(report, RECORD_INTERVAL_MS);

    const onVisibilityChange = () => {
      if (document.visibilityState === "hidden") report();
    };
    window.addEventListener("pagehide", report);
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      window.clearInterval(timer);
      window.removeEventListener("pagehide", report);
      document.removeEventListener("visibilitychange", onVisibilityChange);
      report();
    };
  }, [enabled]);
}
