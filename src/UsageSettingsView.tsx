import { useTranslation } from "react-i18next";
import type { UsageDisplayMode } from "./displayPrefs";
import type { ModelPricingEntry } from "./pricing";
import { PricingSettings } from "./PricingSettings";

function SettingsDisplayToggle({
  label,
  desc,
  mode,
  onChange,
}: {
  label: string;
  desc: string;
  mode: UsageDisplayMode;
  onChange: (mode: UsageDisplayMode) => void;
}) {
  const { t } = useTranslation("common");

  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{label}</span>
        <div className="settings-segmented" role="group" aria-label={label}>
          <button
            type="button"
            className={`settings-segment${mode === "tokens" ? " is-active" : ""}`}
            onClick={() => onChange("tokens")}
            data-no-drag
          >
            {t("displayMode.tokens")}
          </button>
          <button
            type="button"
            className={`settings-segment${mode === "cost" ? " is-active" : ""}`}
            onClick={() => onChange("cost")}
            data-no-drag
          >
            {t("displayMode.cost")}
          </button>
        </div>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

export interface UsageSettingsViewProps {
  foldedCounterDisplay: UsageDisplayMode;
  expandedCounterDisplay: UsageDisplayMode;
  settingsBadgeDisplay: UsageDisplayMode;
  heatmapDisplay: UsageDisplayMode;
  onChangeFoldedCounterDisplay: (mode: UsageDisplayMode) => void;
  onChangeExpandedCounterDisplay: (mode: UsageDisplayMode) => void;
  onChangeSettingsBadgeDisplay: (mode: UsageDisplayMode) => void;
  onChangeHeatmapDisplay: (mode: UsageDisplayMode) => void;
  pricingModels: ModelPricingEntry[];
  onPricingModelsChange: (models: ModelPricingEntry[]) => void;
}

export function UsageSettingsView({
  foldedCounterDisplay,
  expandedCounterDisplay,
  settingsBadgeDisplay,
  heatmapDisplay,
  onChangeFoldedCounterDisplay,
  onChangeExpandedCounterDisplay,
  onChangeSettingsBadgeDisplay,
  onChangeHeatmapDisplay,
  pricingModels,
  onPricingModelsChange,
}: UsageSettingsViewProps) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.display")}</span>
          <SettingsDisplayToggle
            label={t("usagePage.foldedCounterLabel")}
            desc={t("usagePage.foldedCounterDesc")}
            mode={foldedCounterDisplay}
            onChange={onChangeFoldedCounterDisplay}
          />
          <SettingsDisplayToggle
            label={t("usagePage.expandedCounterLabel")}
            desc={t("usagePage.expandedCounterDesc")}
            mode={expandedCounterDisplay}
            onChange={onChangeExpandedCounterDisplay}
          />
          <SettingsDisplayToggle
            label={t("usagePage.settingsBadgeLabel")}
            desc={t("usagePage.settingsBadgeDesc")}
            mode={settingsBadgeDisplay}
            onChange={onChangeSettingsBadgeDisplay}
          />
          <SettingsDisplayToggle
            label={t("usagePage.heatmapLabel")}
            desc={t("usagePage.heatmapDesc")}
            mode={heatmapDisplay}
            onChange={onChangeHeatmapDisplay}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.pricing")}</span>
          <PricingSettings models={pricingModels} onModelsChange={onPricingModelsChange} />
        </div>
      </div>
    </div>
  );
}
