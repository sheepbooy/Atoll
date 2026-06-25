#!/usr/bin/env node

import { resolveHookUrl } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/cursor/hook";
const hookUrl = resolveHookUrl("cursorUrl", defaultHookUrl);

try {
  const payload = await readStdin();
  const response = await fetch(hookUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
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
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
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
  if (!payload) return "preToolUse";

  try {
    return JSON.parse(payload).hook_event_name || "preToolUse";
  } catch {
    return "preToolUse";
  }
}

function fallbackResponse(hookEventName, error) {
  if (
    hookEventName === "postToolUse" ||
    hookEventName === "postToolUseFailure" ||
    hookEventName === "stop" ||
    hookEventName === "subagentStart" ||
    hookEventName === "subagentStop"
  ) {
    return "{}";
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
