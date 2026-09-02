import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  ABSOLUTE_MAX_COMPACT_ICONS,
  MIN_MAX_COMPACT_ICONS,
} from "./compactLayout";
import type { CompactIndicatorMode } from "./displayPrefs";
import type {
  ApprovalNoticeMode,
  GlobalShortcutConfig,
  GlobalShortcutErrors,
  ShortcutAction,
} from "./tauri";
import {
  SHORTCUT_ACTIONS,
  acceleratorFromKeyboardEvent,
  formatAccelerator,
  shortcutPlatform,
  type ShortcutPlatform,
} from "./shortcuts";
import { SettingsSlider, SettingsToggle } from "./SettingsControls";
import type { FoldedIslandSize } from "./SettingsView";

export const MIN_CLIPBOARD_LIMIT = 10;
export const MAX_CLIPBOARD_LIMIT = 500;
export const MIN_RETENTION_MINUTES = 1;
export const MAX_RETENTION_MINUTES = 60;
export const MIN_MAX_SUBAGENT_DISPLAY = 1;
export const MAX_MAX_SUBAGENT_DISPLAY = 10;
export const MIN_IDLE_INTERVAL_MIN = 1;
export const MAX_IDLE_INTERVAL_MIN = 60;
export const MIN_IDLE_DURATION_MIN = 1;
export const MAX_IDLE_DURATION_MIN = 60;

export function IslandSettingsView({
  maxCompactIcons,
  maxCompactIconLimit,
  onChangeMaxCompactIcons,
  showFoldedIslandSizeSetting,
  foldedIslandSize,
  onChangeFoldedIslandSize,
  maxSubagentDisplay,
  onChangeMaxSubagentDisplay,
  showCompactIndicator,
  compactIndicator,
  onChangeCompactIndicator,
}: {
  maxCompactIcons: number;
  maxCompactIconLimit: number;
  onChangeMaxCompactIcons: (value: number) => void;
  showFoldedIslandSizeSetting: boolean;
  foldedIslandSize: FoldedIslandSize;
  onChangeFoldedIslandSize: (small: boolean) => void;
  maxSubagentDisplay: number;
  onChangeMaxSubagentDisplay: (value: number) => void;
  showCompactIndicator: boolean;
  compactIndicator: CompactIndicatorMode;
  onChangeCompactIndicator: (mode: CompactIndicatorMode) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.display")}</span>
          {showCompactIndicator ? (
            <div className="settings-card">
              <div className="settings-card-head">
                <span className="settings-card-title">{t("display.compactIndicatorLabel")}</span>
                <div className="settings-segmented" role="group" aria-label={t("display.compactIndicatorLabel")}>
                  {(["media", "tokens", "both", "none"] as CompactIndicatorMode[]).map((mode) => (
                    <button
                      key={mode}
                      type="button"
                      className={`settings-segment${compactIndicator === mode ? " is-active" : ""}`}
                      onClick={() => onChangeCompactIndicator(mode)}
                      data-no-drag
                    >
                      {t("common:displayMode." + mode)}
                    </button>
                  ))}
                </div>
              </div>
              <span className="settings-card-desc">{t("display.compactIndicatorDesc")}</span>
            </div>
          ) : null}
          {showFoldedIslandSizeSetting ? (
            <SettingsToggle
              label={t("display.smallFoldedIslandLabel")}
              desc={t("display.smallFoldedIslandDesc")}
              checked={foldedIslandSize === "small"}
              onChange={onChangeFoldedIslandSize}
            />
          ) : null}
          <SettingsSlider
            label={t("display.foldedIconLimitLabel")}
            value={maxCompactIcons}
            min={MIN_MAX_COMPACT_ICONS}
            max={maxCompactIconLimit}
            desc={
              maxCompactIconLimit < ABSOLUTE_MAX_COMPACT_ICONS
                ? t("display.foldedIconLimitDescLimited", { limit: maxCompactIconLimit })
                : t("display.foldedIconLimitDescDefault")
            }
            onChange={onChangeMaxCompactIcons}
          />
          <SettingsSlider
            label={t("display.subagentDisplayLimitLabel")}
            value={maxSubagentDisplay}
            min={MIN_MAX_SUBAGENT_DISPLAY}
            max={MAX_MAX_SUBAGENT_DISPLAY}
            desc={t("display.subagentDisplayLimitDesc")}
            onChange={onChangeMaxSubagentDisplay}
          />
        </div>
      </div>
    </div>
  );
}

