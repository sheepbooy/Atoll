/**
 * Atoll bridge plugin for DeepSeek Harness (DSH).
 *
 * atoll-dsh-bridge — this marker line is load-bearing: Atoll's installer
 * verifies the deployed copy contains it, and the hook-status read uses it to
 * tell Atoll's plugin from a user file that happens to share the name.
 *
 * DSH has no command-hook pipeline. This module is a Cordis host plugin: it
 * runs inside DSH, answers DSH's `approval/request` waterfall through Atoll's
 * local HTTP bridge, and forwards lightweight session lifecycle events.
 *
 * Safety rules:
 *   - Never throw from `apply`: a broken bridge must not stop DSH from booting.
 *   - Never hard-require DSH services: headless/TUI profiles (no `webServer`)
 *     load the module as a no-op.
 *   - Never claim an approval without an Atoll answer: unreadable config,
 *     transport failures, timeouts, and malformed responses all call `next()`
 *     so DSH's native approval UI keeps working.
 */

import fs from "node:fs";
import os from "node:os";
import path from "node:path";

export const BRIDGE_MARKER = "atoll-dsh-bridge";
export const BRIDGE_PROTOCOL_VERSION = 1;
export const APPROVAL_TIMEOUT_MS = 30 * 60 * 1000;
export const OBSERVER_TIMEOUT_MS = 1200;
export const MAX_TOOL_INPUT_BYTES = 256 * 1024;
export const MAX_TOOL_STRING_CHARS = 8 * 1024;
export const MAX_CALL_CACHE_ENTRIES = 256;
export const MAX_OBSERVERS_IN_FLIGHT = 8;

const DEFAULT_HOOK_URL = "http://127.0.0.1:47777/dsh/hook";
const DEBUG_ENV = "ATOLL_DSH_HOOK_DEBUG";

const TOOL_LABELS = {
  bash: "Bash",
  pwsh: "PowerShell",
  write: "Write",
  edit: "Edit",
  read: "Read",
  read_image: "ReadImage",
  glob: "Glob",
  grep: "Grep",
  web_fetch: "WebFetch",
  web_search: "WebSearch",
  str_replace_editor: "Edit",
  apply_patch: "Edit",
  job_output: "JobOutput",
  job_kill: "JobKill",
  todo_write: "TodoWrite",
};

/**
 * Tool-input fields Atoll uses for its approval card and rule matching.
 * File contents, patches, and tool output are deliberately excluded: the
 * approval bridge must never ship large or sensitive payloads.
 */
const TOOL_INPUT_FIELDS = [
  "command",
  "description",
  "justification",
  "sandbox_permissions",
  "workdir",
  "path",
  "file_path",
  "url",
  "query",
  "pattern",
  "glob",
  "mode",
  "title",
  "timeoutMs",
  "run_in_background",
];

const MINIMAL_TOOL_INPUT_FIELDS = ["command", "path", "file_path", "url", "description"];

/** Shared per-user data dir for Atoll bridge-side logs. */
export function hookDataDir(platform = process.platform, homedir = os.homedir(), env = process.env) {
  if (platform === "win32") {
    const localAppData = env.LOCALAPPDATA;
    if (localAppData) return path.join(localAppData, "Atoll");
    return path.join(homedir, "AppData", "Local", "Atoll");
  }
  return path.join(homedir, ".atoll");
}

/** Atoll writes bridge.json under the platform data dir; DSH reads it fresh. */
export function bridgeConfigPath(platform = process.platform, homedir = os.homedir(), env = process.env) {
  if (platform === "win32") {
    const localAppData = env.LOCALAPPDATA;
    if (localAppData) return path.join(localAppData, "Atoll", "bridge.json");
    return path.join(homedir, "AppData", "Local", "Atoll", "bridge.json");
  }
  if (platform === "darwin") {
    return path.join(homedir, "Library", "Application Support", "Atoll", "bridge.json");
  }
  const dataHome = env.XDG_DATA_HOME || path.join(homedir, ".local", "share");
  return path.join(dataHome, "Atoll", "bridge.json");
}

export function readBridgeConfig(configPath) {
  try {
    return JSON.parse(fs.readFileSync(configPath, "utf8"));
  } catch {
    return null;
  }
}

