import { useCallback, useEffect, useState } from "react";
import {
  ClipboardEntry,
  getClipboardHistory,
  getClipboardHistoryEnabled,
  getClipboardHistoryLimit,
  onClipboardHistoryChanged,
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

  useEffect(() => {
    getClipboardHistoryEnabled()
      .then(setClipboardEnabled)
      .catch(() => undefined);
    getClipboardHistoryLimit()
      .then(setClipboardLimit)
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

  return {
    clipboardHistory,
    clipboardEnabled,
    clipboardLimit,
    setClipboardHistory,
    handleChangeClipboardEnabled,
    handleChangeClipboardLimit,
  };
}
