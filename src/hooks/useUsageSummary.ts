// Token/cost usage summaries for the compact counters and settings page.
// Extracted from App.tsx; behavior unchanged.
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import type { IslandSnapshot } from "../tauri";
import { byModelCostUsd, pricingRateMap } from "../pricing";
import { formatCompactTokenCount } from "../tokenCounterFormat";
import { formatCompactCost } from "../costFormat";
import { ZERO_TOKEN_USAGE } from "../snapshotDefaults";
import type { UsageDisplayMode } from "../displayPrefs";
import type { TokenUsage } from "../tauri/types";

interface UseUsageSummaryOptions {
  snapshot: IslandSnapshot;
  pricingRates: ReturnType<typeof pricingRateMap>;
  foldedCounterDisplay: UsageDisplayMode;
  expandedCounterDisplay: UsageDisplayMode;
  settingsBadgeDisplay: UsageDisplayMode;
  heatmapDisplay: UsageDisplayMode;
}

export function useUsageSummary({
  snapshot,
  pricingRates,
  foldedCounterDisplay,
  expandedCounterDisplay,
  settingsBadgeDisplay,
  heatmapDisplay,
}: UseUsageSummaryOptions) {
  const { t: tSettings, i18n: i18nInstance } = useTranslation("settings");

  const dailyTokens = snapshot.dailyTokens ?? ZERO_TOKEN_USAGE;
  const dailyTokenTotal = dailyTokens.inputTokens + dailyTokens.outputTokens;
  const activeSessionTokens = snapshot.activeSessionTokens ?? ZERO_TOKEN_USAGE;
  const activeSessionTokenTotal =
    activeSessionTokens.inputTokens + activeSessionTokens.outputTokens;
  const dailyCostTotal = useMemo(
    () => byModelCostUsd(snapshot.dailyTokensByModel, pricingRates),
    [snapshot.dailyTokensByModel, pricingRates],
  );
  const activeSessionCostTotal = useMemo(
    () => byModelCostUsd(snapshot.activeSessionTokensByModel, pricingRates),
    [snapshot.activeSessionTokensByModel, pricingRates],
  );
  const usageDisplaySummary = useMemo(() => {
    const modes = [
      foldedCounterDisplay,
      expandedCounterDisplay,
      settingsBadgeDisplay,
      heatmapDisplay,
    ];
    const costCount = modes.filter((mode) => mode === "cost").length;
    if (costCount === 0) return tSettings("usage.summaryTokens");
    if (costCount === modes.length) return tSettings("usage.summaryCost");
    return tSettings("usage.summaryMixedCost", { count: costCount });
  }, [
    foldedCounterDisplay,
    expandedCounterDisplay,
    settingsBadgeDisplay,
    heatmapDisplay,
    tSettings,
    i18nInstance.language,
  ]);
  const settingsTodayLabel = useMemo(
    () =>
      settingsBadgeDisplay === "cost"
        ? dailyCostTotal > 0
          ? tSettings("usage.todayCost", {
              amount: formatCompactCost(dailyCostTotal, 0, dailyCostTotal),
            })
          : tSettings("usage.noPricedUsage")
        : dailyTokenTotal > 0
          ? tSettings("usage.todayTokens", {
              amount: formatCompactTokenCount(
                dailyTokenTotal,
                dailyTokenTotal >= 1_000 ? 1 : 0,
                dailyTokenTotal,
              ),
            })
          : tSettings("usage.noUsageYet"),
    [
      settingsBadgeDisplay,
      dailyCostTotal,
      dailyTokenTotal,
      tSettings,
      i18nInstance.language,
    ],
  );

  return {
    dailyTokens,
    dailyTokenTotal,
    activeSessionTokens,
    activeSessionTokenTotal,
    dailyCostTotal,
    activeSessionCostTotal,
    usageDisplaySummary,
    settingsTodayLabel,
  };
}
