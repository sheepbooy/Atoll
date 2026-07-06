import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  bridgeConfigPath,
  hookAuthHeaders,
  readBridgeConfig,
  resolveHookConfig,
  resolveHookUrl,
} from "./atoll-hook-bridge.mjs";

const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-bridge-test-"));
const originalLocalAppData = process.env.LOCALAPPDATA;
const originalHome = process.env.HOME;

try {
  process.env.LOCALAPPDATA = tempRoot;
  process.env.HOME = tempRoot;
  delete process.env.ATOLL_HOOK_URL;

  const configPath = bridgeConfigPath();
  fs.mkdirSync(path.dirname(configPath), { recursive: true });
  fs.writeFileSync(
    configPath,
    JSON.stringify({
      port: 47778,
      claudeUrl: "http://127.0.0.1:47778/claude/pre-tool-use",
      codexUrl: "http://127.0.0.1:47778/codex/hook",
      cursorUrl: "http://127.0.0.1:47778/cursor/hook",
      token: "bridge-token",
    }),
  );

  assert.deepEqual(readBridgeConfig(), {
    port: 47778,
    claudeUrl: "http://127.0.0.1:47778/claude/pre-tool-use",
    codexUrl: "http://127.0.0.1:47778/codex/hook",
    cursorUrl: "http://127.0.0.1:47778/cursor/hook",
    token: "bridge-token",
  });
  assert.deepEqual(
    resolveHookConfig("claudeUrl", "http://127.0.0.1:47777/claude/pre-tool-use"),
    {
      url: "http://127.0.0.1:47778/claude/pre-tool-use",
      token: "bridge-token",
    },
  );
  assert.deepEqual(hookAuthHeaders("bridge-token"), {
    "x-atoll-hook-token": "bridge-token",
  });
  assert.deepEqual(hookAuthHeaders(null), {});
  assert.equal(
    resolveHookUrl("claudeUrl", "http://127.0.0.1:47777/claude/pre-tool-use"),
    "http://127.0.0.1:47778/claude/pre-tool-use",
  );
  assert.equal(
    resolveHookUrl("codexUrl", "http://127.0.0.1:47777/codex/hook"),
    "http://127.0.0.1:47778/codex/hook",
  );
  assert.equal(
    resolveHookUrl("cursorUrl", "http://127.0.0.1:47777/cursor/hook"),
    "http://127.0.0.1:47778/cursor/hook",
  );

  process.env.ATOLL_HOOK_URL = "http://127.0.0.1:49999/custom";
  assert.equal(
    resolveHookUrl("claudeUrl", "http://127.0.0.1:47777/claude/pre-tool-use"),
    "http://127.0.0.1:47778/claude/pre-tool-use",
  );

  fs.unlinkSync(configPath);
  process.env.ATOLL_HOOK_TOKEN = "env-token";
  assert.deepEqual(
    resolveHookConfig("claudeUrl", "http://127.0.0.1:47777/claude/pre-tool-use"),
    {
      url: "http://127.0.0.1:49999/custom",
      token: "env-token",
    },
  );
  assert.equal(
    resolveHookUrl("claudeUrl", "http://127.0.0.1:47777/claude/pre-tool-use"),
    "http://127.0.0.1:49999/custom",
  );
} finally {
  if (originalLocalAppData === undefined) {
    delete process.env.LOCALAPPDATA;
  } else {
    process.env.LOCALAPPDATA = originalLocalAppData;
  }
  if (originalHome === undefined) {
    delete process.env.HOME;
  } else {
    process.env.HOME = originalHome;
  }
  delete process.env.ATOLL_HOOK_URL;
  delete process.env.ATOLL_HOOK_TOKEN;
  fs.rmSync(tempRoot, { recursive: true, force: true });
}
