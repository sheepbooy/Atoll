// atoll-pi-bridge — Atoll's approval and telemetry bridge for Pi.
//
// Pi has no built-in permission system and no shell hooks, so this global
// extension is the gate: `tool_call` is awaited before every tool executes,
// giving Atoll a chance to approve or deny. Pi's own TUI confirmation is the
// fallback when Atoll is unreachable (and print/JSON modes fail closed).
//
// Install: Atoll keeps a copy at ~/.pi/agent/extensions/atoll-pi-bridge.js.
// Do not rename the file or remove the marker above.

import { readFileSync } from "node:fs";
import { request as httpRequest } from "node:http";
import { request as httpsRequest } from "node:https";
import { homedir } from "node:os";
import { join } from "node:path";

const APPROVAL_TIMEOUT_MS = Number(process.env.ATOLL_PI_APPROVAL_TIMEOUT_MS ?? 30 * 60 * 1000);
const REPORT_TIMEOUT_MS = Number(process.env.ATOLL_PI_REPORT_TIMEOUT_MS ?? 2_000);
// Atoll rejects permission tool_input over 512 KiB; summarize larger bodies
// here so a big write still produces an approval card instead of a fallback.
const MAX_TOOL_INPUT_BYTES = 512 * 1024;
const DEFAULT_GATED_TOOLS = ["bash", "powershell", "write", "edit"];

function gatedTools() {
  const extra = (process.env.ATOLL_PI_GATED_TOOLS ?? "")
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean);
  return new Set([...DEFAULT_GATED_TOOLS, ...extra]);
}

export function bridgeConfigPath(
  platform = process.platform,
  home = homedir(),
  env = process.env,
) {
  if (platform === "darwin") {
    return join(home, "Library", "Application Support", "Atoll", "bridge.json");
  }
  if (platform === "win32") {
    return join(env.LOCALAPPDATA ?? join(home, "AppData", "Local"), "Atoll", "bridge.json");
  }
  return join(env.XDG_DATA_HOME ?? join(home, ".local", "share"), "Atoll", "bridge.json");
}

/** Read bridge.json on every call: Atoll rewrites the token and port when it
 * restarts, and a cached copy would go stale. */
export function loadBridge() {
  const envUrl = process.env.ATOLL_PI_HOOK_URL;
  const envToken = process.env.ATOLL_PI_HOOK_TOKEN;
  if (envUrl && envToken) return { hookUrl: envUrl, token: envToken };
  try {
    const config = JSON.parse(readFileSync(bridgeConfigPath(), "utf8"));
    const token = envToken ?? config.token;
    if (!token) return undefined;
    const hookUrl =
      envUrl ??
      config.piUrl ??
      (config.port ? `http://127.0.0.1:${config.port}/pi/hook` : undefined);
    return hookUrl ? { hookUrl, token } : undefined;
  } catch {
    return undefined;
  }
}

export function postToBridge(bridge, body, timeoutMs, signal) {
  const url = new URL(bridge.hookUrl);
  const payload = Buffer.from(JSON.stringify(body), "utf8");
  const request = url.protocol === "https:" ? httpsRequest : httpRequest;

  return new Promise((resolve, reject) => {
    const req = request(
      {
        protocol: url.protocol,
        hostname: url.hostname,
        port: url.port || (url.protocol === "https:" ? 443 : 80),
        path: `${url.pathname}${url.search}`,
        method: "POST",
        headers: {
          "content-type": "application/json",
          "content-length": payload.length,
          "x-atoll-hook-token": bridge.token,
        },
      },
      (res) => {
        const chunks = [];
        res.on("data", (chunk) => chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)));
        res.on("end", () => {
          const text = Buffer.concat(chunks).toString("utf8");
          if (!res.statusCode || res.statusCode < 200 || res.statusCode >= 300) {
            reject(new Error(`Atoll bridge HTTP ${res.statusCode}`));
            return;
          }
          try {
            resolve(text ? JSON.parse(text) : {});
          } catch (error) {
            reject(error);
          }
        });
        res.on("error", reject);
      },
    );

    // node:http has no built-in response deadline; global fetch would cap the
    // approval wait at 5 minutes (undici headersTimeout), below Atoll's 30.
    req.setTimeout(timeoutMs, () => req.destroy(new Error(`Atoll bridge timeout after ${timeoutMs}ms`)));
    if (signal?.aborted) {
      req.destroy(new Error("Atoll bridge request aborted"));
      return;
    }
    const onAbort = () => req.destroy(new Error("Atoll bridge request aborted"));
    signal?.addEventListener("abort", onAbort, { once: true });
    req.on("error", (error) => {
      signal?.removeEventListener("abort", onAbort);
      reject(error);
    });
    req.on("close", () => signal?.removeEventListener("abort", onAbort));
    req.end(payload);
  });
}

