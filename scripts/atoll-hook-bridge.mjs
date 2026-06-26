import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export function bridgeConfigPath() {
  if (process.platform === "win32") {
    const localAppData = process.env.LOCALAPPDATA;
    if (localAppData) {
      return path.join(localAppData, "Atoll", "bridge.json");
    }
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
  // Prefer bridge.json written by the running Atoll instance. Stale ATOLL_HOOK_URL
  // values in hooks.json (e.g. 47777) must not override a fallback port (48800).
  const config = readBridgeConfig();
  if (config?.[configKey]) {
    return config[configKey];
  }

  if (process.env.ATOLL_HOOK_URL) {
    return process.env.ATOLL_HOOK_URL;
  }

  return defaultUrl;
}