/**
 * Resolve the Atoll endpoint for one request.
 *
 * Returns null when Atoll has never run: without a bridge.json or explicit
 * env override there is nothing to call, so the plugin should stay out of the
 * way immediately instead of probing a default port.
 */
export function resolveAtollConfig(
  env = process.env,
  platform = process.platform,
  homedir = os.homedir(),
) {
  const config = readBridgeConfig(bridgeConfigPath(platform, homedir, env));
  if (config && typeof config.dshUrl === "string" && config.dshUrl) {
    return {
      url: config.dshUrl,
      token: typeof config.token === "string" && config.token ? config.token : null,
    };
  }

  const explicit = env.ATOLL_DSH_HOOK_URL || env.ATOLL_HOOK_URL;
  if (explicit) {
    return { url: explicit, token: env.ATOLL_HOOK_TOKEN || null };
  }

  // An older Atoll may have written bridge.json without dshUrl. Probe the
  // documented default; a 404/connection error still degrades to next().
  if (config) {
    return { url: DEFAULT_HOOK_URL, token: typeof config.token === "string" ? config.token || null : null };
  }
  return null;
}

export function displayToolName(toolName) {
  const raw = typeof toolName === "string" ? toolName.trim() : "";
  if (!raw) return "approval";
  return TOOL_LABELS[raw] || raw;
}

function truncateString(value) {
  if (value.length <= MAX_TOOL_STRING_CHARS) return value;
  return `${value.slice(0, MAX_TOOL_STRING_CHARS)}… [truncated]`;
}

function jsonByteLength(value) {
  try {
    return Buffer.byteLength(JSON.stringify(value), "utf8");
  } catch {
    return MAX_TOOL_INPUT_BYTES + 1;
  }
}

/** Summarize model-supplied tool arguments before they cross the bridge. */
export function summarizeToolInput(input) {
  if (!input || typeof input !== "object" || Array.isArray(input)) return {};
  const summary = {};
  for (const key of TOOL_INPUT_FIELDS) {
    if (!(key in input)) continue;
    const value = input[key];
    if (typeof value === "string") {
      summary[key] = truncateString(value);
    } else if (typeof value === "number" || typeof value === "boolean") {
      summary[key] = value;
    }
  }
  if (jsonByteLength(summary) <= MAX_TOOL_INPUT_BYTES) return summary;

  const minimal = {};
  for (const key of MINIMAL_TOOL_INPUT_FIELDS) {
    if (!(key in summary)) continue;
    const value = summary[key];
    minimal[key] = typeof value === "string" ? truncateString(value.slice(0, 2048)) : value;
  }
  return minimal;
}