export function shouldGateTool(toolName) {
  // MCP tools can do anything their server exposes; they pass through Pi's
  // tool pipeline with a documented mcp__<server>__<tool> name.
  return toolName.startsWith("mcp__") || gatedTools().has(toolName);
}

/** Permission cards only need a readable command/path; drop huge write/edit
 * payloads rather than failing the whole request at Atoll's 512 KiB cap. */
export function summarizeToolInput(toolName, input) {
  if (!input || typeof input !== "object") return input ?? null;
  const json = JSON.stringify(input);
  if (Buffer.byteLength(json, "utf8") <= MAX_TOOL_INPUT_BYTES) return input;

  const copy = { ...input };
  for (const key of ["content", "newText", "oldText", "new_string", "old_string", "diff", "patch"]) {
    if (typeof copy[key] === "string") {
      copy[key] = `[${Buffer.byteLength(copy[key], "utf8")} bytes omitted by Atoll]`;
    }
  }
  if (Array.isArray(copy.edits)) {
    copy.edits = copy.edits.map((edit) => {
      if (!edit || typeof edit !== "object") return edit;
      const summarized = { ...edit };
      for (const key of ["oldText", "newText"]) {
        if (typeof summarized[key] === "string") {
          summarized[key] = `[${Buffer.byteLength(summarized[key], "utf8")} bytes omitted by Atoll]`;
        }
      }
      return summarized;
    });
  }
  if (Buffer.byteLength(JSON.stringify(copy), "utf8") <= MAX_TOOL_INPUT_BYTES) return copy;

  const path = input.file_path ?? input.path ?? input.command;
  return {
    tool: toolName,
    path: typeof path === "string" ? path : undefined,
    note: "Tool input exceeded 512 KiB and was summarized.",
  };
}

export function permissionPayload(event, ctx) {
  return {
    hook_event_name: "PermissionRequest",
    session_id: sessionId(ctx),
    cwd: ctx.cwd,
    model: ctx.model?.id,
    tool_use_id: event.toolCallId,
    tool_name: event.toolName,
    tool_input: summarizeToolInput(event.toolName, event.input),
  };
}

export function mapApprovalResponse(response, toolName) {
  if (response?.decision === "allow") return undefined;
  if (response?.decision === "deny") {
    return {
      block: true,
      reason: typeof response.reason === "string" ? response.reason : "Denied from Atoll",
    };
  }
  return { fallback: true, reason: `Atoll returned no decision for ${toolName}.` };
}

export function usagePayload(usage) {
  if (!usage) return undefined;
  return {
    input_tokens: usage.input ?? 0,
    output_tokens: usage.output ?? 0,
    cache_read_tokens: usage.cacheRead ?? 0,
    cache_creation_tokens: usage.cacheWrite ?? 0,
    total_tokens: usage.totalTokens ?? 0,
    cost_usd: usage.cost?.total ?? 0,
  };
}

function addUsage(totals, usage) {
  const parsed = usagePayload(usage);
  if (!parsed) return;
  totals.input_tokens += parsed.input_tokens;
  totals.output_tokens += parsed.output_tokens;
  totals.cache_read_tokens += parsed.cache_read_tokens;
  totals.cache_creation_tokens += parsed.cache_creation_tokens;
  totals.total_tokens += parsed.total_tokens;
  totals.cost_usd += parsed.cost_usd;
}

function emptyUsage() {
  return {
    input_tokens: 0,
    output_tokens: 0,
    cache_read_tokens: 0,
    cache_creation_tokens: 0,
    total_tokens: 0,
    cost_usd: 0,
  };
}

/** Usage of the turn that just ended: the assistant message plus any nested
 * tool usage (the subagent example reports child usage there). */
export function turnUsage(event) {
  const totals = emptyUsage();
  if (event.message?.role === "assistant") addUsage(totals, event.message.usage);
  for (const result of event.toolResults ?? []) addUsage(totals, result.usage);
  return totals;
}

/** Cumulative usage of the active branch, mirroring Pi's getSessionStats:
 * assistant messages, standalone usage entries, and compaction summaries. */
export function sessionUsage(ctx) {
  const totals = emptyUsage();
  for (const entry of ctx.sessionManager?.getBranch?.() ?? []) {
    if (entry.type === "message") {
      if (entry.message?.role === "assistant") addUsage(totals, entry.message.usage);
      else if (entry.message?.role === "toolResult") addUsage(totals, entry.message.usage);
    } else if (entry.type === "usage" || entry.type === "compaction" || entry.type === "branch_summary") {
      addUsage(totals, entry.usage);
    }
  }
  return totals;
}

