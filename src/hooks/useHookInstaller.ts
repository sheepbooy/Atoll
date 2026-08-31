import { useEffect, useRef, useState } from "react";
import i18n from "../i18n";
import {
  getSnapshot,
  installClaudeHooks,
  uninstallClaudeHooks,
  removeCompetingClaudeHooks,
  installCodexHooks,
  uninstallCodexHooks,
  installCursorHooks,
  uninstallCursorHooks,
  installZcodeHooks,
  uninstallZcodeHooks,
  installGeminiHooks,
  uninstallGeminiHooks,
  type HookStatus,
  type HookHealthSnapshot,
} from "../tauri";
import {
  markAllHookAgentsConfigured,
  markHookAgentConfigured,
  readConfiguredHookAgents,
} from "../hookAgentsConfigured";
import { mergeHookHealthPreferReady } from "../hookHealth";
import { formatHookInstallErrorMessage } from "../components/HooksView";
import type { IslandSnapshot } from "../tauri";

interface UseHookInstallerOptions {
  applySnapshot: (snapshot: IslandSnapshot) => void;
  snapshotRef: { current: IslandSnapshot };
  invalidatePendingSnapshotLoads: () => void;
  collapseIsland: (skipAnimation?: boolean) => void;
  markHookHealthHydrated: () => void;
  closeMenu: () => void;
}

