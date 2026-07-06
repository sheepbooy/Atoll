import assert from "node:assert/strict";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const payload = {
  session_id: "session-test",
  cwd: "/tmp/project",
  hook_event_name: "PreToolUse",
  tool_name: "Bash",
  tool_input: {
    command: "echo from-test",
    description: "Echo from test",
  },
  tool_use_id: "tool-test",
};

const expectedResponse = {
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    permissionDecision: "allow",
    permissionDecisionReason: "Approved from test",
  },
};

const server = http.createServer((request, response) => {
  let body = "";
  request.setEncoding("utf8");
  request.on("data", (chunk) => {
    body += chunk;
  });
  request.on("end", () => {
    assert.equal(request.method, "POST");
    assert.equal(request.url, "/claude/pre-tool-use");
    assert.deepEqual(JSON.parse(body), payload);

    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(expectedResponse));
  });
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

try {
  const { port } = server.address();
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-claude-url-test-"));
  const child = spawn(process.execPath, ["scripts/atoll-claude-hook.mjs"], {
    env: {
      ...process.env,
      HOME: tempHome,
      LOCALAPPDATA: tempHome,
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/claude/pre-tool-use`,
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  child.stdin.end(JSON.stringify(payload));

  const [stdout, stderr, exitCode] = await Promise.all([
    readStream(child.stdout),
    readStream(child.stderr),
    new Promise((resolve) => child.on("close", resolve)),
  ]);

  assert.equal(stderr, "");
  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), expectedResponse);
  fs.rmSync(tempHome, { recursive: true, force: true });
} finally {
  await new Promise((resolve) => server.close(resolve));
}

// Uses bridge.json when ATOLL_HOOK_URL is unset.
{
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-claude-bridge-test-"));
  const originalLocalAppData = process.env.LOCALAPPDATA;
  const originalHome = process.env.HOME;

  const bridgeServer = http.createServer((request, response) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      assert.equal(request.url, "/claude/pre-tool-use");
      assert.equal(request.headers["x-atoll-hook-token"], "bridge-token");
      assert.deepEqual(JSON.parse(body), payload);
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(expectedResponse));
    });
  });

  await new Promise((resolve) => bridgeServer.listen(0, "127.0.0.1", resolve));

  try {
    process.env.LOCALAPPDATA = tempRoot;
    process.env.HOME = tempRoot;
    delete process.env.ATOLL_HOOK_URL;

    const { port } = bridgeServer.address();
    const configDir =
      process.platform === "darwin"
        ? path.join(tempRoot, "Library", "Application Support", "Atoll")
        : process.platform === "win32"
          ? path.join(tempRoot, "Atoll")
          : path.join(tempRoot, ".local", "share", "Atoll");
    fs.mkdirSync(configDir, { recursive: true });
    fs.writeFileSync(
      path.join(configDir, "bridge.json"),
      JSON.stringify({
        port,
        claudeUrl: `http://127.0.0.1:${port}/claude/pre-tool-use`,
        codexUrl: `http://127.0.0.1:${port}/codex/hook`,
        token: "bridge-token",
      }),
    );

    const child = spawn(process.execPath, ["scripts/atoll-claude-hook.mjs"], {
      env: { ...process.env },
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stdin.end(JSON.stringify(payload));

    const [stdout, stderr, exitCode] = await Promise.all([
      readStream(child.stdout),
      readStream(child.stderr),
      new Promise((resolve) => child.on("close", resolve)),
    ]);

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(stdout), expectedResponse);
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
    await new Promise((resolve) => bridgeServer.close(resolve));
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function readStream(stream) {
  return new Promise((resolve, reject) => {
    let value = "";
    stream.setEncoding("utf8");
    stream.on("data", (chunk) => {
      value += chunk;
    });
    stream.on("end", () => resolve(value));
    stream.on("error", reject);
  });
}
