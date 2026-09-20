import { useCallback, useEffect, useState } from "react";
import {
  ClipboardEntry,
  getClipboardAutoStage,
  getClipboardHistory,
  getClipboardHistoryEnabled,
  getClipboardHistoryLimit,
  onClipboardHistoryChanged,
  setClipboardAutoStage,
  setClipboardHistoryEnabled,
  setClipboardHistoryLimit,
} from "../tauri";
import { manageAsyncUnlisten } from "../asyncUnlisten";
import {
  MAX_CLIPBOARD_LIMIT,
  MIN_CLIPBOARD_LIMIT,
} from "../SettingsPages";

export function useClipboardHistory() {
  const [clipboardHistory, setClipboardHistory] = useState<ClipboardEntry[]>([]);
  const [clipboardEnabled, setClipboardEnabled] = useState(false);
  const [clipboardLimit, setClipboardLimit] = useState(50);
  const [clipboardAutoStage, setAutoStage] = useState(false);

  useEffect(() => {
    getClipboardHistoryEnabled()
      .then(setClipboardEnabled)
      .catch(() => undefined);
    getClipboardHistoryLimit()
      .then(setClipboardLimit)
      .catch(() => undefined);
    getClipboardAutoStage()
      .then(setAutoStage)
      .catch(() => undefined);
    getClipboardHistory()
      .then(setClipboardHistory)
      .catch(() => undefined);
    const unsubscribe = manageAsyncUnlisten(
      onClipboardHistoryChanged((entries) => {
        setClipboardHistory(entries);
      }),
    );
    return () => {
      unsubscribe();
    };
  }, []);

  const handleChangeClipboardEnabled = useCallback((enabled: boolean) => {
    setClipboardEnabled(enabled);
    setClipboardHistoryEnabled(enabled).catch(() => undefined);
    if (enabled) {
      getClipboardHistory()
        .then(setClipboardHistory)
        .catch(() => undefined);
    }
  }, []);

  const handleChangeClipboardLimit = useCallback((limit: number) => {
    const clamped = Math.min(
      MAX_CLIPBOARD_LIMIT,
      Math.max(MIN_CLIPBOARD_LIMIT, Math.round(limit)),
    );
    setClipboardLimit(clamped);
    setClipboardHistoryLimit(clamped).catch(() => undefined);
  }, []);

  const handleChangeClipboardAutoStage = useCallback((enabled: boolean) => {
    setAutoStage(enabled);
    setClipboardAutoStage(enabled).catch(() => undefined);
  }, []);

  return {
    clipboardHistory,
    clipboardEnabled,
    clipboardLimit,
    clipboardAutoStage,
    setClipboardHistory,
    handleChangeClipboardEnabled,
    handleChangeClipboardLimit,
    handleChangeClipboardAutoStage,
  };
}
