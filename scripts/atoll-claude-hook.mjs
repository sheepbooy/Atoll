#!/usr/bin/env node

const defaultHookUrl = "http://127.0.0.1:47777/claude/pre-tool-use";
const hookUrl = process.env.ATOLL_HOOK_URL || defaultHookUrl;

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
  if (!payload) return "PreToolUse";

  try {
    return JSON.parse(payload).hook_event_name || "PreToolUse";
  } catch {
    return "PreToolUse";
  }
}

function fallbackResponse(hookEventName, error) {
  if (hookEventName === "PermissionRequest" || hookEventName === "PostToolUse") {
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
