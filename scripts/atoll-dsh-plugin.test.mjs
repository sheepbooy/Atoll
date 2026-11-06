import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

import {
  BRIDGE_MARKER,
  apply,
  bridgeConfigPath,
  buildApprovalPayload,
  displayToolName,
  mapDecisionToOutcome,
  normalizeDecision,
  parentSessionId,
  parseToolArguments,
  resolveAtollConfig,
  summarizeToolInput,
} from "./dsh-plugin/index.mjs";

function makeHome(t) {
  const home = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-dsh-test-"));
  const previous = {
    HOME: process.env.HOME,
    ATOLL_DSH_HOOK_URL: process.env.ATOLL_DSH_HOOK_URL,
    ATOLL_HOOK_URL: process.env.ATOLL_HOOK_URL,
    ATOLL_HOOK_TOKEN: process.env.ATOLL_HOOK_TOKEN,
  };
  process.env.HOME = home;
  delete process.env.ATOLL_DSH_HOOK_URL;
  delete process.env.ATOLL_HOOK_URL;
  delete process.env.ATOLL_HOOK_TOKEN;
  t.after(() => {
    for (const [key, value] of Object.entries(previous)) {
      if (value === undefined) delete process.env[key];
      else process.env[key] = value;
    }
    fs.rmSync(home, { recursive: true, force: true });
  });
  return home;
}

function createCtx({ webServer = true } = {}) {
  const listeners = new Map();
  const ctx = {
    on(name, callback, options) {
      const entries = listeners.get(name) ?? [];
      entries.push({ callback, options });
      listeners.set(name, entries);
      return () => {};
    },
    inject(dependencies, callback) {
      if (webServer) callback(ctx);
    },
  };
  return { ctx, listeners };
}

function listenerFor(listeners, name) {
  const entries = listeners.get(name);
  assert.ok(entries && entries.length > 0, `missing listener for ${name}`);
  return entries[entries.length - 1].callback;
}

function makeSession({
  id = "session-1",
  cwd = "/repo",
  origin,
  parentSession,
} = {}) {
  return {
    id,
    header: {
      id,
      cwd,
      ...(origin ? { origin } : {}),
      ...(parentSession ? { parentSession } : {}),
    },
  };
}

function makeApproval({
  session = makeSession(),
  toolName = "bash",
  callId = "call-1",
  reason,
  signal,
} = {}) {
  return {
    agent: { session },
    toolName,
    ...(callId === undefined ? {} : { callId }),
    ...(reason ? { reason } : {}),
    ...(signal ? { signal } : {}),
  };
}

async function startServer(handler) {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    port,
    close: () => new Promise((resolve) => server.close(resolve)),
  };
}

function collectBody(request) {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      try {
        resolve(JSON.parse(body));
      } catch (error) {
        reject(error);
      }
    });
    request.on("error", reject);
  });
}

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

test("normalizes DSH tool names and summarises sensitive inputs", () => {
  assert.equal(displayToolName("bash"), "Bash");
  assert.equal(displayToolName("write"), "Write");
  assert.equal(displayToolName("custom-mcp_tool"), "custom-mcp_tool");
  assert.equal(displayToolName(""), "approval");

  const longCommand = "x".repeat(9000);
  const summary = summarizeToolInput({
    command: longCommand,
    description: "Run a command",
    content: "secret file body",
    path: "/repo/file.txt",
    nested: { secret: true },
  });
  assert.equal(summary.description, "Run a command");
  assert.equal(summary.path, "/repo/file.txt");
  assert.equal(summary.content, undefined);
  assert.equal(summary.nested, undefined);
  assert.equal(summary.command.length, 8 * 1024 + "… [truncated]".length);
  assert.ok(summary.command.endsWith("… [truncated]"));

  assert.deepEqual(parseToolArguments('{"command":"ls"}'), { command: "ls" });
  assert.deepEqual(parseToolArguments("not json"), {});
  assert.deepEqual(parseToolArguments(null), {});
});

