#!/usr/bin/env node

import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { resolveHookUrl } from "./atoll-hook-bridge.mjs";

const defaultHookUrl = "http://127.0.0.1:47777/cursor/hook";
const hookUrl = resolveHookUrl("cursorUrl", defaultHookUrl);

try {
  const rawPayload = await readStdin();
  const payload = (rawPayload || "").replace(/^\uFEFF/, "");
  logHookInvoke(payload);
  let response;
  try {
    response = await fetch(hookUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: payload || "{}",
    });
  } catch (fetchError) {
    logHookInvoke(payload, fetchError);
    throw fetchError;
  }

  if (!response.ok) {
    const httpError = new Error(`Atoll hook bridge returned HTTP ${response.status}`);
    logHookInvoke(payload, httpError);
    throw httpError;
  }

  const text = await response.text();
  JSON.parse(text);
  process.stdout.write(text);
} catch (error) {
  process.stdout.write(fallbackResponse(hookEventNameFromPayload(globalThis.__ATOLL_LAST_PAYLOAD__), error));
}

function logHookInvoke(payload, error = null) {
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
    process.stdin.on("data", (chunk) => {
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
