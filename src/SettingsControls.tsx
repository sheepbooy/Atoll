import type { CSSProperties } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight } from "lucide-react";
import type { AppLanguage } from "./i18n";

export function SettingsLanguageToggle({
  language,
  onChange,
}: {
  language: AppLanguage;
  onChange: (language: AppLanguage) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{t("display.languageLabel")}</span>
        <div className="settings-segmented" role="group" aria-label={t("display.languageLabel")}>
          <button
            type="button"
            className={`settings-segment${language === "en" ? " is-active" : ""}`}
            onClick={() => onChange("en")}
            data-no-drag
          >
            {t("display.languageEnglish")}
          </button>
          <button
            type="button"
            className={`settings-segment${language === "zh-CN" ? " is-active" : ""}`}
            onClick={() => onChange("zh-CN")}
            data-no-drag
          >
            {t("display.languageChinese")}
          </button>
        </div>
      </div>
      <span className="settings-card-desc">{t("display.languageDesc")}</span>
    </div>
  );
}

export function SettingsToggle({
  label,
  desc,
  checked,
  disabled = false,
  onChange,
}: {
  label: string;
  desc: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (enabled: boolean) => void;
}) {
  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{label}</span>
        <button
          type="button"
          role="switch"
          aria-checked={checked}
          aria-label={label}
          className={`settings-toggle${checked ? " is-on" : ""}`}
          disabled={disabled}
          onClick={() => onChange(!checked)}
          data-no-drag
        >
          <span className="settings-toggle-thumb" />
        </button>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

export function SettingsSlider({
  label,
  value,
  min,
  max,
  step = 1,
  unit,
  desc,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  unit?: string;
  desc: string;
  onChange: (v: number) => void;
}) {
  const pct = ((value - min) / (max - min)) * 100;
  return (
    <div className="settings-card">
      <div className="settings-card-head">
        <span className="settings-card-title">{label}</span>
        <span className="settings-card-value">
          {value}
          {unit ? <span className="settings-card-unit">{unit}</span> : null}
        </span>
      </div>
      <div className="settings-slider-wrap">
        <input
          type="range"
          className="settings-slider"
          min={min}
          max={max}
          step={step}
          value={value}
          style={{ "--slider-pct": `${pct}%` } as CSSProperties}
          onChange={(e) => onChange(Number(e.target.value))}
        />
        <div className="settings-slider-labels">
          <span>{min}{unit ?? ""}</span>
          <span>{max}{unit ?? ""}</span>
        </div>
      </div>
      <span className="settings-card-desc">{desc}</span>
    </div>
  );
}

export function SettingsNavCard({
  title,
  desc,
  badge,
  badgeTone,
  onClick,
}: {
  title: string;
  desc: string;
  badge?: string;
  badgeTone?: "installed" | "missing" | "";
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className="settings-nav-card"
      onClick={onClick}
      data-no-drag
    >
      <div className="settings-nav-card-copy">
        <span className="settings-card-title">{title}</span>
        <span className="settings-card-desc">{desc}</span>
      </div>
      <div className="settings-nav-card-meta">
        {badge ? (
          <span
            className={`settings-hook-badge is-summary${
              badgeTone === "missing"
                ? " is-missing"
                : badgeTone === "installed"
                  ? " is-installed"
                  : ""
            }`}
          >
            {badge}
          </span>
        ) : null}
        <ChevronRight size={14} className="settings-nav-chevron" />
      </div>
    </button>
  );
}
