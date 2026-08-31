#!/usr/bin/env node

import os from "node:os";
import path from "node:path";
import {
  createHookLogger,
  hookEventNameFromPayload,
  parseHookTimeoutMs,
  postToBridge,
  readStdin,
  resolveHookConfig,
} from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/gemini/hook";
const hookConfig = resolveHookConfig("geminiUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_GEMINI_HOOK_TIMEOUT_MS);
const logHookInvoke = createHookLogger({
  agent: "gemini",
  hookUrl,
  debugEnvKey: "ATOLL_GEMINI_HOOK_DEBUG",
  // Gemini logs to ~/.atoll on macOS/Linux (LOCALAPPDATA is Windows-only).
  logBase:
    process.platform === "win32" && process.env.LOCALAPPDATA
      ? path.join(process.env.LOCALAPPDATA, "Atoll")
      : path.join(os.homedir(), ".atoll"),
});

// Gemini CLI fires BeforeTool for *every* tool call, but Atoll should only gate
// tools with side effects; read-only tools are approved locally so they never
// touch the bridge. Keep this in sync with the BeforeTool matcher that
// install_gemini_hooks writes into ~/.gemini/settings.json.
const DEFAULT_GATED_TOOL_PATTERN =
  "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_";

function gatedToolPattern() {
  const pattern = process.env.ATOLL_GEMINI_GATED_TOOLS;
  return pattern && pattern.trim() ? pattern.trim() : DEFAULT_GATED_TOOL_PATTERN;
}

function isGatedTool(payload) {
  let toolName = "";
  try {
    toolName = JSON.parse(payload || "{}").tool_name || "";
  } catch {
    // ignore parse errors; treat as gated so the bridge decides
    return true;
  }
  try {
    return new RegExp(gatedToolPattern()).test(toolName);
  } catch {
    // An invalid override pattern must not silently approve everything.
    return true;
  }
}

try {
  const rawPayload = await readStdin(5000);
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  const hookEventName = hookEventNameFromPayload(payload);
  // BeforeTool must block until the user decides (Gemini CLI gates tool
  // execution on the hook response). Observer events use a short timeout so
  // SessionStart/AfterTool cannot stall the Gemini turn.
  if (hookEventName === "BeforeTool" && isGatedTool(payload)) {
    const text = await postToBridge({
      hookUrl,
      token: hookConfig.token,
      payload,
      logger: logHookInvoke,
    });
    process.stdout.write(text);
  } else if (hookEventName === "BeforeTool") {
    // Not a side-effect tool: Gemini CLI treats missing output as allow, and an
    // explicit allow keeps that contract stable.
    process.stdout.write(JSON.stringify({ decision: "allow" }));
  } else {
    const text = await postToBridge({
      hookUrl,
      token: hookConfig.token,
      payload,
      timeoutMs: hookTimeoutMs,
      logger: logHookInvoke,
    });
    process.stdout.write(text);
  }
} catch (error) {
  logHookInvoke(globalThis.__ATOLL_LAST_PAYLOAD__, error);
  // Gemini CLI defaults to "Allow" when a hook prints nothing, so Atoll being
  // unavailable degrades to Gemini's own permission flow instead of blocking
  // the session. Observer events degrade the same way.
  process.stdout.write("{}");
}

process.exit(0);
