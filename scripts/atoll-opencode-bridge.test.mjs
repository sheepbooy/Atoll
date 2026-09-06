import assert from "node:assert/strict";
import {
  bridgeConfigPath,
  mapDecisionToResponse,
  normalizePermissionEvent,
  permissionToolName,
} from "./atoll-opencode-bridge.js";

// ─── event normalization ─────────────────────────────────────────────────

assert.deepEqual(
  normalizePermissionEvent({
    type: "permission.updated",
    properties: {
      id: "perm-1",
      sessionID: "ses-1",
      type: "bash",
      title: "Run tests",
      pattern: "npm test*",
      metadata: { command: "npm test" },
      directory: "/repo",
    },
  }),
  {
    hook_event_name: "PermissionRequest",
    session_id: "ses-1",
    tool_use_id: "perm-1",
    tool_name: "Bash",
    tool_input: {
      command: "npm test",
      pattern: "npm test*",
      description: "Run tests",
    },
    cwd: "/repo",
  },
);

// permission.asked (older bus name) normalizes the same way.
const asked = normalizePermissionEvent({
  type: "permission.asked",
  properties: { id: "perm-2", sessionID: "ses-2", type: "edit" },
});
assert.equal(asked.hook_event_name, "PermissionRequest");
assert.equal(asked.tool_name, "edit");

// Real-world 1.18.29 `permission.asked` payload: `permission` names the gate,
// `patterns`/`always` are arrays, and `tool` is a call-reference OBJECT —
// which must not leak into the tool label.
const realWorld = normalizePermissionEvent({
  type: "permission.asked",
  properties: {
    id: "per_072b75abe001",
    sessionID: "ses_f8d48aa00ffe",
    permission: "bash",
    tool: { messageID: "msg_1", callID: "call_1" },
    patterns: ["echo atoll-e2e"],
    always: ["echo *"],
    metadata: { command: "echo atoll-e2e" },
  },
});
assert.equal(realWorld.tool_name, "Bash");
assert.equal(realWorld.session_id, "ses_f8d48aa00ffe");
assert.equal(realWorld.tool_use_id, "per_072b75abe001");
assert.equal(realWorld.tool_input.command, "echo atoll-e2e");
assert.equal(realWorld.tool_input.pattern, "echo atoll-e2e");
assert.equal(realWorld.cwd, ".");

// Non-permission events and malformed payloads never surface a card.
assert.equal(normalizePermissionEvent({ type: "session.idle", properties: {} }), null);
assert.equal(normalizePermissionEvent(null), null);
assert.equal(
  normalizePermissionEvent({ type: "permission.updated", properties: { sessionID: "ses-1" } }),
  null,
);
assert.equal(normalizePermissionEvent({ type: "permission.updated" }), null);

// Array patterns join into a readable single string.
const multiPattern = normalizePermissionEvent({
  type: "permission.updated",
  properties: {
    id: "perm-3",
    sessionID: "ses-3",
    type: "bash",
    pattern: ["git status*", "git diff*"],
  },
});
assert.equal(multiPattern.tool_input.pattern, "git status*, git diff*");

// ─── tool naming ──────────────────────────────────────────────────────────

assert.equal(permissionToolName("bash"), "Bash");
assert.equal(permissionToolName("edit"), "edit");
assert.equal(permissionToolName(undefined), "approval");

// ─── decision mapping ────────────────────────────────────────────────────

assert.equal(mapDecisionToResponse("allow"), "once");
assert.equal(mapDecisionToResponse("deny"), "reject");
assert.equal(mapDecisionToResponse("anything-else"), "reject");

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
  bridgeConfigPath("win32", "/home/u", {}),
  "/home/u/AppData/Local/Atoll/bridge.json",
);
assert.equal(
  bridgeConfigPath("linux", "/home/u", {}),
  "/home/u/.local/share/Atoll/bridge.json",
);
assert.equal(
  bridgeConfigPath("linux", "/home/u", { XDG_DATA_HOME: "/xdg" }),
  "/xdg/Atoll/bridge.json",
);

console.log("atoll-opencode-bridge.test.mjs: all assertions passed");
