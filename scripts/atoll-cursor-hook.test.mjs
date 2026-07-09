import assert from "node:assert/strict";
import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const payload = {
  conversation_id: "session-cursor-test",
  cwd: "/tmp/project",
  hook_event_name: "preToolUse",
  tool_name: "Shell",
  tool_input: {
    command: "echo from-cursor-test",
  },
  tool_use_id: "tool-cursor-test",
};

const expectedResponse = {
  permission: "allow",
};

const server = http.createServer((request, response) => {
  let body = "";
  request.setEncoding("utf8");
  request.on("data", (chunk) => {
    body += chunk;
  });
  request.on("end", () => {
    assert.equal(request.method, "POST");
    assert.equal(request.url, "/cursor/hook");
    assert.equal(request.headers["x-atoll-hook-token"], "env-token");
    assert.deepEqual(JSON.parse(body), payload);

    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(expectedResponse));
  });
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

try {
  const { port } = server.address();
  const { stdout, stderr, exitCode } = await runHook(payload, {
    ATOLL_HOOK_URL: `http://127.0.0.1:${port}/cursor/hook`,
    ATOLL_HOOK_TOKEN: "env-token",
  });

  assert.equal(stderr, "");
  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), expectedResponse);
} finally {
  await new Promise((resolve) => server.close(resolve));
}

for (const hookEventName of ["sessionStart", "afterAgentResponse", "sessionEnd", "afterAgentThought"]) {
  const payload = {
    session_id: "session-ask-test",
    hook_event_name: hookEventName,
    composer_mode: "ask",
    workspace_roots: ["/tmp/project"],
  };

  const { stdout, stderr, exitCode } = await runHook(payload, {
    ATOLL_HOOK_URL: "http://127.0.0.1:1/cursor/hook",
  });

  assert.equal(stderr, "", hookEventName);
  assert.equal(exitCode, 0, hookEventName);
  assert.deepEqual(JSON.parse(stdout), {}, hookEventName);
}

{
  const payload = {
    conversation_id: "session-submit-test",
    hook_event_name: "beforeSubmitPrompt",
    composer_mode: "debug",
    workspace_roots: ["/tmp/project"],
    prompt: "reproduce the bug",
  };

  const { stdout, stderr, exitCode } = await runHook(payload, {
    ATOLL_HOOK_URL: "http://127.0.0.1:1/cursor/hook",
  });

  assert.equal(stderr, "");
  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), { continue: true });
}

{
  const slowServer = http.createServer((_request, response) => {
    response.writeHead(200, { "content-type": "application/json" });
    setTimeout(() => response.end(JSON.stringify({ permission: "allow" })), 2000);
  });
  await new Promise((resolve) => slowServer.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = slowServer.address();
    const startedAt = Date.now();
    const { stdout, stderr, exitCode } = await runHook(payload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/cursor/hook`,
      ATOLL_CURSOR_HOOK_TIMEOUT_MS: "200",
    });
    const elapsedMs = Date.now() - startedAt;

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert(elapsedMs < 1500, `expected fast fallback, took ${elapsedMs}ms`);
    assert.deepEqual(JSON.parse(stdout), { permission: "allow" });
  } finally {
    await new Promise((resolve) => slowServer.close(resolve));
  }
}

{
  const payload = {
    conversation_id: "session-submit-timeout-test",
    hook_event_name: "beforeSubmitPrompt",
    prompt: "still let Cursor submit",
  };
  const slowServer = http.createServer((_request, response) => {
    setTimeout(() => {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ continue: true }));
    }, 2000);
  });
  await new Promise((resolve) => slowServer.listen(0, "127.0.0.1", resolve));

  try {
    const { port } = slowServer.address();
    const { stdout, stderr, exitCode } = await runHook(payload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/cursor/hook`,
      ATOLL_CURSOR_HOOK_TIMEOUT_MS: "200",
    });

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert.deepEqual(JSON.parse(stdout), { continue: true });
  } finally {
    await new Promise((resolve) => slowServer.close(resolve));
  }
}

async function runHook(payload, env = {}) {
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-cursor-hook-test-"));
  try {
    const child = spawn(process.execPath, ["scripts/atoll-cursor-hook.mjs"], {
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
