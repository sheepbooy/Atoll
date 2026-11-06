import assert from "node:assert/strict";
import { createServer } from "node:http";
import atollPiBridge, {
  bridgeConfigPath,
  mapApprovalResponse,
  permissionPayload,
  sessionUsage,
  shouldGateTool,
  summarizeToolInput,
  turnUsage,
  usagePayload,
} from "./atoll-pi-bridge.js";

// ─── bridge config path ──────────────────────────────────────────────────

assert.equal(
  bridgeConfigPath("darwin", "/home/u", {}),
  "/home/u/Library/Application Support/Atoll/bridge.json",
);
assert.equal(
  bridgeConfigPath("win32", "/home/u", { LOCALAPPDATA: "C:\\Users\\u\\AppData\\Local" }),
  ["C:\\Users\\u\\AppData\\Local", "Atoll", "bridge.json"].join("/"),
);
assert.equal(
  bridgeConfigPath("linux", "/home/u", { XDG_DATA_HOME: "/xdg" }),
  "/xdg/Atoll/bridge.json",
);

// ─── gated tools ─────────────────────────────────────────────────────────

for (const tool of ["bash", "powershell", "write", "edit", "mcp__server__tool"]) {
  assert.equal(shouldGateTool(tool), true, `${tool} must require approval`);
}
for (const tool of ["read", "grep", "find", "ls"]) {
  assert.equal(shouldGateTool(tool), false, `${tool} is read-only/ungated`);
}

// ─── permission payload ──────────────────────────────────────────────────

const ctx = {
  cwd: "/repo",
  model: { id: "claude-sonnet-5-5" },
  sessionManager: { getSessionId: () => "pi-session-1" },
};
assert.deepEqual(
  permissionPayload(
    { toolCallId: "call-1", toolName: "bash", input: { command: "npm test" } },
    ctx,
  ),
  {
    hook_event_name: "PermissionRequest",
    session_id: "pi-session-1",
    cwd: "/repo",
    model: "claude-sonnet-5-5",
    tool_use_id: "call-1",
    tool_name: "bash",
    tool_input: { command: "npm test" },
  },
);

// Oversized write payloads are summarized, keeping the path readable.
const huge = summarizeToolInput("write", {
  path: "/repo/big.txt",
  content: "x".repeat(600 * 1024),
});
assert.equal(huge.path, "/repo/big.txt");
assert.equal(typeof huge.content, "string");
assert.ok(huge.content.includes("omitted"));
assert.ok(Buffer.byteLength(JSON.stringify(huge), "utf8") < 512 * 1024);

// ─── decision mapping ────────────────────────────────────────────────────

assert.equal(mapApprovalResponse({ decision: "allow" }, "bash"), undefined);
assert.deepEqual(mapApprovalResponse({ decision: "deny", reason: "no" }, "bash"), {
  block: true,
  reason: "no",
});
assert.deepEqual(mapApprovalResponse({ decision: "deny" }, "bash"), {
  block: true,
  reason: "Denied from Atoll",
});
assert.deepEqual(mapApprovalResponse({}, "bash"), {
  fallback: true,
  reason: "Atoll returned no decision for bash.",
});

// ─── usage ───────────────────────────────────────────────────────────────

assert.deepEqual(
  usagePayload({
    input: 10,
    output: 20,
    cacheRead: 30,
    cacheWrite: 40,
    totalTokens: 100,
    cost: { total: 0.01 },
  }),
  {
    input_tokens: 10,
    output_tokens: 20,
    cache_read_tokens: 30,
    cache_creation_tokens: 40,
    total_tokens: 100,
    cost_usd: 0.01,
  },
);
assert.equal(usagePayload(undefined), undefined);

assert.deepEqual(
  turnUsage({
    message: { role: "assistant", usage: { input: 1, output: 2, cacheRead: 3, cacheWrite: 4, totalTokens: 10, cost: { total: 0 } } },
    toolResults: [
      { usage: { input: 5, output: 6, cacheRead: 7, cacheWrite: 8, totalTokens: 26, cost: { total: 0 } } },
    ],
  }).input_tokens,
  6,
);

