import { useTranslation } from "react-i18next";
import type { AppLanguage } from "./i18n";
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
  onOpenClipboard: () => void;
  onOpenSessions: () => void;
  onOpenMascot: () => void;
  onOpenNotifications: () => void;
  onOpenShortcuts: () => void;
  todayLabel: string;
  usageDisplaySummary: string;
  hooksSummary: string;
  hooksNeedAttention: boolean;
  hooksAllConnected: boolean;
  showMediaSettings: boolean;
  mediaCardEnabled: boolean;
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
  onOpenClipboard,
  onOpenSessions,
  onOpenMascot,
  onOpenNotifications,
  onOpenShortcuts,
  todayLabel,
  usageDisplaySummary,
  hooksSummary,
  hooksNeedAttention,
  hooksAllConnected,
  showMediaSettings,
  mediaCardEnabled,
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
            badge={todayLabel}
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
