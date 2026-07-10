#!/usr/bin/env node

import { hookAuthHeaders, resolveHookConfig } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/claude/pre-tool-use";
const hookConfig = resolveHookConfig("claudeUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const MAX_STDIN_BYTES = 2 * 1024 * 1024;

try {
  const rawPayload = await readStdin();
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  const response = await fetch(hookUrl, {
    method: "POST",
    headers: { "content-type": "application/json", ...hookAuthHeaders(hookConfig.token) },
    body: payload || "{}",
  });

  if (!response.ok) {
    throw new Error(`Atoll hook bridge returned HTTP ${response.status}`);
  }

  const text = await response.text();
  JSON.parse(text);
  process.stdout.write(text);
} catch (error) {
  process.stdout.write(fallbackResponse(hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__), error));
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let value = "";
    let totalBytes = 0;
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      totalBytes += Buffer.byteLength(chunk, "utf8");
      if (totalBytes > MAX_STDIN_BYTES) {
        reject(new Error("Atoll hook payload exceeds 2 MiB"));
        process.stdin.removeAllListeners();
        return;
      }
      value += chunk;
    });
    process.stdin.on("end", () => {
      globalThis.__ATOLL_LAST_PAYLOAD__ = value;
      resolve(value);
    });
    process.stdin.on("error", reject);
  });
}

function hookEventNameFromPayload(payload) {
  if (!payload) return "PreToolUse";

  try {
    return JSON.parse(payload).hook_event_name || "PreToolUse";
  } catch {
    return "PreToolUse";
  }
}

function fallbackResponse(hookEventName, error) {
  if (
    hookEventName === "PermissionRequest" ||
    hookEventName === "PostToolUse" ||
    hookEventName === "PostToolUseFailure" ||
    hookEventName === "Stop" ||
    hookEventName === "StopFailure" ||
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
