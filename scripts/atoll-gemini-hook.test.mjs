import assert from "node:assert/strict";
import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const permissionPayload = {
  session_id: "session-gemini-test",
  transcript_path: "/tmp/gemini-transcript.json",
  cwd: "/tmp/project",
  hook_event_name: "BeforeTool",
  timestamp: "2026-08-30T00:00:00Z",
  tool_name: "run_shell_command",
  tool_input: {
    command: "echo from-gemini-test",
    description: "Echo from Gemini test",
  },
};

const expectedAllowResponse = {
  decision: "allow",
};

const expectedDenyResponse = {
  decision: "deny",
  reason: "Denied from Atoll: not today",
};

function createBridgeServer(expectedToolName, response) {
  return http.createServer((request, response_) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk) => {
      body += chunk;
    });
    request.on("end", () => {
      assert.equal(request.method, "POST");
      assert.equal(request.url, "/gemini/hook");
      assert.equal(request.headers["x-atoll-hook-token"], "env-token");
      const payload = JSON.parse(body);
      assert.equal(payload.tool_name, expectedToolName);
      assert.equal(payload.hook_event_name, "BeforeTool");

      response_.writeHead(200, { "content-type": "application/json" });
      response_.end(JSON.stringify(response));
    });
  });
}

async function runHook(payload, env = {}) {
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-gemini-hook-test-"));
  try {
    const child = spawn(process.execPath, ["scripts/atoll-gemini-hook.mjs"], {
      env: {
        ...process.env,
        HOME: tempHome,
        LOCALAPPDATA: tempHome,
        ...env,
      },
      stdio: ["pipe", "pipe", "pipe"],
    });

    child.stdin.end(JSON.stringify(payload));

    const [stdout, stderr, exitCode] = await Promise.all([
      readStream(child.stdout),
      readStream(child.stderr),
      new Promise((resolve) => child.on("close", resolve)),
    ]);

    return { stdout, stderr, exitCode };
  } finally {
    fs.rmSync(tempHome, { recursive: true, force: true });
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

// 1. Gated BeforeTool forwards the payload and passes the bridge decision through.
{
  const server = createBridgeServer("run_shell_command", expectedAllowResponse);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = server.address();
    const { stdout, stderr, exitCode } = await runHook(permissionPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/gemini/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
    });

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(stdout), expectedAllowResponse);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

// 2. A deny decision from the bridge is passed through verbatim.
{
  const server = createBridgeServer("run_shell_command", expectedDenyResponse);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = server.address();
    const { stdout, stderr, exitCode } = await runHook(permissionPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/gemini/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
    });

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(stdout), expectedDenyResponse);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

// 3. Read-only tools are approved locally without contacting the bridge.
{
  const readPayload = {
    ...permissionPayload,
    tool_name: "read_file",
  };
  const { stdout, stderr, exitCode } = await runHook(readPayload, {
    // Unreachable port: the hook must decide without the bridge.
    ATOLL_HOOK_URL: "http://127.0.0.1:1/gemini/hook",
  });

  assert.equal(stderr, "");
  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), { decision: "allow" });
}

// 4. ATOLL_GEMINI_GATED_TOOLS overrides the default gate pattern.
{
  const readPayload = {
    ...permissionPayload,
    tool_name: "read_file",
  };
  const server = createBridgeServer("read_file", expectedAllowResponse);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = server.address();
    const { stdout, stderr, exitCode } = await runHook(readPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/gemini/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
      ATOLL_GEMINI_GATED_TOOLS: "read_file|run_shell_command",
    });

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(stdout), expectedAllowResponse);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
}

// 5. Observer events degrade to "{}" when Atoll is unavailable.
for (const hookEventName of [
  "SessionStart",
  "SessionEnd",
  "AfterTool",
  "AfterAgent",
  "Notification",
]) {
  const payload = {
    session_id: "session-gemini-observer-test",
    cwd: "/tmp/project",
    hook_event_name: hookEventName,
  };

  const { stdout, stderr, exitCode } = await runHook(payload, {
    ATOLL_HOOK_URL: "http://127.0.0.1:1/gemini/hook",
  });

  assert.equal(stderr, "", hookEventName);
  assert.equal(exitCode, 0, hookEventName);
  assert.deepEqual(JSON.parse(stdout), {}, hookEventName);
}

// 6. Slow observer events fall back fast instead of stalling the Gemini turn.
{
  const stopPayload = {
    session_id: "session-gemini-slow-observer",
    cwd: "/tmp/project",
    hook_event_name: "SessionEnd",
  };
  const slowServer = http.createServer((_request, response) => {
    setTimeout(() => {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({}));
    }, 2000);
  });
  await new Promise((resolve) => slowServer.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = slowServer.address();
    const startedAt = Date.now();
    const { stdout, stderr, exitCode } = await runHook(stopPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/gemini/hook`,
      ATOLL_GEMINI_HOOK_TIMEOUT_MS: "200",
    });
    const elapsedMs = Date.now() - startedAt;

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert(elapsedMs < 1500, `expected fast fallback, took ${elapsedMs}ms`);
    assert.deepEqual(JSON.parse(stdout), {});
  } finally {
    await new Promise((resolve) => slowServer.close(resolve));
  }
}

// 7. Gated BeforeTool waits for the approval even when the bridge is slow.
{
  const slowPermissionServer = http.createServer((_request, response) => {
    setTimeout(() => {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(expectedAllowResponse));
    }, 2000);
  });
  await new Promise((resolve) =>
    slowPermissionServer.listen(0, "127.0.0.1", resolve),
  );

  try {
    const { port } = slowPermissionServer.address();
    const startedAt = Date.now();
    const { stdout, stderr, exitCode } = await runHook(permissionPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/gemini/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
      ATOLL_GEMINI_HOOK_TIMEOUT_MS: "200",
    });
    const elapsedMs = Date.now() - startedAt;

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert(elapsedMs >= 2000, `expected to wait for approval, took ${elapsedMs}ms`);
    assert.deepEqual(JSON.parse(stdout), expectedAllowResponse);
  } finally {
    await new Promise((resolve) => slowPermissionServer.close(resolve));
  }
}
