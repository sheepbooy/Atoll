#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { hookAuthHeaders, resolveHookConfig } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/codex/hook";
const hookConfig = resolveHookConfig("codexUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_CODEX_HOOK_TIMEOUT_MS);
const STDIN_TIMEOUT_MS = 5000;
const MAX_STDIN_BYTES = 2 * 1024 * 1024;

try {
  const rawPayload = await readStdin(STDIN_TIMEOUT_MS);
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  let text;
  let timeout;
  const controller = new AbortController();
  try {
    timeout = setTimeout(() => controller.abort(), hookTimeoutMs);
    const response = await fetch(hookUrl, {
      method: "POST",
      headers: { "content-type": "application/json", ...hookAuthHeaders(hookConfig.token) },
      body: payload || "{}",
      signal: controller.signal,
    });

    if (!response.ok) {
      throw new Error(`Atoll hook bridge returned HTTP ${response.status}`);
    }

    text = await response.text();
  } catch (fetchError) {
    logHookInvoke(payload, fetchError);
    throw fetchError;
  } finally {
    clearTimeout(timeout);
  }

  try {
    JSON.parse(text);
  } catch (parseError) {
    logHookInvoke(payload, parseError);
    throw parseError;
  }
  process.stdout.write(text);
} catch (error) {
  logHookInvoke(globalThis.__ATOLL_LAST_PAYLOAD__, error);
  process.stdout.write(
    fallbackResponse(hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__), error),
  );
}

process.exit(0);

function parseHookTimeoutMs(value) {
  const parsed = Number.parseInt(value || "", 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return Math.min(parsed, 5000);
  }
  return 1200;
}

function logHookInvoke(payload, error = null) {
  if (!error && process.env.ATOLL_CODEX_HOOK_DEBUG !== "1") {
    return;
  }

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
    let totalBytes = 0;
    const timer = setTimeout(() => {
      cleanup();
      globalThis.__ATOLL_LAST_PAYLOAD__ = Buffer.concat(chunks).toString("utf-8");
      resolve(globalThis.__ATOLL_LAST_PAYLOAD__);
    }, timeoutMs);

    const onData = (chunk) => {
      totalBytes += chunk.length;
      if (totalBytes > MAX_STDIN_BYTES) {
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
