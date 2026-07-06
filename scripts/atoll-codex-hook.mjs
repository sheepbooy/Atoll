#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { hookAuthHeaders, resolveHookConfig } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/codex/hook";
const hookConfig = resolveHookConfig("codexUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const STDIN_TIMEOUT_MS = 5000;

try {
  const rawPayload = await readStdin(STDIN_TIMEOUT_MS);
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  logHookInvoke(payload);
  let response;
  try {
    response = await fetch(hookUrl, {
      method: "POST",
      headers: { "content-type": "application/json", ...hookAuthHeaders(hookConfig.token) },
      body: payload || "{}",
    });
  } catch (fetchError) {
    logHookInvoke(payload, fetchError);
    throw fetchError;
  }

  if (!response.ok) {
    const httpError = new Error(`Atoll hook bridge returned HTTP ${response.status}`);
    logHookInvoke(payload, httpError);
    throw httpError;
  }

  const text = (await response.text()) || "{}";
  JSON.parse(text);
  process.stdout.write(text);
} catch (error) {
  logHookInvoke(globalThis.__ATOLL_LAST_PAYLOAD__, error);
  process.stdout.write(
    fallbackResponse(hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__), error),
  );
}

process.exit(0);

function logHookInvoke(payload, error = null) {
  try {
    const localAppData = process.env.LOCALAPPDATA;
    const base = localAppData
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
      path.join(base, "codex-hook-invoke.log"),
      `${new Date().toISOString()} event=${event} bytes=${payloadBytes} url=${hookUrl}${errorSuffix}\n`,
    );
  } catch {
    // logging must never break the hook
  }
}

function readStdin(timeoutMs) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    const timer = setTimeout(() => {
      cleanup();
      globalThis.__ATOLL_LAST_PAYLOAD__ = Buffer.concat(chunks).toString("utf-8");
      resolve(globalThis.__ATOLL_LAST_PAYLOAD__);
    }, timeoutMs);

    const onData = (chunk) => {
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
      clearTimeout(timer);
      process.stdin.off("data", onData);
      process.stdin.off("end", onEnd);
      process.stdin.off("error", onError);
    };

    process.stdin.on("data", onData);
    process.stdin.on("end", onEnd);
    process.stdin.on("error", onError);
  });
}

function hookEventNameFromPayload(payload) {
  if (!payload) return "PermissionRequest";

  try {
    return JSON.parse(payload).hook_event_name || "PermissionRequest";
  } catch {
    return "PermissionRequest";
  }
}

function fallbackResponse(hookEventName, error) {
  if (
    hookEventName === "PermissionRequest" ||
    hookEventName === "PostToolUse" ||
    hookEventName === "Stop" ||
    hookEventName === "SubagentStart" ||
    hookEventName === "SubagentStop"
  ) {
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