function sessionId(ctx) {
  return ctx.sessionManager?.getSessionId?.() ?? "pi";
}

function setStatus(ctx, text) {
  if (!ctx.hasUI) return;
  try {
    ctx.ui.setStatus("atoll", text);
  } catch {
    // Status text is cosmetic; never break a tool call over it.
  }
}

async function report(bridge, eventName, payload) {
  try {
    await postToBridge(bridge, { hook_event_name: eventName, ...payload }, REPORT_TIMEOUT_MS);
  } catch {
    // Telemetry is best effort: a missed usage/session event is not worth
    // interrupting the agent.
  }
}

/** Ask Pi's own UI when Atoll cannot answer. Fail closed everywhere else. */
async function fallbackDecision(ctx, reason, toolName) {
  if (process.env.ATOLL_PI_FAIL_OPEN === "1") return undefined;
  if (ctx.hasUI && typeof ctx.ui?.confirm === "function") {
    try {
      const allowed = await ctx.ui.confirm(
        "Atoll approval unavailable",
        `${reason}\n\nAllow ${toolName} to run without Atoll?`,
        { signal: ctx.signal },
      );
      if (allowed) return undefined;
      return { block: true, reason: "Denied in Pi" };
    } catch {
      // Fall through to the fail-closed answer.
    }
  }
  return { block: true, reason };
}

export default function atollPiBridge(pi) {
  pi.on("session_start", (event, ctx) => {
    const bridge = loadBridge();
    setStatus(ctx, bridge ? "Atoll: connected" : "Atoll: not running");
    if (!bridge) return;
    void report(bridge, "SessionStart", {
      session_id: sessionId(ctx),
      cwd: ctx.cwd,
      model: ctx.model?.id,
      reason: event.reason,
    });
  });

  pi.on("tool_call", async (event, ctx) => {
    if (!shouldGateTool(event.toolName)) return undefined;
    const bridge = loadBridge();
    if (!bridge) {
      return fallbackDecision(ctx, "Atoll is not running (bridge.json not found).", event.toolName);
    }

    try {
      setStatus(ctx, "Atoll: waiting for approval");
      const response = await postToBridge(
        bridge,
        permissionPayload(event, ctx),
        APPROVAL_TIMEOUT_MS,
        ctx.signal,
      );
      const mapped = mapApprovalResponse(response, event.toolName);
      if (!mapped?.fallback) return mapped;
      return fallbackDecision(ctx, mapped.reason, event.toolName);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return fallbackDecision(ctx, `Atoll approval failed: ${message}`, event.toolName);
    } finally {
      setStatus(ctx, "Atoll: connected");
    }
  });

  pi.on("tool_result", (event, ctx) => {
    if (!shouldGateTool(event.toolName)) return;
    const bridge = loadBridge();
    if (!bridge) return;
    void report(bridge, "PostToolUse", {
      session_id: sessionId(ctx),
      cwd: ctx.cwd,
      model: ctx.model?.id,
      tool_name: event.toolName,
      tool_use_id: event.toolCallId,
      tool_input: summarizeToolInput(event.toolName, event.input),
      is_error: event.isError === true,
    });
  });

  pi.on("turn_end", (event, ctx) => {
    const bridge = loadBridge();
    if (!bridge) return;
    void report(bridge, "AfterAgentResponse", {
      session_id: sessionId(ctx),
      cwd: ctx.cwd,
      model: ctx.model?.id,
      token_usage: turnUsage(event),
      usage_mode: "delta",
    });
  });

  pi.on("agent_end", (event, ctx) => {
    const bridge = loadBridge();
    if (!bridge) return;
    // Cumulative so retries/compaction are counted once; Atoll takes a
    // component-wise max for this event.
    void report(bridge, "Stop", {
      session_id: sessionId(ctx),
      cwd: ctx.cwd,
      model: ctx.model?.id,
      aborted: event.aborted === true,
      token_usage: sessionUsage(ctx),
      usage_mode: "cumulative",
    });
  });

  pi.on("session_shutdown", (event, ctx) => {
    // "reload" only tears down the extension runtime; the session continues.
    if (event.reason === "reload") return;
    setStatus(ctx, undefined);
    const bridge = loadBridge();
    if (!bridge) return;
    void report(bridge, "sessionEnd", {
      session_id: sessionId(ctx),
      cwd: ctx.cwd,
      model: ctx.model?.id,
      reason: event.reason,
      token_usage: sessionUsage(ctx),
      usage_mode: "cumulative",
    });
  });
}
