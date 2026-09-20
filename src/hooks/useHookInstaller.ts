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
  installOpencodeHooks,
  uninstallOpencodeHooks,
  type HookStatus,
  type HookHealthSnapshot,
} from "../tauri";
import {
  markAllHookAgentsConfigured,
  markHookAgentConfigured,
  readConfiguredHookAgents,
} from "../hookAgentsConfigured";
import { HOOK_AGENT_LABELS, mergeHookHealthPreferReady } from "../hookHealth";
import type { HookAgentKey } from "../hookHealth";
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
  const [hookBusy, setHookBusy] = useState<HookAgentKey | "all" | false>(false);
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

  // The per-agent install/uninstall commands can fail individually (config
  // write errors, marker verification, …). Running them with allSettled keeps
  // one agent's failure from masking the others' success — both in the
  // applied health snapshot and in the error shown to the user.
  const ALL_HOOK_AGENTS: ReadonlyArray<{
    key: HookAgentKey;
    label: string;
    install: () => Promise<HookStatus>;
    uninstall: () => Promise<HookStatus>;
  }> = [
    { key: "claude", label: HOOK_AGENT_LABELS.claude, install: installClaudeHooks, uninstall: uninstallClaudeHooks },
    { key: "codex", label: HOOK_AGENT_LABELS.codex, install: installCodexHooks, uninstall: uninstallCodexHooks },
    { key: "cursor", label: HOOK_AGENT_LABELS.cursor, install: installCursorHooks, uninstall: uninstallCursorHooks },
    { key: "zcode", label: HOOK_AGENT_LABELS.zcode, install: installZcodeHooks, uninstall: uninstallZcodeHooks },
    { key: "gemini", label: HOOK_AGENT_LABELS.gemini, install: installGeminiHooks, uninstall: uninstallGeminiHooks },
    { key: "opencode", label: HOOK_AGENT_LABELS.opencode, install: installOpencodeHooks, uninstall: uninstallOpencodeHooks },
  ];

  async function runHookAgentActions(
    action: (agent: (typeof ALL_HOOK_AGENTS)[number]) => Promise<HookStatus>,
  ): Promise<{
    statuses: Partial<Record<HookAgentKey, HookStatus>>;
    rejectedLabels: string[];
    firstError: unknown | null;
  }> {
    const settled = await Promise.allSettled(ALL_HOOK_AGENTS.map((agent) => action(agent)));
    const statuses: Partial<Record<HookAgentKey, HookStatus>> = {};
    const rejectedLabels: string[] = [];
    let firstError: unknown | null = null;
    settled.forEach((result, index) => {
      const agent = ALL_HOOK_AGENTS[index];
      if (result.status === "fulfilled") {
        statuses[agent.key] = result.value;
      } else {
        rejectedLabels.push(agent.label);
        if (firstError === null) firstError = result.reason;
      }
    });
    return { statuses, rejectedLabels, firstError };
  }

  function applyHookInstallSnapshot(
    statuses: Partial<
      Record<"claude" | "codex" | "cursor" | "zcode" | "gemini" | "opencode", HookStatus>
    >,
  ) {
    invalidatePendingSnapshotLoads();
    const installedHealth: HookHealthSnapshot = {
      claude: statuses.claude ?? snapshotRef.current.hookHealth.claude,
      codex: statuses.codex ?? snapshotRef.current.hookHealth.codex,
      cursor: statuses.cursor ?? snapshotRef.current.hookHealth.cursor,
      zcode: statuses.zcode ?? snapshotRef.current.hookHealth.zcode,
      gemini: statuses.gemini ?? snapshotRef.current.hookHealth.gemini,
      opencode: statuses.opencode ?? snapshotRef.current.hookHealth.opencode,
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
    setHookBusy("claude");
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
    setHookBusy("codex");
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
    setHookBusy("zcode");
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
    setHookBusy("gemini");
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

  async function handleInstallOpencodeHooks() {
    setHookBusy("opencode");
    setHookInstallError(null);
    try {
      const status = await installOpencodeHooks();
      if (status.installed) {
        setConfiguredHookAgents(markHookAgentConfigured("opencode"));
      }
      await applyHookInstallSnapshot({ opencode: status });
      if (status.installed) {
        collapseIsland(true);
      }
    } catch (error) {
      setHookInstallError(
        i18n.t("error.installFailed", {
          ns: "hooks",
          agentLabel: "OpenCode",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleInstallAllHooks() {
    setHookBusy("all");
    setHookInstallError(null);
    try {
      setConfiguredHookAgents(markAllHookAgentsConfigured());
      const { statuses, rejectedLabels, firstError } = await runHookAgentActions(
        (agent) => agent.install(),
      );
      await applyHookInstallSnapshot(statuses);
      if (Object.values(statuses).some((status) => status.installed)) {
        collapseIsland(true);
      }
      // Agents that reported not-installed plus agents whose command failed.
      const failures = [
        ...ALL_HOOK_AGENTS.filter(
          (agent) => statuses[agent.key] && !statuses[agent.key]?.installed,
        ).map((agent) => agent.label),
        ...rejectedLabels,
      ];
      if (failures.length === ALL_HOOK_AGENTS.length && firstError !== null) {
        setHookInstallError(
          i18n.t("error.installFailed", {
            ns: "hooks",
            agentLabel: "Agent hooks",
            message: formatHookInstallErrorMessage(firstError),
          }),
        );
      } else if (failures.length > 0) {
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
    setHookBusy("claude");
    setHookInstallError(null);
    try {
      const status = await uninstallClaudeHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          claude: status,
        },
      });    } catch (error) {
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
    setHookBusy("claude");
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
    setHookBusy("codex");
    try {
      const status = await uninstallCodexHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          codex: status,
        },
      });    } catch (error) {
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
    setHookBusy("zcode");
    try {
      const status = await uninstallZcodeHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          zcode: status,
        },
      });    } catch (error) {
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
    setHookBusy("gemini");
    try {
      const status = await uninstallGeminiHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          gemini: status,
        },
      });    } catch (error) {
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

  async function handleUninstallOpencodeHooks() {
    closeMenu();
    setHookBusy("opencode");
    try {
      const status = await uninstallOpencodeHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          opencode: status,
        },
      });    } catch (error) {
      setHookInstallError(
        i18n.t("error.uninstallFailed", {
          ns: "hooks",
          agentLabel: "OpenCode",
          message: formatHookInstallErrorMessage(error),
        }),
      );
    } finally {
      setHookBusy(false);
    }
  }

  async function handleUninstallHooks() {
    closeMenu();
    setHookBusy("all");
    setHookInstallError(null);
    try {
      const { statuses, rejectedLabels, firstError } = await runHookAgentActions(
        (agent) => agent.uninstall(),
      );
      // Apply the returned statuses directly: a getSnapshot() here races the
      // per-command cache-refresh threads and can read the pre-uninstall
      // hook health, which the prefer-ready event merge would then keep
      // alive — that read as "uninstall needs two clicks".
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          ...statuses,
        },
      });
      if (rejectedLabels.length === ALL_HOOK_AGENTS.length && firstError !== null) {
        setHookInstallError(
          i18n.t("error.uninstallAllFailed", {
            ns: "hooks",
            message: formatHookInstallErrorMessage(firstError),
          }),
        );
      } else if (rejectedLabels.length > 0) {
        setHookInstallError(
          i18n.t("error.uninstallPartial", {
            ns: "hooks",
            agents: rejectedLabels.join(", "),
          }),
        );
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
    setHookBusy("cursor");
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
    setHookBusy("cursor");
    setHookInstallError(null);
    try {
      const status = await uninstallCursorHooks();
      // The returned status is authoritative: apply it immediately instead of
      // blocking the button flip on a second full-snapshot build.
      applySnapshot({
        ...snapshotRef.current,
        hookHealth: {
          ...snapshotRef.current.hookHealth,
          cursor: status,
        },
      });    } catch (error) {
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
    handleInstallOpencodeHooks,
    handleInstallCursorHooks,
    handleInstallAllHooks,
    handleUninstallClaudeHooks,
    handleUninstallCodexHooks,
    handleUninstallZcodeHooks,
    handleUninstallGeminiHooks,
    handleUninstallOpencodeHooks,
    handleUninstallCursorHooks,
    handleUninstallHooks,
    handleRemoveCompetingClaudeHooks,
  };
}
