#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { hookAuthHeaders, resolveHookConfig } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/cursor/hook";
const hookConfig = resolveHookConfig("cursorUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_CURSOR_HOOK_TIMEOUT_MS);
const MAX_STDIN_BYTES = 2 * 1024 * 1024;

try {
  const rawPayload = await readStdin();
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
  process.stdout.write(fallbackResponse(hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__), error));
}

function parseHookTimeoutMs(value) {
  const parsed = Number.parseInt(value || "", 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return Math.min(parsed, 5000);
  }
  return 1200;
}

function logHookInvoke(payload, error = null) {
  if (!error && process.env.ATOLL_CURSOR_HOOK_DEBUG !== "1") {
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
      const parsed = parseHookPayload(payload);
      event = parsed.hook_event_name || parsed.hookEventName || event;
    } catch {
      // ignore parse errors for logging
    }
    const payloadBytes = Buffer.byteLength(payload || "", "utf8");
    const errorSuffix = error ? ` error=${error.message}` : "";
    fs.appendFileSync(
      path.join(base, "cursor-hook-invoke.log"),
      `${new Date().toISOString()} event=${event} bytes=${payloadBytes} url=${hookUrl}${errorSuffix}\n`,
    );
  } catch {
    // logging must never break the hook
  }
}

function readStdin() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let totalBytes = 0;
    process.stdin.on("data", (chunk) => {
      totalBytes += chunk.length;
      if (totalBytes > MAX_STDIN_BYTES) {
        reject(new Error("Atoll hook payload exceeds 2 MiB"));
        process.stdin.removeAllListeners();
        return;
      }
      chunks.push(chunk);
    });
    process.stdin.on("end", () => {
      const buf = Buffer.concat(chunks);
      const value = buf.toString("utf-8");
      globalThis.__ATOLL_LAST_PAYLOAD__ = value;
      resolve(value);
    });
    process.stdin.on("error", reject);
  });
}

function parseHookPayload(payload) {
  const trimmed = (payload || "").replace(/^\uFEFF/, "").trim();
  if (!trimmed) return {};
  return JSON.parse(trimmed);
}

function hookEventNameFromPayload(payload) {
  if (!payload) return "preToolUse";

  try {
    const parsed = parseHookPayload(payload);
    return parsed.hook_event_name || parsed.hookEventName || "preToolUse";
  } catch {
    return "preToolUse";
  }
}

function fallbackResponse(hookEventName, error) {
  if (
    hookEventName === "sessionStart" ||
    hookEventName === "afterAgentResponse" ||
    hookEventName === "afterAgentThought" ||
    hookEventName === "sessionEnd" ||
    hookEventName === "postToolUse" ||
    hookEventName === "postToolUseFailure" ||
    hookEventName === "stop" ||
    hookEventName === "subagentStart" ||
    hookEventName === "subagentStop"
  ) {
    return "{}";
  }

  if (hookEventName === "beforeSubmitPrompt") {
    return JSON.stringify({ continue: true });
  }

  if (hookEventName === "preToolUse") {
    return JSON.stringify({ permission: "allow" });
  }

  const message = `Atoll unavailable: ${error.message}`;
  return JSON.stringify({
    permission: "deny",
    user_message: message,
    agent_message: message,
  });
}
