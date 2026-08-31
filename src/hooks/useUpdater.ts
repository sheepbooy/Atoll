import { useEffect, useRef, useState } from "react";
import {
  checkAppUpdate,
  getAppVersion,
  installAppUpdate,
  UPDATE_INITIAL_DELAY_MS,
  UPDATE_RECHECK_MS,
  type AppUpdateState,
} from "../appUpdate";

export function useUpdater({ closeMenu }: { closeMenu: () => void }) {
  const [updateState, setUpdateState] = useState<AppUpdateState>({ status: "idle" });
  const [updateNotice, setUpdateNotice] = useState<string | null>(null);
  const updateCheckInFlightRef = useRef(false);
  const updateNoticeTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!updateNotice) {
      return;
    }

    function closeOnEscape(event: KeyboardEvent) {
      if (event.key === "Escape") {
        dismissUpdateNotice();
      }
    }

    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [updateNotice]);

  useEffect(() => {
    return () => {
      if (updateNoticeTimerRef.current !== null) {
        window.clearTimeout(updateNoticeTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    const runSilentCheck = async () => {
      if (updateCheckInFlightRef.current) {
        return;
      }
      updateCheckInFlightRef.current = true;
      const result = await checkAppUpdate();
      updateCheckInFlightRef.current = false;
      if (cancelled || result.status === "error") {
        return;
      }
      setUpdateState(result);
    };

    const initialTimer = window.setTimeout(() => {
      if (!cancelled) {
        void runSilentCheck();
      }
    }, UPDATE_INITIAL_DELAY_MS);

    const intervalId = window.setInterval(() => {
      if (!cancelled) {
        void runSilentCheck();
      }
    }, UPDATE_RECHECK_MS);

    return () => {
      cancelled = true;
      window.clearTimeout(initialTimer);
      window.clearInterval(intervalId);
    };
  }, []);

  const updateAvailable = updateState.status === "available";
  const updateVersion =
    updateState.status === "available" || updateState.status === "downloading"
      ? updateState.version
      : null;
  const updateDownloading = updateState.status === "downloading";
  const updateDownloadProgress =
    updateState.status === "downloading" ? updateState.progress : 0;
  const updateChecking = updateState.status === "checking";

  function dismissUpdateNotice() {
    setUpdateNotice(null);
    if (updateNoticeTimerRef.current !== null) {
      window.clearTimeout(updateNoticeTimerRef.current);
      updateNoticeTimerRef.current = null;
    }
  }

  function showUpdateNotice(version: string) {
    dismissUpdateNotice();
    setUpdateNotice(version);
    updateNoticeTimerRef.current = window.setTimeout(() => {
      dismissUpdateNotice();
    }, 5000);
  }

  async function runUpdateCheck() {
    if (updateCheckInFlightRef.current || updateDownloading) {
      return;
    }
    updateCheckInFlightRef.current = true;
    setUpdateState({ status: "checking" });
    const result = await checkAppUpdate();
    updateCheckInFlightRef.current = false;
    if (result.status === "error") {
      setUpdateState({ status: "idle" });
      return;
    }
    setUpdateState(result);
  }

  async function handleCheckForUpdates() {
    closeMenu();
    if (updateCheckInFlightRef.current || updateDownloading) {
      return;
    }
    updateCheckInFlightRef.current = true;
    setUpdateState({ status: "checking" });
    const result = await checkAppUpdate();
    updateCheckInFlightRef.current = false;
    if (result.status === "error") {
      setUpdateState({ status: "idle" });
      return;
    }
    setUpdateState(result);
    if (result.status === "idle") {
      const version = (await getAppVersion()) ?? "0.0.0";
      showUpdateNotice(version);
    }
  }

  async function handleInstallUpdate() {
    if (!updateAvailable || !updateVersion) {
      return;
    }
    closeMenu();
    setUpdateState({ status: "downloading", version: updateVersion, progress: 0 });
    try {
      await installAppUpdate((progress) => {
        setUpdateState({ status: "downloading", version: updateVersion, progress });
      });
    } catch {
      setUpdateState({ status: "idle" });
    }
  }

  return {
    updateState,
    updateNotice,
    updateAvailable,
    updateVersion,
    updateDownloading,
    updateDownloadProgress,
    updateChecking,
    dismissUpdateNotice,
    showUpdateNotice,
    runUpdateCheck,
    handleCheckForUpdates,
    handleInstallUpdate,
  };
}
