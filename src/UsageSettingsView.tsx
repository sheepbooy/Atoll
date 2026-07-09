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
            Tokens
          </button>
          <button
            type="button"
            className={`settings-segment${mode === "cost" ? " is-active" : ""}`}
            onClick={() => onChange("cost")}
            data-no-drag
          >
            Cost
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
  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">Display</span>
          <SettingsDisplayToggle
            label="Folded counter"
            desc="Active session counter in the folded island."
            mode={foldedCounterDisplay}
            onChange={onChangeFoldedCounterDisplay}
          />
          <SettingsDisplayToggle
            label="Expanded counter"
            desc="Today's total in the expanded island header."
            mode={expandedCounterDisplay}
            onChange={onChangeExpandedCounterDisplay}
          />
          <SettingsDisplayToggle
            label="Settings badge"
            desc="Summary shown on the Token activity entry card."
            mode={settingsBadgeDisplay}
            onChange={onChangeSettingsBadgeDisplay}
          />
          <SettingsDisplayToggle
            label="Heatmap page"
            desc="Daily totals, tooltips, and trend charts on Token activity."
            mode={heatmapDisplay}
            onChange={onChangeHeatmapDisplay}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">Pricing</span>
          <PricingSettings models={pricingModels} onModelsChange={onPricingModelsChange} />
        </div>
      </div>
    </div>
  );
}
