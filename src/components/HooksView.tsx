import {
  Download,
  Trash2,
} from "lucide-react";
import {
  useTranslation,
} from "react-i18next";
import {
  type HookStatus,
} from "../tauri";
import {
  hookRetrustNote,
  hookStatusIssue,
  isHookReady,
  deadCompetingHooks,
  type HookAgentKey,
} from "../hookHealth";
import i18n from "../i18n";

export interface HookMenuAgent {
  key: string;
  label: string;
  status: HookStatus | null;
  note?: string;
  onInstall: () => void;
  onUninstall: () => void;
  onRemoveCompetingHooks?: () => void;
}

export interface HooksViewProps {
  agents: HookMenuAgent[];
  hookBusy: boolean;
  hookInstallError: string | null;
  onInstallAll: () => void;
  onUninstallAll: () => void;
}

export function formatHookInstallErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return i18n.t("error.unknown", { ns: "hooks" });
}

export function HooksView({
  agents,
  hookBusy,
  hookInstallError,
  onInstallAll,
  onUninstallAll,
}: HooksViewProps) {
  const { t } = useTranslation("hooks");
  const installedCount = agents.filter((agent) => agent.status?.installed).length;
  const missingCount = agents.filter(
    (agent) => agent.status && !agent.status.installed,
  ).length;

  return (
    <div className="settings-view" data-no-drag>
      <div className="settings-body">
        <div className="settings-section">
          <span className="settings-section-label">{t("section.agents")}</span>
          {hookInstallError ? (
            <span className="settings-card-desc settings-hook-warning">{hookInstallError}</span>
          ) : null}
          {missingCount > 0 || installedCount > 1 ? (
            <div className="settings-hook-bulk">
              {missingCount > 0 ? (
                <button
                  type="button"
                  className="settings-hook-button"
                  onClick={onInstallAll}
                  disabled={hookBusy}
                  data-no-drag
                >
                  <Download size={13} />
                  {t("bulk.installAll")}
                </button>
              ) : null}
              {installedCount > 1 ? (
                <button
                  type="button"
                  className="settings-hook-button is-muted"
                  onClick={onUninstallAll}
                  disabled={hookBusy}
                  data-no-drag
                >
                  <Trash2 size={13} />
                  {t("bulk.uninstallAll")}
                </button>
              ) : null}
            </div>
          ) : null}
          {agents.map((agent) => {
            const installed = Boolean(agent.status?.installed);
            const scriptMissing = agent.status && !agent.status.scriptFound;
            const ready = isHookReady(agent.status);
            const needsRetrust = ready && Boolean(agent.status?.needsRetrust);
            const statusIssue = hookStatusIssue(agent.status);
            const deadCompetitors = deadCompetingHooks(agent.status);
            return (
              <div key={agent.key} className="settings-card settings-hook-card">
                <div className="settings-card-head">
                  <span className="settings-card-title">{agent.label}</span>
                  <span
                    className={`settings-hook-badge${
                      ready
                        ? needsRetrust
                          ? " is-warning"
                          : " is-installed"
                        : installed
                          ? " is-warning"
                          : " is-missing"
                    }`}
                  >
                    {ready
                      ? needsRetrust
                        ? t("status.needsRetrust")
                        : t("status.connected")
                      : installed
                        ? t("status.shimMissing")
                        : t("status.notInstalled")}
                  </span>
                </div>
                {agent.status?.settingsPath ? (
                  <span className="settings-card-desc settings-hook-path">
                    {agent.status.settingsPath}
                  </span>
                ) : null}
                {agent.note && (!installed || agent.key === "codex" || agent.key === "claude" || agent.key === "cursor" || agent.key === "zcode" || agent.key === "gemini") ? (
                  <span className="settings-card-desc">{agent.note}</span>
                ) : null}
                {agent.key === "claude" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>{t("checklist.claudeTitle")}</summary>
                    <ul>
                      <li>{t("checklist.claudeNode")}</li>
                      <li>{t("checklist.claudePermissions")}</li>
                      <li>{t("checklist.claudeRestart")}</li>
                      <li>{t("checklist.claudeVerify")}</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "codex" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>{t("checklist.codexTitle")}</summary>
                    <ul>
                      <li>{t("checklist.codexNode")}</li>
                      <li>{t("checklist.codexTrust")}</li>
                      <li>{t("checklist.codexRestart")}</li>
                      <li>{t("checklist.codexVerify")}</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "zcode" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>{t("checklist.zcodeTitle")}</summary>
                    <ul>
                      <li>{t("checklist.zcodeNode")}</li>
                      <li>{t("checklist.zcodeEnable")}</li>
                      <li>{t("checklist.zcodeRestart")}</li>
                      <li>{t("checklist.zcodeVerify")}</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "gemini" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>{t("checklist.geminiTitle")}</summary>
                    <ul>
                      <li>{t("checklist.geminiNode")}</li>
                      <li>{t("checklist.geminiTrust")}</li>
                      <li>{t("checklist.geminiRestart")}</li>
                      <li>{t("checklist.geminiVerify")}</li>
                    </ul>
                  </details>
                ) : null}
                {agent.key === "cursor" ? (
                  <details className="settings-hook-desktop-note">
                    <summary>{t("checklist.cursorTitle")}</summary>
                    <ul>
                      <li>{t("checklist.cursorNode")}</li>
                      <li>{t("checklist.cursorSettings")}</li>
                      <li>{t("checklist.cursorRestart")}</li>
                      <li>{t("checklist.cursorVerify")}</li>
                    </ul>
                  </details>
                ) : null}
                {needsRetrust ? (
                  <span className="settings-card-desc settings-hook-warning">
                    {hookRetrustNote(agent.key as HookAgentKey)}
                  </span>
                ) : null}
                {statusIssue ? (
                  <span className="settings-card-desc settings-hook-warning">
                    {statusIssue}
                  </span>
                ) : null}
                {scriptMissing ? (
                  <span className="settings-card-desc settings-hook-warning">
                    {t("warning.scriptMissing")}
                  </span>
                ) : null}
                {deadCompetitors.length > 0 && agent.onRemoveCompetingHooks ? (
                  <div className="settings-hook-competing">
                    <span className="settings-card-desc settings-hook-warning">
                      {t("competing.deadWarning", {
                        count: deadCompetitors.length,
                      })}
                    </span>
                    <ul className="settings-hook-competing-list">
                      {deadCompetitors.map((hook) => (
                        <li key={`${hook.event}:${hook.command}`} className="settings-hook-competing-item">
                          <code>{hook.command}</code>
                          <span className="settings-hook-competing-event">
                            {hook.event}
                          </span>
                        </li>
                      ))}
                    </ul>
                    <button
                      type="button"
                      className="settings-hook-button"
                      onClick={agent.onRemoveCompetingHooks}
                      disabled={hookBusy}
                      data-no-drag
                    >
                      <Trash2 size={13} />
                      {t("competing.removeDead")}
                    </button>
                  </div>
                ) : null}
                <div className="settings-hook-actions">
                  {installed ? (
                    <button
                      type="button"
                      className="settings-hook-button is-muted"
                      onClick={agent.onUninstall}
                      disabled={hookBusy}
                      data-no-drag
                    >
                      <Trash2 size={13} />
                      {t("action.uninstall")}
                    </button>
                  ) : (
                    <button
                      type="button"
                      className="settings-hook-button"
                      onClick={agent.onInstall}
                      disabled={hookBusy}
                      data-no-drag
                    >
                      <Download size={13} />
                      {hookBusy ? t("action.installing") : t("action.install")}
                    </button>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

