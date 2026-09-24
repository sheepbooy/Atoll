import { useTranslation } from "react-i18next";
import type { AppLanguage } from "./i18n";
import type { UsageDisplayMode } from "./displayPrefs";
import { formatSalaryEarnings } from "./salaryFormat";
import { DEFAULT_SALARY_SETTINGS, type SalarySettings } from "./salarySettings";
import { useSalaryTicker } from "./hooks/useSalaryTicker";
import {
  SettingsLanguageToggle,
  SettingsNavCard,
  SettingsToggle,
} from "./SettingsControls";

export type FoldedIslandSize = "small" | "regular";

export interface SettingsViewProps {
  launchAtLogin: boolean;
  launchAtLoginBusy?: boolean;
  onChangeLaunchAtLogin: (enabled: boolean) => void;
  language: AppLanguage;
  onChangeLanguage: (language: AppLanguage) => void;
  onOpenHooks: () => void;
  onOpenTokens: () => void;
  onOpenUsage: () => void;
  onOpenIsland: () => void;
  onOpenMedia: () => void;
  onOpenBluetooth: () => void;
  onOpenClipboard: () => void;
  onOpenSessions: () => void;
  onOpenMascot: () => void;
  onOpenNotifications: () => void;
  onOpenRules: () => void;
  onOpenShortcuts: () => void;
  todayLabel: string;
  /** Display mode of the token-activity badge; "salary" ticks live. */
  todayBadgeMode?: UsageDisplayMode;
  salarySettings?: SalarySettings;
  usageDisplaySummary: string;
  hooksSummary: string;
  hooksNeedAttention: boolean;
  hooksAllConnected: boolean;
  showMediaSettings: boolean;
  mediaCardEnabled: boolean;
  showBluetoothSettings: boolean;
  bluetoothCardEnabled: boolean;
  clipboardHistoryEnabled: boolean;
  noticeModeLabel: string;
  shortcutsEnabled: boolean;
}

export function SettingsView({
  launchAtLogin,
  launchAtLoginBusy = false,
  onChangeLaunchAtLogin,
  language,
  onChangeLanguage,
  onOpenHooks,
  onOpenTokens,
  onOpenUsage,
  onOpenIsland,
  onOpenMedia,
  onOpenBluetooth,
  onOpenClipboard,
  onOpenSessions,
  onOpenMascot,
  onOpenNotifications,
  onOpenRules,
  onOpenShortcuts,
  todayLabel,
  todayBadgeMode = "tokens",
  salarySettings,
  usageDisplaySummary,
  hooksSummary,
  hooksNeedAttention,
  hooksAllConnected,
  showMediaSettings,
  mediaCardEnabled,
  showBluetoothSettings,
  bluetoothCardEnabled,
  clipboardHistoryEnabled,
  noticeModeLabel,
  shortcutsEnabled,
}: SettingsViewProps) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.general")}</span>
          <SettingsLanguageToggle language={language} onChange={onChangeLanguage} />
          <SettingsToggle
            label={t("general.launchAtLoginLabel")}
            desc={t("general.launchAtLoginDesc")}
            checked={launchAtLogin}
            disabled={launchAtLoginBusy}
            onChange={onChangeLaunchAtLogin}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.appearance")}</span>
          <SettingsNavCard
            title={t("pages.islandTitle")}
            desc={t("pages.islandDesc")}
            onClick={onOpenIsland}
          />
          <SettingsNavCard
            title={t("pages.mascotTitle")}
            desc={t("pages.mascotDesc")}
            onClick={onOpenMascot}
          />
        </div>

        {showMediaSettings ? (
          <div className="settings-section">
            <span className="settings-section-label">{t("section.media")}</span>
            <SettingsNavCard
              title={t("pages.mediaTitle")}
              desc={t("pages.mediaDesc")}
              badge={mediaCardEnabled ? t("badge.on") : t("badge.off")}
              badgeTone={mediaCardEnabled ? "installed" : ""}
              onClick={onOpenMedia}
            />
          </div>
        ) : null}

        {showBluetoothSettings ? (
          <div className="settings-section">
            <span className="settings-section-label">{t("section.bluetooth")}</span>
            <SettingsNavCard
              title={t("pages.bluetoothTitle")}
              desc={t("pages.bluetoothDesc")}
              badge={bluetoothCardEnabled ? t("badge.on") : t("badge.off")}
              badgeTone={bluetoothCardEnabled ? "installed" : ""}
              onClick={onOpenBluetooth}
            />
          </div>
        ) : null}

        <div className="settings-section">
          <span className="settings-section-label">{t("section.clipboard")}</span>
          <SettingsNavCard
            title={t("pages.clipboardTitle")}
            desc={t("pages.clipboardDesc")}
            badge={clipboardHistoryEnabled ? t("badge.on") : t("badge.off")}
            badgeTone={clipboardHistoryEnabled ? "installed" : ""}
            onClick={onOpenClipboard}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.notifications")}</span>
          <SettingsNavCard
            title={t("pages.notificationsTitle")}
            desc={t("pages.notificationsDesc")}
            badge={noticeModeLabel}
            badgeTone="installed"
            onClick={onOpenNotifications}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.rules")}</span>
          <SettingsNavCard
            title={t("pages.rulesTitle")}
            desc={t("pages.rulesDesc")}
            onClick={onOpenRules}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.shortcuts")}</span>
          <SettingsNavCard
            title={t("pages.shortcutsTitle")}
            desc={t("pages.shortcutsDesc")}
            badge={shortcutsEnabled ? t("badge.on") : t("badge.off")}
            badgeTone={shortcutsEnabled ? "installed" : ""}
            onClick={onOpenShortcuts}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.sessions")}</span>
          <SettingsNavCard
            title={t("pages.sessionsTitle")}
            desc={t("pages.sessionsDesc")}
            onClick={onOpenSessions}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.usage")}</span>
          <SettingsNavCard
            title={t("usage.displayPricingTitle")}
            desc={t("usage.displayPricingDesc")}
            badge={usageDisplaySummary}
            badgeTone="installed"
            onClick={onOpenUsage}
          />
          <SettingsNavCard
            title={t("usage.tokenActivityTitle")}
            desc={t("usage.tokenActivityDesc")}
            badge={
              todayBadgeMode === "salary" ? (
                <SalaryTodayBadgeLabel salarySettings={salarySettings} />
              ) : (
                todayLabel
              )
            }
            badgeTone="installed"
            onClick={onOpenTokens}
          />
        </div>

        <div className="settings-section">
          <span className="settings-section-label">{t("section.integrations")}</span>
          <SettingsNavCard
            title={t("integrations.agentHooksTitle")}
            desc={t("integrations.agentHooksDesc")}
            badge={hooksSummary}
            badgeTone={hooksNeedAttention ? "missing" : hooksAllConnected ? "installed" : ""}
            onClick={onOpenHooks}
          />
        </div>
      </div>
    </div>
  );
}

function SalaryTodayBadgeLabel({ salarySettings }: { salarySettings?: SalarySettings }) {
  const { t } = useTranslation("settings");
  const salary = salarySettings ?? DEFAULT_SALARY_SETTINGS;
  const earned = useSalaryTicker(salarySettings, true);
  return (
    <>
      {t("usage.todaySalary", {
        amount: formatSalaryEarnings(earned, salary.currency, 0, earned),
      })}
    </>
  );
}