export function MediaSettingsView({
  mediaCardEnabled,
  onChangeMediaCardEnabled,
  artworkBackdropEnabled,
  onChangeArtworkBackdropEnabled,
  lyricsEnabled,
  onChangeLyricsEnabled,
}: {
  mediaCardEnabled: boolean;
  onChangeMediaCardEnabled: (enabled: boolean) => void;
  artworkBackdropEnabled: boolean;
  onChangeArtworkBackdropEnabled: (enabled: boolean) => void;
  lyricsEnabled: boolean;
  onChangeLyricsEnabled: (enabled: boolean) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.media")}</span>
          <SettingsToggle
            label={t("display.mediaCardLabel")}
            desc={t("display.mediaCardDesc")}
            checked={mediaCardEnabled}
            onChange={onChangeMediaCardEnabled}
          />
          <SettingsToggle
            label={t("display.artworkBackdropLabel")}
            desc={t("display.artworkBackdropDesc")}
            checked={artworkBackdropEnabled}
            onChange={onChangeArtworkBackdropEnabled}
          />
          <SettingsToggle
            label={t("display.lyricsLabel")}
            desc={t("display.lyricsDesc")}
            checked={lyricsEnabled}
            onChange={onChangeLyricsEnabled}
          />
        </div>
      </div>
    </div>
  );
}

export function ClipboardSettingsView({
  clipboardHistoryEnabled,
  onChangeClipboardHistoryEnabled,
  clipboardLimit,
  onChangeClipboardLimit,
}: {
  clipboardHistoryEnabled: boolean;
  onChangeClipboardHistoryEnabled: (enabled: boolean) => void;
  clipboardLimit: number;
  onChangeClipboardLimit: (limit: number) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.clipboard")}</span>
          <SettingsToggle
            label={t("display.clipboardHistoryLabel")}
            desc={t("display.clipboardHistoryDesc")}
            checked={clipboardHistoryEnabled}
            onChange={onChangeClipboardHistoryEnabled}
          />
          <SettingsSlider
            label={t("display.clipboardHistoryLimitLabel")}
            value={clipboardLimit}
            min={MIN_CLIPBOARD_LIMIT}
            max={MAX_CLIPBOARD_LIMIT}
            step={10}
            unit={t("display.clipboardHistoryLimitUnit")}
            desc={t("display.clipboardHistoryLimitDesc")}
            onChange={onChangeClipboardLimit}
          />
        </div>
      </div>
    </div>
  );
}

export function SessionSettingsView({
  retentionMinutes,
  onChangeRetentionMinutes,
  subagentRetentionMinutes,
  onChangeSubagentRetentionMinutes,
}: {
  retentionMinutes: number;
  onChangeRetentionMinutes: (value: number) => void;
  subagentRetentionMinutes: number;
  onChangeSubagentRetentionMinutes: (value: number) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.sessions")}</span>
          <SettingsSlider
            label={t("display.sessionAutoArchiveLabel")}
            value={retentionMinutes}
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            unit={t("display.unitMinutes")}
            desc={t("display.sessionAutoArchiveDesc")}
            onChange={onChangeRetentionMinutes}
          />
          <SettingsSlider
            label={t("display.subagentAutoArchiveLabel")}
            value={subagentRetentionMinutes}
            min={MIN_RETENTION_MINUTES}
            max={MAX_RETENTION_MINUTES}
            unit={t("display.unitMinutes")}
            desc={t("display.subagentAutoArchiveDesc")}
            onChange={onChangeSubagentRetentionMinutes}
          />
        </div>
      </div>
    </div>
  );
}

export function MascotSettingsView({
  idleIntervalMin,
  onChangeIdleInterval,
  idleDurationMin,
  onChangeIdleDuration,
}: {
  idleIntervalMin: number;
  onChangeIdleInterval: (value: number) => void;
  idleDurationMin: number;
  onChangeIdleDuration: (value: number) => void;
}) {
  const { t } = useTranslation("settings");

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.mascot")}</span>
          <SettingsSlider
            label={t("mascot.activityIntervalLabel")}
            value={idleIntervalMin}
            min={MIN_IDLE_INTERVAL_MIN}
            max={MAX_IDLE_INTERVAL_MIN}
            unit={t("display.unitMinutes")}
            desc={t("mascot.activityIntervalDesc")}
            onChange={onChangeIdleInterval}
          />
          <SettingsSlider
            label={t("mascot.activityDurationLabel")}
            value={idleDurationMin}
            min={MIN_IDLE_DURATION_MIN}
            max={MAX_IDLE_DURATION_MIN}
            unit={t("display.unitMinutes")}
            desc={t("mascot.activityDurationDesc")}
            onChange={onChangeIdleDuration}
          />
        </div>
      </div>
    </div>
  );
}