export function useHookInstaller({
  applySnapshot,
  snapshotRef,
  invalidatePendingSnapshotLoads,
  collapseIsland,
  markHookHealthHydrated,
  closeMenu,
}: UseHookInstallerOptions) {
  const [hookBusy, setHookBusy] = useState(false);
  const [hookInstallError, setHookInstallError] = useState<string | null>(null);
  const [configuredHookAgents, setConfiguredHookAgents] = useState(() =>
    readConfiguredHookAgents(),
  );

  // Safety net for a wedged hook invoke: without a forced reset, one call that
  // never resolves would keep every hook button disabled until app restart.
  useEffect(() => {
    if (!hookBusy) return;
    const timeout = window.setTimeout(() => setHookBusy(false), 30_000);
    return () => window.clearTimeout(timeout);
  }, [hookBusy]);

  function applyHookInstallSnapshot(
    statuses: Partial<
      Record<"claude" | "codex" | "cursor" | "zcode" | "gemini", HookStatus>
    >,
  ) {
    invalidatePendingSnapshotLoads();
    const installedHealth: HookHealthSnapshot = {
      claude: statuses.claude ?? snapshotRef.current.hookHealth.claude,
      codex: statuses.codex ?? snapshotRef.current.hookHealth.codex,
      cursor: statuses.cursor ?? snapshotRef.current.hookHealth.cursor,
      zcode: statuses.zcode ?? snapshotRef.current.hookHealth.zcode,
      gemini: statuses.gemini ?? snapshotRef.current.hookHealth.gemini,
    };
    const optimisticHookHealth = mergeHookHealthPreferReady(
      snapshotRef.current.hookHealth,
      installedHealth,
    );
    applySnapshot({
      ...snapshotRef.current,
      hookHealth: optimisticHookHealth,
      online: true,
    });
    markHookHealthHydrated();
    return getSnapshot()
      .catch(() => null)
      .then((nextSnapshot) => {
        if (!nextSnapshot) return;
        applySnapshot({
          ...nextSnapshot,
          hookHealth: mergeHookHealthPreferReady(
            nextSnapshot.hookHealth,
            installedHealth,
          ),
          online: nextSnapshot.online || true,
        });
      });
  }

  async function handleInstallClaudeHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installClaudeHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("claude"));
      }
      await applyHookInstallSnapshot({ claude: status });
      if (status.installed) {
        collapseIsland(true);
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "Claude Code",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallCodexHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installCodexHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("codex"));
      }
      await applyHookInstallSnapshot({ codex: status });
      if (status.installed) {
        setHookInstallError(null);
      } else {
        setHookInstallError(
          i18n.t("error.codexNotSaved", { ns: "hooks" }),
        );
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "Codex",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallZcodeHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installZcodeHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("zcode"));
      }
      await applyHookInstallSnapshot({ zcode: status });
      if (status.installed) {
        collapseIsland(true);
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "ZCode",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallGeminiHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installGeminiHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("gemini"));
      }
      await applyHookInstallSnapshot({ gemini: status });
      if (status.installed) {
        collapseIsland(true);
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "Gemini CLI",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallAllHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      setConfiguredHookAgents(markAllHookAgentsConfigured());
      const [claudeStatus, codexStatus, cursorStatus, zcodeStatus, geminiStatus] =
        await Promise.all([
          installClaudeHooks(),
          installCodexHooks(),
          installCursorHooks(),
          installZcodeHooks(),
          installGeminiHooks(),
        ]);
      await applyHookInstallSnapshot({
        claude: claudeStatus,
        codex: codexStatus,
        cursor: cursorStatus,
        zcode: zcodeStatus,
        gemini: geminiStatus,
      });
      if (
        claudeStatus.installed ||
        codexStatus.installed ||
        cursorStatus.installed ||
        zcodeStatus.installed ||
        geminiStatus.installed
      ) {
        collapseIsland(true);
      }
      const failures = [
        !claudeStatus.installed ? "Claude Code" : null,
        !codexStatus.installed ? "Codex" : null,
        !cursorStatus.installed ? "Cursor" : null,
        !zcodeStatus.installed ? "ZCode" : null,
        !geminiStatus.installed ? "Gemini CLI" : null,
      ].filter(Boolean);
      if (failures.length > 0) {
        setHookInstallError(
          i18n.t("error.installPartial", {
            ns: "hooks",
            agents: failures.join(", "),
          }),
        );
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "Agent hooks",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallClaudeHooks() {
    closeMenu();
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await uninstallClaudeHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            claude: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "Claude Code",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleRemoveCompetingClaudeHooks() {
    closeMenu();
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await removeCompetingClaudeHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            claude: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.cleanupFailed", {
          ns: "hooks",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallCodexHooks() {
    closeMenu();
    setHookBusy(true);
    try {
      const status = await uninstallCodexHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            codex: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "Codex",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallZcodeHooks() {
    closeMenu();
    setHookBusy(true);
    try {
      const status = await uninstallZcodeHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            zcode: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "ZCode",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallGeminiHooks() {
    closeMenu();
    setHookBusy(true);
    try {
      const status = await uninstallGeminiHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            gemini: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "Gemini CLI",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallHooks() {
    closeMenu();
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const [claudeStatus, codexStatus, cursorStatus, zcodeStatus, geminiStatus] =
        await Promise.all([
          uninstallClaudeHooks(),
          uninstallCodexHooks(),
          uninstallCursorHooks(),
          uninstallZcodeHooks(),
          uninstallGeminiHooks(),
        ]);
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            claude: claudeStatus,
            codex: codexStatus,
            cursor: cursorStatus,
            zcode: zcodeStatus,
            gemini: geminiStatus,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallAllFailed", {
          ns: "hooks",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallCursorHooks() {
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await installCursorHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("cursor"));
      }
      await applyHookInstallSnapshot({ cursor: status });
      if (status.installed) {
        setHookInstallError(null);
      } else {
        setHookInstallError(
          i18n.t("error.cursorNotSaved", { ns: "hooks" }),
        );
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "Cursor",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallCursorHooks() {
    closeMenu();
    setHookBusy(true);
    setHookInstallError(null);
    try {
      const status = await uninstallCursorHooks();
      const nextSnapshot = await getSnapshot().catch(() => null);
      if (nextSnapshot) {
        applySnapshot(nextSnapshot);
      } else {
        applySnapshot({
          ...snapshotRef.current,
          hookHealth: {
            ...snapshotRef.current.hookHealth,
            cursor: status,
          },
        });
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "Cursor",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  return {
    hookBusy,
    hookInstallError,
    setHookInstallError,
    configuredHookAgents,
    setConfiguredHookAgents,
    applyHookInstallSnapshot,
    handleInstallClaudeHooks,
    handleInstallCodexHooks,
    handleInstallZcodeHooks,
    handleInstallGeminiHooks,
    handleInstallCursorHooks,
    handleInstallAllHooks,
    handleUninstallClaudeHooks,
    handleUninstallCodexHooks,
    handleUninstallZcodeHooks,
    handleUninstallGeminiHooks,
    handleUninstallCursorHooks,
    handleUninstallHooks,
    handleRemoveCompetingClaudeHooks,
  };
}
