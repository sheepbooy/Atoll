#!/usr/bin/env bun
/**
 * Atoll bridge plugin for OpenCode.
 *
 * OpenCode has no command-hook pipeline, so unlike the other agent
 * integrations this file is not spawned per event: it runs inside OpenCode's
 * own Bun runtime (installed to ~/.config/opencode/plugins by Atoll, which
 * auto-loads it at startup).
 *
 * Flow for each native permission prompt:
 *   1. OpenCode emits `permission.updated` on its event bus.
 *   2. This plugin normalizes it into Atoll's Claude-style hook payload and
 *      POSTs it to the local Atoll bridge, blocking on the HTTP response.
 *   3. The user answers from the Atoll island; the bridge responds
 *      { decision: "allow" | "deny", reason? }.
 *   4. The plugin relays the answer to OpenCode's native permission API
 *      (POST /session/{id}/permissions/{permissionID}), which resolves the
 *      TUI prompt.
 *
 * Fail-safe by design: if Atoll is not running the fetch fails and OpenCode's
 * own TUI prompt stays up, exactly as if this plugin were not installed.
 */

const DEFAULT_ATOLL_URL = "http://127.0.0.1:47777/opencode/hook";
const REQUEST_TIMEOUT_MS = 30 * 60 * 1000;

// ─── pure helpers (unit-tested via node --test) ──────────────────────────

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export function bridgeConfigPath(platform, homedir, env) {
  if (platform === "win32") {
    const localAppData = env.LOCALAPPDATA;
    if (localAppData) return path.join(localAppData, "Atoll", "bridge.json");
    return path.join(homedir, "AppData", "Local", "Atoll", "bridge.json");
  }
  if (platform === "darwin") {
    return path.join(homedir, "Library", "Application Support", "Atoll", "bridge.json");
  }
  const dataHome = env.XDG_DATA_HOME || path.join(homedir, ".local", "share");
  return path.join(dataHome, "Atoll", "bridge.json");
}

/**
 * Map an OpenCode permission event onto Atoll's Claude-style hook payload.
 * Real-world `permission.asked` properties (observed on 1.18.29):
 *   { id, sessionID, permission: "bash", patterns: [...],
 *     metadata: {...}, always: [...], tool: {messageID, callID} }
 * `permission` names the gate and is the tool label; `tool` is a call
 * reference OBJECT (never a string), so it must not feed the label.
 * Returns null for events that should not surface a card.
 */
export function normalizePermissionEvent(event) {
  if (!event || typeof event !== "object") return null;
  const type = event.type || "";
  if (type !== "permission.updated" && type !== "permission.asked") return null;
  const props = event.properties;
  if (!props || typeof props !== "object") return null;
  const permissionID = props.id || props.permissionID;
  const sessionID = props.sessionID;
  if (!permissionID || !sessionID) return null;

  return {
    hook_event_name: "PermissionRequest",
    session_id: sessionID,
    tool_use_id: permissionID,
    tool_name: permissionToolName(props.permission || props.type),
    tool_input: permissionToolInput(props),
    cwd: typeof props.directory === "string" && props.directory ? props.directory : ".",
  };
}

/** OpenCode permission gates ("bash", "edit", "webfetch", ...) → tool label. */
export function permissionToolName(permissionType) {
  const type = typeof permissionType === "string" ? permissionType : "";
  if (type === "bash") return "Bash";
  return type || "approval";
}

function permissionToolInput(props) {
  const metadata = props.metadata && typeof props.metadata === "object" ? props.metadata : {};
  const input = { ...metadata };
  // `patterns` (current) or `pattern` (older SDK typings) carry the suggested
  // allow patterns shown alongside the native prompt.
  const patterns = props.patterns ?? props.pattern;
  if (typeof patterns === "string") {
    input.pattern = patterns;
  } else if (Array.isArray(patterns)) {
    input.pattern = patterns.join(", ");
  }
  if (typeof props.title === "string" && props.title) {
    input.description = props.title;
  }
  return input;
}

/** Map Atoll's decision onto OpenCode's native permission response. */
export function mapDecisionToResponse(decision) {
  return decision === "allow" ? "once" : "reject";
}

// ─── runtime plumbing ────────────────────────────────────────────────────

export function resolveAtollConfig() {
  try {
    const raw = fs.readFileSync(
      bridgeConfigPath(process.platform, os.homedir(), process.env),
      "utf8",
    );
    const config = JSON.parse(raw);
    if (config?.opencodeUrl) {
      return { url: config.opencodeUrl, token: config.token || null };
    }
  } catch {
    // No bridge.json: fall through to env/default.
  }
  if (process.env.ATOLL_HOOK_URL) {
    return { url: process.env.ATOLL_HOOK_URL, token: process.env.ATOLL_HOOK_TOKEN || null };
  }
  return { url: DEFAULT_ATOLL_URL, token: process.env.ATOLL_HOOK_TOKEN || null };
}

async function postToBridge(payload) {
  const { url, token } = resolveAtollConfig();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        ...(token ? { "x-atoll-hook-token": token } : {}),
      },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    if (!response.ok) return null;
    const text = await response.text();
    return JSON.parse(text);
  } catch {
    return null;
  } finally {
    clearTimeout(timeout);
  }
}

async function replyToNativePermission(client, sessionID, permissionID, response) {
  // The generated SDK client exposes the permission reply as a top-level
  // method; fall back to a plain fetch against the same server when the
  // method shape differs across SDK versions.
  try {
    if (typeof client?.postSessionIdPermissionsPermissionId === "function") {
      await client.postSessionIdPermissionsPermissionId({
        path: { id: sessionID, permissionID },
        body: { response },
      });
      return true;
    }
  } catch {
    // Fall through to the fetch attempt below.
  }
  try {
    const baseUrl =
      client?._client?.config?.baseUrl?.() ??
      client?._client?.config?.baseUrl ??
      process.env.OPENCODE_BASE_URL ??
      "http://localhost:4096";
    const url = `${String(baseUrl).replace(/\/$/, "")}/session/${sessionID}/permissions/${permissionID}`;
    const res = await fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ response }),
    });
    return res.ok;
  } catch {
    return false;
  }
}

export const AtollOpencodePlugin = async ({ client }) => {
  // One in-flight permission at a time per prompt; prompts are keyed by
  // permissionID so concurrent prompts across sessions don't collide.
  const inFlight = new Set();

  return {
    event: async ({ event }) => {
      const payload = normalizePermissionEvent(event);
      if (!payload) return;

      const permissionID = payload.tool_use_id;
      if (inFlight.has(permissionID)) return;
      inFlight.add(permissionID);
      try {
        const decision = await postToBridge(payload);
        // No decision (Atoll down / user answered in the TUI first): the
        // native prompt remains for manual handling.
        if (!decision || typeof decision.decision !== "string") return;
        const response = mapDecisionToResponse(decision.decision);
        await replyToNativePermission(client, payload.session_id, permissionID, response);
      } catch {
        // Never let bridge hiccups break OpenCode's event loop.
      } finally {
        inFlight.delete(permissionID);
      }
    },
  };
};

export default AtollOpencodePlugin;