export function parseToolArguments(rawArguments) {
  if (rawArguments && typeof rawArguments === "object") return rawArguments;
  if (typeof rawArguments !== "string" || rawArguments.trim() === "") return {};
  try {
    const parsed = JSON.parse(rawArguments);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

export function isSubagentSession(session) {
  return session?.header?.origin === "subagent";
}

/** Parent session id for a subagent; null for roots and forks. */
export function parentSessionId(session) {
  if (!isSubagentSession(session)) return null;
  const parent = session?.header?.parentSession;
  return typeof parent === "string" && parent ? parent : null;
}

export function callCacheKey(sessionId, callId) {
  return `${sessionId}\u0000${callId}`;
}

export function normalizeDecision(response) {
  if (!response || typeof response !== "object") return null;
  if (response.decision === "allow") return "allow";
  if (response.decision === "deny") return "deny";
  return null;
}

export function mapDecisionToOutcome(decision) {
  if (decision === "allow") return "allowed-once";
  if (decision === "deny") return "rejected";
  return null;
}

function sessionCwd(session) {
  const cwd = session?.header?.cwd;
  return typeof cwd === "string" && cwd ? cwd : process.cwd();
}

/** Normalize one DSH approval request into Atoll's tool-payload contract. */
export function buildApprovalPayload(req, call) {
  const session = req?.agent?.session;
  const childId = typeof session?.id === "string" ? session.id : "";
  const parentId = parentSessionId(session);
  const sessionId = parentId || childId || "dsh";
  const reason = typeof req?.reason === "string" ? req.reason.trim() : "";
  const toolName = displayToolName(req?.toolName || call?.name);
  const input = { ...(call?.input || {}) };

  if (reason && !input.description) {
    input.description = reason;
  } else if (reason && input.description && input.description !== reason) {
    input.description = `${input.description} — ${reason}`;
  }
  if (typeof input.justification === "string" && input.justification && input.description) {
    if (!input.description.includes(input.justification)) {
      input.description = `${input.description} — Escalation: ${input.justification}`;
    }
  }

  const toolUseId = req?.callId ?? call?.callId;
  const payload = {
    hook_event_name: "PermissionRequest",
    session_id: sessionId,
    tool_name: toolName,
    tool_input: input,
    cwd: sessionCwd(session),
    bridge_version: BRIDGE_PROTOCOL_VERSION,
  };
  if (parentId && childId && parentId !== childId) {
    payload.subagent_session_id = childId;
  }
  if (toolUseId !== undefined && toolUseId !== null) {
    payload.tool_use_id = String(toolUseId);
  }
  if (reason) payload.reason = reason;
  return payload;
}

export function buildObserverPayload(hookEventName, session, extra = {}) {
  return {
    hook_event_name: hookEventName,
    session_id: session?.id || "dsh",
    cwd: sessionCwd(session),
    ...extra,
  };
}

function authHeaders(token) {
  return token ? { "x-atoll-hook-token": token } : {};
}

/**
 * POST one payload and return the parsed JSON object.
 * The caller owns the fallback policy; this helper only throws.
 */
export async function postToBridge(config, payload, { timeoutMs = 0, signal = null } = {}) {
  const controller = new AbortController();
  let timer = null;
  const onAbort = () => controller.abort(signal?.reason);
  if (signal) {
    if (signal.aborted) controller.abort(signal.reason);
    else signal.addEventListener("abort", onAbort, { once: true });
  }
  if (timeoutMs > 0) {
    timer = setTimeout(() => controller.abort(new Error("Atoll bridge request timed out")), timeoutMs);
  }

  try {
    const response = await fetch(config.url, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        ...authHeaders(config.token),
      },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });
    if (!response.ok) {
      throw new Error(`Atoll bridge returned HTTP ${response.status}`);
    }
    const text = await response.text();
    if (!text.trim()) return {};
    const parsed = JSON.parse(text);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Atoll bridge returned non-object JSON");
    }
    return parsed;
  } finally {
    if (timer !== null) clearTimeout(timer);
    if (signal) signal.removeEventListener("abort", onAbort);
  }
}

function logHook(event, payload, error = null) {
  if (!error && process.env[DEBUG_ENV] !== "1") return;
  try {
    const base = hookDataDir();
    fs.mkdirSync(base, { recursive: true });
    let bytes = 0;
    try {
      bytes = Buffer.byteLength(JSON.stringify(payload || {}), "utf8");
    } catch {
      bytes = 0;
    }
    const suffix = error ? ` error=${error.message}` : "";
    fs.appendFileSync(
      path.join(base, "dsh-bridge-invoke.log"),
      `${new Date().toISOString()} event=${event} bytes=${bytes}${suffix}\n`,
    );
  } catch {
    // Logging must never break DSH.
  }
}

function nextOrUnavailable(next) {
  return typeof next === "function" ? next() : "unavailable";
}

async function handleApproval(state, req, next) {
  const session = req?.agent?.session;
  if (!session) return nextOrUnavailable(next);

  const config = resolveAtollConfig();
  if (!config) return nextOrUnavailable(next);

  const call = req?.callId !== undefined && req?.callId !== null
    ? state.calls.get(callCacheKey(session.id, String(req.callId)))
    : undefined;
  const payload = buildApprovalPayload(req, call);

  try {
    const response = await postToBridge(config, payload, {
      timeoutMs: APPROVAL_TIMEOUT_MS,
      signal: req?.signal || null,
    });
    const decision = normalizeDecision(response);
    const outcome = mapDecisionToOutcome(decision);
    if (!outcome) {
      logHook("PermissionRequest", payload);
      return nextOrUnavailable(next);
    }
    return outcome;
  } catch (error) {
    if (req?.signal?.aborted) return "cancelled";
    logHook("PermissionRequest", payload, error);
    return nextOrUnavailable(next);
  }
}

