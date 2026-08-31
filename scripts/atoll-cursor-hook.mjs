#!/usr/bin/env node

import {
  createHookLogger,
  fallbackResponse,
  hookEventNameFromPayload,
  parseHookTimeoutMs,
  postToBridge,
  readStdin,
  resolveHookConfig,
} from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/cursor/hook";
const hookConfig = resolveHookConfig("cursorUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_CURSOR_HOOK_TIMEOUT_MS);
const logHookInvoke = createHookLogger({
  agent: "cursor",
  hookUrl,
  debugEnvKey: "ATOLL_CURSOR_HOOK_DEBUG",
});

// Cursor's own events (camelCase) must not block the session when Atoll is
// unavailable; beforeSubmitPrompt continues and preToolUse allows locally.
const SILENT_FALLBACK_EVENTS = [
  "sessionStart",
  "afterAgentResponse",
  "afterAgentThought",
  "sessionEnd",
  "postToolUse",
  "postToolUseFailure",
  "stop",
  "subagentStart",
  "subagentStop",
];

function fallbackForEvent(hookEventName, error) {
  if (hookEventName === "beforeSubmitPrompt") {
    return JSON.stringify({ continue: true });
  }

  if (hookEventName === "preToolUse") {
    return JSON.stringify({ permission: "allow" });
  }

  return fallbackResponse(hookEventName, error, SILENT_FALLBACK_EVENTS);
}

try {
  const rawPayload = await readStdin();
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  // Cursor hooks always run with the short timeout: none of its events are
  // allowed to stall the editor.
  const text = await postToBridge({
    hookUrl,
    token: hookConfig.token,
    payload,
    timeoutMs: hookTimeoutMs,
    logger: logHookInvoke,
  });
  process.stdout.write(text);
} catch (error) {
  logHookInvoke(globalThis.__ATOLL_LAST_PAYLOAD__, error);
  process.stdout.write(
    fallbackForEvent(
      hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__),
      error,
    ),
  );
}
