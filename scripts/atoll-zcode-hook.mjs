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

const defaultHookUrl = "http://127.0.0.1:47777/zcode/hook";
const hookConfig = resolveHookConfig("zcodeUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_ZCODE_HOOK_TIMEOUT_MS);
const logHookInvoke = createHookLogger({
  agent: "zcode",
  hookUrl,
  debugEnvKey: "ATOLL_ZCODE_HOOK_DEBUG",
});

// PermissionRequest must block until the user decides. Observer events use a
// short timeout so Stop/PostToolUse cannot stall the ZCode turn. ZCode falls
// back to its own permission prompt when a hook prints nothing, so observer
// events and PermissionRequest degrade to "{}" instead of blocking.
const SILENT_FALLBACK_EVENTS = [
  "PermissionRequest",
  "PostToolUse",
  "PostToolUseFailure",
  "Stop",
  "SessionStart",
  "UserPromptSubmit",
];

try {
  const rawPayload = await readStdin(5000);
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  const hookEventName = hookEventNameFromPayload(payload);
  const useShortTimeout = hookEventName !== "PermissionRequest";
  const text = await postToBridge({
    hookUrl,
    token: hookConfig.token,
    payload,
    timeoutMs: useShortTimeout ? hookTimeoutMs : 0,
    logger: logHookInvoke,
  });
  process.stdout.write(text);
} catch (error) {
  logHookInvoke(globalThis.__ATOLL_LAST_PAYLOAD__, error);
  process.stdout.write(
    fallbackResponse(
      hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__),
      error,
      SILENT_FALLBACK_EVENTS,
    ),
  );
}

process.exit(0);