function postObserver(state, payload) {
  if (state.observersInFlight >= MAX_OBSERVERS_IN_FLIGHT) return;
  const config = resolveAtollConfig();
  if (!config) return;
  state.observersInFlight += 1;
  postToBridge(config, payload, { timeoutMs: OBSERVER_TIMEOUT_MS })
    // Observer events are best-effort; an offline Atoll must not fill the log.
    .catch(() => {})
    .finally(() => {
      state.observersInFlight -= 1;
    });
}

function pruneCallCache(state) {
  while (state.calls.size > MAX_CALL_CACHE_ENTRIES) {
    const oldest = state.calls.keys().next();
    if (oldest.done) return;
    state.calls.delete(oldest.value);
  }
}

function rememberToolCall(state, session, event) {
  const callId = event?.data?.callId;
  if (callId === undefined || callId === null) return;
  state.calls.set(callCacheKey(session.id, String(callId)), {
    callId: String(callId),
    name: event.data.name,
    input: summarizeToolInput(parseToolArguments(event.data.arguments)),
    at: Date.now(),
  });
  pruneCallCache(state);
}

function onToolResult(state, session, event) {
  const content = event?.data?.message?.content;
  const first = Array.isArray(content) ? content[0] : null;
  const callId = first?.toolCallId ?? event?.data?.message?.source?.callId;
  const key = callId !== undefined && callId !== null
    ? callCacheKey(session.id, String(callId))
    : null;
  const call = key ? state.calls.get(key) : undefined;

  if (!isSubagentSession(session)) {
    const failed = Boolean(event?.data?.error) || first?.isError === true;
    postObserver(
      state,
      buildObserverPayload(
        failed ? "PostToolUseFailure" : "PostToolUse",
        session,
        {
          ...(call?.name ? { tool_name: displayToolName(call.name) } : {}),
          ...(callId !== undefined && callId !== null ? { tool_use_id: String(callId) } : {}),
          ...(call?.input ? { tool_input: call.input } : {}),
        },
      ),
    );
  }
  if (key) state.calls.delete(key);
}

function registerSessionObservers(ctx, state) {
  ctx.on("session/created", (session) => {
    if (!isSubagentSession(session)) {
      postObserver(state, buildObserverPayload("SessionStart", session));
    }
  });

  ctx.on("session/disposed", (session) => {
    if (!isSubagentSession(session)) {
      postObserver(state, buildObserverPayload("SessionEnd", session));
    }
  });

  ctx.on("session/event", (session, event) => {
    switch (event?.type) {
      case "tool/call":
        rememberToolCall(state, session, event);
        break;
      case "tool/result":
        onToolResult(state, session, event);
        break;
      case "turn/start":
        if (!isSubagentSession(session)) {
          postObserver(
            state,
            buildObserverPayload("UserPromptSubmit", session, {
              ...(typeof event?.data?.turn === "number" ? { turn: event.data.turn } : {}),
            }),
          );
        }
        break;
      case "turn/end":
        if (!isSubagentSession(session)) {
          postObserver(
            state,
            buildObserverPayload("Stop", session, {
              ...(typeof event?.data?.turn === "number" ? { turn: event.data.turn } : {}),
              ...(typeof event?.data?.reason === "string" ? { reason: event.data.reason } : {}),
            }),
          );
        }
        break;
      default:
        break;
    }
  });
}

/** Wire the bridge into one activated DSH context. */
export function startBridge(ctx) {
  const state = { calls: new Map(), observersInFlight: 0 };
  registerSessionObservers(ctx, state);
  ctx.on(
    "approval/request",
    (req, next) => handleApproval(state, req, next),
    { prepend: true },
  );
  return state;
}

/**
 * Cordis entry point. The `webServer` gate keeps headless and non-Web TUI
 * compositions untouched: the plugin loads but does nothing there.
 */
export function apply(ctx) {
  try {
    if (!ctx || typeof ctx.inject !== "function") return;
    ctx.inject(["webServer"], (hostCtx) => {
      try {
        startBridge(hostCtx);
      } catch (error) {
        logHook("activation", null, error);
      }
    });
  } catch (error) {
    // A failed bridge must never veto DSH boot.
    logHook("activation", null, error);
  }
}

export const name = "atoll-dsh-bridge";