const branchUsage = sessionUsage({
  sessionManager: {
    getBranch: () => [
      { type: "message", message: { role: "assistant", usage: { input: 1, output: 2, cacheRead: 0, cacheWrite: 0, totalTokens: 3 } } },
      { type: "message", message: { role: "user", content: "hi" } },
      { type: "message", message: { role: "toolResult", usage: { input: 3, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 3 } } },
      { type: "usage", usage: { input: 4, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 4 } },
      { type: "compaction", usage: { input: 5, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 5 } },
    ],
  },
});
assert.equal(branchUsage.input_tokens, 13);
assert.equal(branchUsage.output_tokens, 2);

// ─── approval round-trip ─────────────────────────────────────────────────

const requests = [];
let responseBody = { decision: "deny", reason: "nope" };
const server = createServer((req, res) => {
  const chunks = [];
  req.on("data", (chunk) => chunks.push(chunk));
  req.on("end", () => {
    requests.push({
      headers: req.headers,
      path: req.url,
      body: Buffer.concat(chunks).toString("utf8"),
    });
    res.setHeader("content-type", "application/json");
    res.end(JSON.stringify(responseBody));
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const { port } = server.address();
process.env.ATOLL_PI_HOOK_URL = `http://127.0.0.1:${port}/pi/hook`;
process.env.ATOLL_PI_HOOK_TOKEN = "test-token";

const handlers = new Map();
atollPiBridge({ on: (name, handler) => handlers.set(name, handler) });
const baseCtx = {
  cwd: "/repo",
  hasUI: false,
  mode: "json",
  model: { id: "pi-model" },
  sessionManager: { getSessionId: () => "pi-session-1" },
};
const toolCall = (toolName = "bash", input = { command: "rm -rf build" }) => ({
  toolCallId: "call-1",
  toolName,
  input,
});

// Pi supplies hasUI, not a mode field, and emits agent_end for each loop.
const statuses = [];
const uiCtx = { ...baseCtx, hasUI: true, ui: { setStatus: (key, value) => statuses.push([key, value]) } };
handlers.get("session_start")({ reason: "new" }, uiCtx);
assert.deepEqual(statuses[0], ["atoll", "Atoll: connected"]);
assert.equal(handlers.has("agent_end"), true);
assert.equal(handlers.has("agent_settled"), false);
handlers.get("agent_end")({ messages: [] }, baseCtx);
for (let i = 0; i < 50 && !requests.some((r) => JSON.parse(r.body).hook_event_name === "Stop"); i++) {
  await new Promise((resolve) => setTimeout(resolve, 10));
}
assert.ok(requests.some((r) => JSON.parse(r.body).hook_event_name === "Stop"));

const denied = await handlers.get("tool_call")(toolCall(), baseCtx);
assert.deepEqual(denied, { block: true, reason: "nope" });
assert.equal(requests.at(-1).path, "/pi/hook");
assert.equal(requests.at(-1).headers["x-atoll-hook-token"], "test-token");
const posted = JSON.parse(requests.at(-1).body);
assert.equal(posted.hook_event_name, "PermissionRequest");
assert.equal(posted.session_id, "pi-session-1");
assert.equal(posted.tool_name, "bash");
assert.equal(posted.tool_input.command, "rm -rf build");

responseBody = { decision: "allow" };
assert.equal(await handlers.get("tool_call")(toolCall("write", { path: "/repo/a" }), baseCtx), undefined);

// No decision from Atoll → Pi's own TUI confirmation, never a silent allow.
responseBody = {};
const countBeforeFallback = requests.length;
const allowedByTui = await handlers.get("tool_call")(toolCall(), {
  ...baseCtx,
  hasUI: true,
  mode: "tui",
  ui: { confirm: async () => true },
});
assert.equal(allowedByTui, undefined);
assert.equal(requests.length, countBeforeFallback + 1);

const deniedByTui = await handlers.get("tool_call")(toolCall(), {
  ...baseCtx,
  hasUI: true,
  mode: "tui",
  ui: { confirm: async () => false },
});
assert.deepEqual(deniedByTui, { block: true, reason: "Denied in Pi" });

// Read-only tools never reach the bridge.
const countBeforeRead = requests.length;
assert.equal(await handlers.get("tool_call")(toolCall("read", { path: "/repo/a" }), baseCtx), undefined);
assert.equal(requests.length, countBeforeRead);

server.close();

console.log("atoll-pi-bridge.test.mjs: all assertions passed");