test("maps subagent sessions to their parent and maps decisions", () => {
  const root = makeSession({ id: "root" });
  const child = makeSession({ id: "child", origin: "subagent", parentSession: "root" });

  assert.equal(parentSessionId(root), null);
  assert.equal(parentSessionId(child), "root");

  const payload = buildApprovalPayload(
    {
      agent: { session: child },
      toolName: "bash",
      callId: "call-9",
      reason: "sandbox escalation",
    },
    { name: "bash", input: { command: "ls", description: "List files" } },
  );
  assert.equal(payload.session_id, "root");
  assert.equal(payload.subagent_session_id, "child");
  assert.equal(payload.tool_use_id, "call-9");
  assert.equal(payload.tool_name, "Bash");
  assert.match(payload.tool_input.description, /List files/);
  assert.match(payload.tool_input.description, /sandbox escalation/);

  assert.equal(normalizeDecision({ decision: "allow" }), "allow");
  assert.equal(normalizeDecision({ decision: "deny" }), "deny");
  assert.equal(normalizeDecision({}), null);
  assert.equal(mapDecisionToOutcome("allow"), "allowed-once");
  assert.equal(mapDecisionToOutcome("deny"), "rejected");
  assert.equal(mapDecisionToOutcome(null), null);
});

test("bridge config path matches Atoll per platform", () => {
  assert.equal(
    bridgeConfigPath("darwin", "/home/u", {}),
    "/home/u/Library/Application Support/Atoll/bridge.json",
  );
  assert.equal(
    bridgeConfigPath("linux", "/home/u", { XDG_DATA_HOME: "/xdg" }),
    "/xdg/Atoll/bridge.json",
  );
  assert.equal(
    bridgeConfigPath("win32", "/home/u", { LOCALAPPDATA: "C:\\\\Users\\\\u\\\\AppData\\\\Local" }),
    path.join("C:\\\\Users\\\\u\\\\AppData\\\\Local", "Atoll", "bridge.json"),
  );
});

test("does nothing without a webServer surface", () => {
  const { ctx, listeners } = createCtx({ webServer: false });
  apply(ctx);
  assert.equal(listeners.size, 0);
});

test("allow response becomes allowed-once and carries tool arguments", async (t) => {
  makeHome(t);
  const captured = [];
  const server = await startServer(async (request, response) => {
    const body = await collectBody(request);
    captured.push({ url: request.url, token: request.headers["x-atoll-hook-token"], body });
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(body.hook_event_name === "PermissionRequest" ? { decision: "allow" } : {}));
  });
  t.after(server.close);
  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;
  process.env.ATOLL_HOOK_TOKEN = "test-token";

  const { ctx, listeners } = createCtx();
  apply(ctx);
  const session = makeSession({ id: "session-allow" });
  listenerFor(listeners, "session/created")(session);
  listenerFor(listeners, "session/event")(session, {
    type: "tool/call",
    data: { callId: "call-allow", name: "bash", arguments: JSON.stringify({ command: "echo ok" }) },
  });

  const outcome = await listenerFor(listeners, "approval/request")(
    makeApproval({ session, callId: "call-allow", reason: "sandbox escalation" }),
    () => "delegated",
  );
  assert.equal(outcome, "allowed-once");

  const permission = captured.find((entry) => entry.body.hook_event_name === "PermissionRequest");
  assert.ok(permission);
  assert.equal(permission.url, "/dsh/hook");
  assert.equal(permission.token, "test-token");
  assert.equal(permission.body.session_id, "session-allow");
  assert.equal(permission.body.tool_use_id, "call-allow");
  assert.equal(permission.body.tool_name, "Bash");
  assert.equal(permission.body.tool_input.command, "echo ok");
  assert.equal(permission.body.cwd, "/repo");
});

test("deny response becomes rejected", async (t) => {
  makeHome(t);
  const server = await startServer(async (request, response) => {
    await collectBody(request);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ decision: "deny", reason: "not today" }));
  });
  t.after(server.close);
  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;

  const { ctx, listeners } = createCtx();
  apply(ctx);
  const outcome = await listenerFor(listeners, "approval/request")(
    makeApproval(),
    () => "delegated",
  );
  assert.equal(outcome, "rejected");
});

