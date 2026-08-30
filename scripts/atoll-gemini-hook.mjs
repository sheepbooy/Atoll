#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { hookAuthHeaders, resolveHookConfig } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/gemini/hook";
const hookConfig = resolveHookConfig("geminiUrl", defaultHookUrl);
const hookUrl = hookConfig.url;
const hookTimeoutMs = parseHookTimeoutMs(process.env.ATOLL_GEMINI_HOOK_TIMEOUT_MS);
const STDIN_TIMEOUT_MS = 5000;
const MAX_STDIN_BYTES = 2 * 1024 * 1024;
// Gemini CLI fires BeforeTool for *every* tool call, but Atoll should only gate
// tools with side effects; read-only tools are approved locally so they never
// touch the bridge. Keep this in sync with the BeforeTool matcher that
// install_gemini_hooks writes into ~/.gemini/settings.json.
const DEFAULT_GATED_TOOL_PATTERN =
  "run_shell_command|write_file|replace|web_fetch|save_memory|invoke_agent|mcp_";

try {
  const rawPayload = await readStdin(STDIN_TIMEOUT_MS);
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  const hookEventName = hookEventNameFromPayload(payload);
  // BeforeTool must block until the user decides (Gemini CLI gates tool
  // execution on the hook response). Observer events use a short timeout so
  // SessionStart/AfterTool cannot stall the Gemini turn.
  if (hookEventName === "BeforeTool" && isGatedTool(payload)) {
    const text = await forwardToBridge(payload, null);
    process.stdout.write(text);
  } else if (hookEventName === "BeforeTool") {
    // Not a side-effect tool: Gemini CLI treats missing output as allow, and an
    // explicit allow keeps that contract stable.
    process.stdout.write(JSON.stringify({ decision: "allow" }));
  } else {
    const text = await forwardToBridge(payload, hookTimeoutMs);
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

function parseHookTimeoutMs(value) {
  const parsed = Number.parseInt(value || "", 10);
  if (Number.isFinite(parsed) && parsed > 0) {
    return Math.min(parsed, 5000);
  }
  return 1200;
}

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

async function forwardToBridge(payload, timeoutMs) {
  let response;
  const controller = timeoutMs == null ? null : new AbortController();
  let timeout;
  try {
    if (controller) {
      timeout = setTimeout(() => controller.abort(), timeoutMs);
    }
    response = await fetch(hookUrl, {
      method: "POST",
      headers: { "content-type": "application/json", ...hookAuthHeaders(hookConfig.token) },
      body: payload || "{}",
      ...(controller ? { signal: controller.signal } : {}),
    });
  } catch (fetchError) {
    logHookInvoke(payload, fetchError);
    throw fetchError;
  } finally {
    clearTimeout(timeout);
  }

  if (!response.ok) {
    const error = new Error(`Atoll hook bridge returned HTTP ${response.status}`);
    logHookInvoke(payload, error);
    throw error;
  }

  const text = await response.text();
  try {
    JSON.parse(text);
  } catch (parseError) {
    logHookInvoke(payload, parseError);
    throw parseError;
  }
  return text;
}

function logHookInvoke(payload, error = null) {
  if (!error && process.env.ATOLL_GEMINI_HOOK_DEBUG !== "1") {
    return;
  }

  try {
    const localAppData = process.env.LOCALAPPDATA;
    const base = process.platform === "win32" && localAppData
      ? path.join(localAppData, "Atoll")
      : path.join(os.homedir(), ".atoll");
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
      path.join(base, "gemini-hook-invoke.log"),
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
  if (!payload) return "BeforeTool";

  try {
    return JSON.parse(payload).hook_event_name || "BeforeTool";
  } catch {
    return "BeforeTool";
  }
}
