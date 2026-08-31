import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export function bridgeConfigPath() {
  if (process.platform === "win32") {
    // LOCALAPPDATA is the canonical location, but some hook host processes
    // (notably Cursor's hook subprocess) spawn with a sanitized environment
    // where LOCALAPPDATA is missing. Reconstruct the same path from the home
    // directory (USERPROFILE), which remains available, so the hook can still
    // find the running Atoll instance's bridge.json and hit the right port
    // instead of falling back to the default (possibly reserved) port.
    const localAppData = process.env.LOCALAPPDATA;
    if (localAppData) {
      return path.join(localAppData, "Atoll", "bridge.json");
    }
    return path.join(os.homedir(), "AppData", "Local", "Atoll", "bridge.json");
  }

  if (process.platform === "darwin") {
    return path.join(
      os.homedir(),
      "Library",
      "Application Support",
      "Atoll",
      "bridge.json",
    );
  }

  const dataHome =
    process.env.XDG_DATA_HOME || path.join(os.homedir(), ".local", "share");
  return path.join(dataHome, "Atoll", "bridge.json");
}

export function readBridgeConfig() {
  try {
    const raw = fs.readFileSync(bridgeConfigPath(), "utf8");
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function resolveHookUrl(configKey, defaultUrl) {
  return resolveHookConfig(configKey, defaultUrl).url;
}

export function resolveHookConfig(configKey, defaultUrl) {
  // Prefer bridge.json written by the running Atoll instance. Stale ATOLL_HOOK_URL
  // values in hooks.json (e.g. 47777) must not override a fallback port (48800).
  const config = readBridgeConfig();
  if (config?.[configKey]) {
    return { url: config[configKey], token: config.token || null };
  }

  if (process.env.ATOLL_HOOK_URL) {
    return {
      url: process.env.ATOLL_HOOK_URL,
      token: process.env.ATOLL_HOOK_TOKEN || null,
    };
  }

  return { url: defaultUrl, token: process.env.ATOLL_HOOK_TOKEN || null };
}

export function hookAuthHeaders(token) {
  return token ? { "x-atoll-hook-token": token } : {};
}

// ─── shared hook-shim runtime ───────────────────────────────────────────

export const MAX_HOOK_STDIN_BYTES = 2 * 1024 * 1024;

export function parseHookTimeoutMs(value) {
  const parsed = Number.parseInt(value || "", 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return Math.min(parsed, 5000);
  }
  return 1200;
}

export function hookEventNameFromPayload(payload, defaultEventName = "PermissionRequest") {
  if (!payload) return defaultEventName;

  try {
    return JSON.parse(payload).hook_event_name || defaultEventName;
  } catch {
    return defaultEventName;
  }
}

// Reads stdin fully, capped at MAX_HOOK_STDIN_BYTES. With `timeoutMs > 0` a
// stalled agent host resolves with whatever arrived instead of hanging forever.
// The raw payload is mirrored on globalThis.__ATOLL_LAST_PAYLOAD__ so error
// fallbacks can still inspect the event after a failure.
export function readStdin(timeoutMs = 0) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let totalBytes = 0;
    const timer = timeoutMs > 0
      ? setTimeout(() => {
          cleanup();
          const value = Buffer.concat(chunks).toString("utf-8");
          globalThis.__ATOLL_LAST_PAYLOAD__ = value;
          resolve(value);
        }, timeoutMs)
      : null;

    const onData = (chunk) => {
      totalBytes += chunk.length;
      if (totalBytes > MAX_HOOK_STDIN_BYTES) {
        cleanup();
        reject(new Error("Atoll hook payload exceeds 2 MiB"));
        return;
      }
      chunks.push(chunk);
    };
    const onEnd = () => {
      cleanup();
      const value = Buffer.concat(chunks).toString("utf-8");
      globalThis.__ATOLL_LAST_PAYLOAD__ = value;
      resolve(value);
    };
    const onError = (error) => {
      cleanup();
      reject(error);
    };
    const cleanup = () => {
      if (timer) clearTimeout(timer);
      process.stdin.off("data", onData);
      process.stdin.off("end", onEnd);
      process.stdin.off("error", onError);
    };

    process.stdin.on("data", onData);
    process.stdin.on("end", onEnd);
    process.stdin.on("error", onError);
  });
}

// Debug logging to <data>/Atoll/<agent>-hook-invoke.log; silent unless the
// per-agent ATOLL_<AGENT>_HOOK_DEBUG=1 env is set, and never fatal.
export function createHookLogger({ agent, hookUrl, debugEnvKey, logBase = null }) {
  return (payload, error = null) => {
    if (!error && process.env[debugEnvKey] !== "1") {
      return;
    }

    try {
      const localAppData = process.env.LOCALAPPDATA;
      const base = logBase
        ? logBase
        : localAppData
          ? path.join(localAppData, "Atoll")
          : path.join(os.homedir(), "AppData", "Local", "Atoll");
      fs.mkdirSync(base, { recursive: true });
      let event = "unknown";
      try {
        event = JSON.parse(payload || "{}").hook_event_name || event;
      } catch {
        // ignore parse errors for logging
      }
      const payloadBytes = Buffer.byteLength(payload || "", "utf8");
      const errorSuffix = error ? ` error=${error.message}` : "";
      fs.appendFileSync(
        path.join(base, `${agent}-hook-invoke.log`),
        `${new Date().toISOString()} event=${event} bytes=${payloadBytes} url=${hookUrl}${errorSuffix}\n`,
      );
    } catch {
      // logging must never break the hook
    }
  };
}

// POSTs the payload and returns the validated JSON text. `timeoutMs > 0`
// aborts the request after the delay (observer events must not stall the
// agent's turn). Throws on transport/HTTP/JSON errors after logging.
export async function postToBridge({ hookUrl, token, payload, timeoutMs = 0, logger = null }) {
  let text;
  let timeout;
  const controller = timeoutMs > 0 ? new AbortController() : null;
  try {
    if (controller) {
      timeout = setTimeout(() => controller.abort(), timeoutMs);
    }
    const response = await fetch(hookUrl, {
      method: "POST",
      headers: { "content-type": "application/json", ...hookAuthHeaders(token) },
      body: payload || "{}",
      ...(controller ? { signal: controller.signal } : {}),
    });

    if (!response.ok) {
      throw new Error(`Atoll hook bridge returned HTTP ${response.status}`);
    }

    text = await response.text();
  } catch (fetchError) {
    if (logger) logger(payload, fetchError);
    throw fetchError;
  } finally {
    if (timeout) clearTimeout(timeout);
  }

  try {
    JSON.parse(text);
  } catch (parseError) {
    if (logger) logger(payload, parseError);
    throw parseError;
  }
  return text;
}

// Events in `silentEvents` degrade to "{}" (the agent's own flow proceeds);
// everything else asks the agent to prompt the user.
export function fallbackResponse(hookEventName, error, silentEvents) {
  if (silentEvents.includes(hookEventName)) {
    return "{}";
  }

  return JSON.stringify({
    hookSpecificOutput: {
      hookEventName,
      permissionDecision: "ask",
      permissionDecisionReason: `Atoll unavailable: ${error.message}`,
    },
  });
}
