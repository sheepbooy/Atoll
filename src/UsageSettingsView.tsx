import { useTranslation } from "react-i18next";
import type { UsageDisplayMode } from "./displayPrefs";
import type { ModelPricingEntry } from "./pricing";
import { PricingSettings } from "./PricingSettings";
import {
  clampMonthlyWage,
  clampWorkDays,
  clampWorkHours,
  type SalaryCurrency,
  type SalarySettings,
} from "./salarySettings";
import { formatSalaryRate, salaryPerSecond } from "./salaryFormat";

const DISPLAY_MODES: UsageDisplayMode[] = ["tokens", "cost", "salary"];
const SALARY_CURRENCIES: SalaryCurrency[] = ["¥", "$", "€"];

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
          {DISPLAY_MODES.map((candidate) => (
            <button
              key={candidate}
              type="button"
              className={`settings-segment${mode === candidate ? " is-active" : ""}`}
              onClick={() => onChange(candidate)}
              data-no-drag
            >
              {t(`displayMode.${candidate}`)}
            </button>
          ))}
        </div>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

function SalaryNumberField({
  label,
  value,
  step,
  clamp,
  onChange,
}: {
  label: string;
  value: number;
  step: number;
  clamp: (value: number) => number;
  onChange: (value: number) => void;
}) {
  return (
    <label className="pricing-rate-field">
      <span>{label}</span>
      <input
        type="number"
        min={0}
        step={step}
        value={Number.isFinite(value) ? value : 0}
        onChange={(event) => {
          const next = Number(event.target.value);
          if (Number.isFinite(next)) onChange(clamp(next));
        }}
        data-no-drag
      />
    </label>
  );
}

function SalarySettingsCard({
  salarySettings,
  onChange,
}: {
  salarySettings: SalarySettings;
  onChange: (settings: SalarySettings) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{t("usagePage.salaryCardTitle")}</span>
        <div
          className="settings-segmented"
          role="group"
          aria-label={t("usagePage.salaryCurrency")}
        >
          {SALARY_CURRENCIES.map((currency) => (
            <button
              key={currency}
              type="button"
              className={`settings-segment${
                salarySettings.currency === currency ? " is-active" : ""
              }`}
              onClick={() => onChange({ ...salarySettings, currency })}
              data-no-drag
            >
              {currency}
            </button>
          ))}
        </div>
      </div>
      <div className="salary-fields-grid">
        <SalaryNumberField
          label={t("usagePage.salaryMonthlyWage")}
          value={salarySettings.monthlyWage}
          step={100}
          clamp={clampMonthlyWage}
          onChange={(monthlyWage) => onChange({ ...salarySettings, monthlyWage })}
        />
        <SalaryNumberField
          label={t("usagePage.salaryWorkDays")}
          value={salarySettings.workDaysPerMonth}
          step={1}
          clamp={clampWorkDays}
          onChange={(workDaysPerMonth) =>
            onChange({ ...salarySettings, workDaysPerMonth })
          }
        />
        <SalaryNumberField
          label={t("usagePage.salaryWorkHours")}
          value={salarySettings.workHoursPerDay}
          step={0.5}
          clamp={clampWorkHours}
          onChange={(workHoursPerDay) =>
            onChange({ ...salarySettings, workHoursPerDay })
          }
        />
      </div>
      <span className="settings-card-desc">
        {t("usagePage.salaryRatePreview", {
          rate: formatSalaryRate(salaryPerSecond(salarySettings), salarySettings.currency),
        })}
      </span>
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
  salarySettings: SalarySettings;
  onChangeSalarySettings: (settings: SalarySettings) => void;
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
  salarySettings,
  onChangeSalarySettings,
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
          <span className="settings-section-label">{t("section.salary")}</span>
          <SalarySettingsCard
            salarySettings={salarySettings}
            onChange={onChangeSalarySettings}
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
