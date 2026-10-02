import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { getHookObservations, onHookObserved, type HookObservation } from "../tauri/agentEvents";
import { manageAsyncUnlisten } from "../asyncUnlisten";

export const HOOK_VERIFY_TIMEOUT_MS = 60_000;
export function verificationSucceeded(observation: HookObservation | null, startedAt: number | null, generation: number): boolean {
  return startedAt !== null && observation?.generation === generation && observation.lastEventAt !== null && observation.lastEventAt >= startedAt;
}

export function HookConnectionCheck({ agent, installed }: { agent: string; installed: boolean }) {
  const { t, i18n } = useTranslation("hooks");
  const [ready, setReady] = useState(false);
  const [observation, setObservation] = useState<HookObservation | null>(null);
  const [attempt, setAttempt] = useState<{ at: number; generation: number } | null>(null);
  const [expired, setExpired] = useState(false);
  useEffect(() => {
    setReady(false);
    setObservation(null);
    setAttempt(null);
    setExpired(false);
    let received = false;
    let cancelled = false;
    const unsubscribe = manageAsyncUnlisten(onHookObserved(event => {
      if (event.agent === agent) { received = true; setReady(true); setObservation(event.observation); }
    }));
    getHookObservations().then(values => { if (!cancelled && !received) { setObservation(values?.[agent] ?? null); setReady(true); } }).catch(() => { if (!cancelled) setReady(true); });
    return () => { cancelled = true; unsubscribe(); };
  }, [agent]);
  const verified = verificationSucceeded(observation, attempt?.at ?? null, attempt?.generation ?? 0);
  useEffect(() => {
    if (!installed || (attempt && observation && attempt.generation !== observation.generation)) {
      setAttempt(null); setExpired(false);
    }
  }, [installed, observation?.generation, attempt]);
  useEffect(() => {
    if (!attempt || verified) return;
    const timer = window.setTimeout(() => setExpired(true), Math.max(0, HOOK_VERIFY_TIMEOUT_MS - (Date.now() - attempt.at)));
    return () => window.clearTimeout(timer);
  }, [attempt, verified]);

  const permissionSeen = verified && (observation?.lastPermissionAt ?? 0) >= (attempt?.at ?? Infinity);
  return (
    <div className="hook-connection-check" data-no-drag>
      <div className="settings-hook-actions">
        <button type="button" className="settings-hook-button" disabled={!installed || !ready}
          onClick={() => { setExpired(false); setAttempt({ at: Date.now(), generation: observation?.generation ?? 0 }); }}>
          {t(attempt ? "verify.retry" : "verify.start")}
        </button>
        <span className="settings-card-desc" role="status">
          {t(verified ? (agent === "cursor" ? "verify.observerConnected" : "verify.connected") : expired ? "verify.timeout" : attempt ? "verify.waiting" : installed ? "verify.installed" : "verify.notInstalled")}
        </span>
      </div>
      {observation?.lastEventAt ? <span className="settings-card-desc">{t("verify.lastEvent", {
        event: observation.lastEventName,
        time: new Date(observation.lastEventAt).toLocaleString(i18n.language),
      })}</span> : null}
      {observation?.lastPermissionAt && agent !== "cursor" ? <span className="settings-card-desc">{t("verify.lastPermission", {
        time: new Date(observation.lastPermissionAt).toLocaleString(i18n.language),
      })}</span> : null}
      {permissionSeen && agent !== "cursor" ? <span className="settings-card-desc">{t("verify.permissionSeen")}</span> : null}
      {attempt && !verified ? <span className="settings-card-desc">{t("verify.steps", { agent })}</span> : null}
    </div>
  );
}