test("empty and failed bridge responses delegate to DSH", async (t) => {
  makeHome(t);
  const server = await startServer(async (request, response) => {
    await collectBody(request);
    response.writeHead(200, { "content-type": "application/json" });
    response.end("{}");
  });
  t.after(server.close);
  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;

  const first = createCtx();
  apply(first.ctx);
  assert.equal(
    await listenerFor(first.listeners, "approval/request")(makeApproval(), () => "delegated"),
    "delegated",
  );

  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;
  const second = createCtx();
  apply(second.ctx);
  process.env.ATOLL_DSH_HOOK_URL = "http://127.0.0.1:1/dsh/hook";
  const failed = await listenerFor(second.listeners, "approval/request")(
    makeApproval(),
    () => "delegated",
  );
  assert.equal(failed, "delegated");
});

test("no bridge config leaves DSH approvals untouched", async (t) => {
  makeHome(t);
  const { ctx, listeners } = createCtx();
  apply(ctx);
  assert.equal(
    await listenerFor(listeners, "approval/request")(makeApproval(), () => "delegated"),
    "delegated",
  );
});

test("aborted approval requests return cancelled", async (t) => {
  makeHome(t);
  const server = await startServer(async (request, response) => {
    await collectBody(request);
    await wait(500);
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ decision: "allow" }));
  });
  t.after(server.close);
  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;

  const { ctx, listeners } = createCtx();
  apply(ctx);
  const controller = new AbortController();
  const pending = listenerFor(listeners, "approval/request")(
    makeApproval({ signal: controller.signal }),
    () => "delegated",
  );
  controller.abort();
  assert.equal(await pending, "cancelled");
});

test("forwards root session lifecycle and suppresses subagent observers", async (t) => {
  makeHome(t);
  const events = [];
  const server = await startServer(async (request, response) => {
    const body = await collectBody(request);
    events.push(body);
    response.writeHead(200, { "content-type": "application/json" });
    response.end("{}");
  });
  t.after(server.close);
  process.env.ATOLL_DSH_HOOK_URL = `http://127.0.0.1:${server.port}/dsh/hook`;

  const { ctx, listeners } = createCtx();
  apply(ctx);
  const root = makeSession({ id: "root-session", cwd: "/project" });
  const child = makeSession({ id: "child-session", cwd: "/project", origin: "subagent", parentSession: "root-session" });

  listenerFor(listeners, "session/created")(root);
  listenerFor(listeners, "session/created")(child);
  listenerFor(listeners, "session/event")(root, { type: "turn/start", data: { turn: 1 } });
  listenerFor(listeners, "session/event")(root, {
    type: "tool/result",
    data: {
      turn: 1,
      step: 1,
      message: { content: [{ type: "tool-result", toolCallId: "call-1", content: [] }] },
    },
  });
  listenerFor(listeners, "session/event")(root, { type: "turn/end", data: { turn: 1, reason: "completed" } });
  listenerFor(listeners, "session/disposed")(root);

  await wait(100);
  const names = events.map((event) => event.hook_event_name);
  assert.ok(names.includes("SessionStart"));
  assert.ok(names.includes("UserPromptSubmit"));
  assert.ok(names.includes("PostToolUse"));
  assert.ok(names.includes("Stop"));
  assert.ok(names.includes("SessionEnd"));
  assert.equal(events.filter((event) => event.session_id === "child-session").length, 0);
});

test("shipped plugin carries the install marker", () => {
  const source = fs.readFileSync(new URL("./dsh-plugin/index.mjs", import.meta.url), "utf8");
  assert.ok(source.includes(BRIDGE_MARKER));
});

test("resolveAtollConfig falls back to env and default bridge port", (t) => {
  const home = makeHome(t);
  assert.equal(resolveAtollConfig(process.env, "darwin", home), null);

  fs.mkdirSync(path.join(home, "Library", "Application Support", "Atoll"), { recursive: true });
  fs.writeFileSync(
    path.join(home, "Library", "Application Support", "Atoll", "bridge.json"),
    JSON.stringify({ token: "bridge-token" }),
  );
  assert.deepEqual(resolveAtollConfig(process.env, "darwin", home), {
    url: "http://127.0.0.1:47777/dsh/hook",
    token: "bridge-token",
  });

  process.env.ATOLL_DSH_HOOK_URL = "http://127.0.0.1:9/custom";
  process.env.ATOLL_HOOK_TOKEN = "env-token";
  assert.deepEqual(resolveAtollConfig(process.env, "darwin", home), {
    url: "http://127.0.0.1:9/custom",
    token: "env-token",
  });
});
