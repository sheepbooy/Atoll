// Language, approval notice mode and global-shortcut settings state, with the
// one-time hydration from persisted backend settings. Extracted from App.tsx.
import { useEffect, useState } from "react";
import {
  getApprovalNoticeMode,
  getGlobalShortcutConfig,
  setApprovalNoticeMode,
  setGlobalShortcutConfig,
  setNotificationLanguage,
  setSessionRetention,
  type ApprovalNoticeMode,
  type GlobalShortcutView,
} from "../tauri";
import { changeAppLanguage, readLanguage, type AppLanguage } from "../i18n";
import { readRetentionMinutes } from "../settingsStorage";

export function useAppSettings() {
  const [language, setLanguage] = useState<AppLanguage>(() => readLanguage());
  const [approvalNoticeMode, setApprovalNoticeModeState] =
    useState<ApprovalNoticeMode>("interrupt");
  const [globalShortcutView, setGlobalShortcutView] = useState<GlobalShortcutView | null>(null);

  useEffect(() => {
    setSessionRetention(readRetentionMinutes()).catch(() => undefined);
    getApprovalNoticeMode()
      .then(setApprovalNoticeModeState)
      .catch(() => undefined);
    getGlobalShortcutConfig()
      .then(setGlobalShortcutView)
      .catch(() => undefined);
    setNotificationLanguage(readLanguage()).catch(() => undefined);
  }, []);

  async function handleChangeLanguage(nextLanguage: AppLanguage) {
    setLanguage(nextLanguage);
    setNotificationLanguage(nextLanguage).catch(() => undefined);
    await changeAppLanguage(nextLanguage);
  }

  function handleChangeApprovalNoticeMode(mode: ApprovalNoticeMode) {
    setApprovalNoticeModeState(mode);
    setApprovalNoticeMode(mode).catch(() => undefined);
  }

  // Optimistically apply the edit, then adopt the backend view: it carries the
  // per-action registration errors (hotkey taken, invalid accelerator) that the
  // settings rows render.
  function handleChangeGlobalShortcutConfig(next: GlobalShortcutView["config"]) {
    setGlobalShortcutView((prev) => (prev ? { ...prev, config: next, errors: {} } : prev));
    setGlobalShortcutConfig(next)
      .then(setGlobalShortcutView)
      .catch(() => undefined);
  }

  return {
    language,
    approvalNoticeMode,
    globalShortcutView,
    handleChangeLanguage,
    handleChangeApprovalNoticeMode,
    handleChangeGlobalShortcutConfig,
  };
}
