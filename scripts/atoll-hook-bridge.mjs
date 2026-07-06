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
