#!/usr/bin/env node

import {
  fallbackResponse,
  hookEventNameFromPayload,
  hookAuthHeaders,
  postToBridge,
  readStdin,
  resolveHookConfig,
} from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/claude/pre-tool-use";
const hookConfig = resolveHookConfig("claudeUrl", defaultHookUrl);
const hookUrl = hookConfig.url;

// Claude Code keeps its own permission prompt when a hook prints nothing, so
// observer events and PermissionRequest degrade to "{}" instead of blocking.
const SILENT_FALLBACK_EVENTS = [
  "PermissionRequest",
  "PostToolUse",
  "PostToolUseFailure",
  "Stop",
  "StopFailure",
  "SubagentStop",
];

try {
  const rawPayload = await readStdin();
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  const text = await postToBridge({
    hookUrl,
    token: hookConfig.token,
    payload,
  });
  process.stdout.write(text);
} catch (error) {
  process.stdout.write(
    fallbackResponse(
      hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__, "PreToolUse"),
      error,
      SILENT_FALLBACK_EVENTS,
    ),
  );
}