export function NotificationSettingsView({
  mode,
  onChangeMode,
}: {
  mode: ApprovalNoticeMode;
  onChangeMode: (mode: ApprovalNoticeMode) => void;
}) {
  const { t } = useTranslation("settings");
  const modes: { value: ApprovalNoticeMode; labelKey: string; descKey: string }[] = [
    { value: "interrupt", labelKey: "notice.modeInterrupt", descKey: "notice.modeInterruptDesc" },
    { value: "notify", labelKey: "notice.modeNotify", descKey: "notice.modeNotifyDesc" },
  ];
  const activeMode = modes.find((entry) => entry.value === mode) ?? modes[0];

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.notifications")}</span>
          <div className="settings-card">
            <div className="settings-card-head">
              <span className="settings-card-title">{t("notice.modeLabel")}</span>
              <div
                className="settings-segmented"
                role="group"
                aria-label={t("notice.modeLabel")}
              >
                {modes.map((entry) => (
                  <button
                    key={entry.value}
                    type="button"
                    className={`settings-segment${mode === entry.value ? " is-active" : ""}`}
                    onClick={() => onChangeMode(entry.value)}
                    data-no-drag
                  >
                    {t(entry.labelKey)}
                  </button>
                ))}
              </div>
            </div>
            <span className="settings-card-desc">{t(activeMode.descKey)}</span>
          </div>
        </div>
      </div>
    </div>
  );
}

export function ShortcutSettingsView({
  config,
  errors = {},
  platform = shortcutPlatform(),
  onChangeEnabled,
  onChangeAccelerator,
}: {
  config: GlobalShortcutConfig;
  errors?: GlobalShortcutErrors;
  platform?: ShortcutPlatform;
  onChangeEnabled: (enabled: boolean) => void;
  onChangeAccelerator: (action: ShortcutAction, value: string) => void;
}) {
  const { t } = useTranslation("settings");
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [needsModifier, setNeedsModifier] = useState(false);

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.shortcuts")}</span>
          <SettingsToggle
            label={t("shortcuts.enableLabel")}
            desc={t("shortcuts.enableDesc")}
            checked={config.enabled}
            onChange={onChangeEnabled}
          />
          {SHORTCUT_ACTIONS.map((action) => {
            const value = config[action];
            const error = errors[action];
            const isRecording = recording === action;
            return (
              <div className="settings-card" key={action}>
                <div className="settings-card-head">
                  <span className="settings-card-title">
                    {t(`shortcuts.${action}Label`)}
                  </span>
                  <div className="settings-shortcut-field">
                    <input
                      type="text"
                      className={`settings-shortcut-input${isRecording ? " is-recording" : ""}`}
                      value={formatAccelerator(value, platform)}
                      placeholder={t("shortcuts.notSet")}
                      readOnly
                      disabled={!config.enabled}
                      aria-label={t(`shortcuts.${action}Label`)}
                      onFocus={() => {
                        setRecording(action);
                        setNeedsModifier(false);
                      }}
                      onBlur={() => {
                        if (isRecording) {
                          setRecording(null);
                          setNeedsModifier(false);
                        }
                      }}
                      onKeyDown={(event) => {
                        if (event.key === "Escape") {
                          event.currentTarget.blur();
                          return;
                        }
                        const accelerator = acceleratorFromKeyboardEvent(
                          event.nativeEvent,
                          platform,
                        );
                        if (accelerator) {
                          event.preventDefault();
                          setNeedsModifier(false);
                          onChangeAccelerator(action, accelerator);
                        } else {
                          setNeedsModifier(true);
                        }
                      }}
                      data-no-drag
                    />
                    {value ? (
                      <button
                        type="button"
                        className="settings-shortcut-clear"
                        disabled={!config.enabled}
                        onClick={() => onChangeAccelerator(action, "")}
                        data-no-drag
                      >
                        {t("shortcuts.clearLabel")}
                      </button>
                    ) : null}
                  </div>
                </div>
                <span className="settings-card-desc">{t(`shortcuts.${action}Desc`)}</span>
                {isRecording ? (
                  <span className="settings-card-desc settings-shortcut-hint">
                    {t(needsModifier ? "shortcuts.needsModifierHint" : "shortcuts.pressHint")}
                  </span>
                ) : null}
                {error ? <span className="settings-shortcut-error">{error}</span> : null}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
