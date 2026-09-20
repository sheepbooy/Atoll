import assert from "node:assert/strict";
import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const permissionPayload = {
  session_id: "session-zcode-test",
  cwd: "/tmp/project",
  hook_event_name: "PermissionRequest",
  tool_name: "Bash",
  tool_input: {
    command: "echo from-zcode-test",
    description: "Echo from ZCode test",
  },
  tool_use_id: "tool-zcode-test",
};

const expectedPermissionResponse = {
  hookSpecificOutput: {
    hookEventName: "PermissionRequest",
    decision: {
      behavior: "allow",
    },
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
    assert.equal(request.url, "/zcode/hook");
    assert.equal(request.headers["x-atoll-hook-token"], "env-token");
    assert.deepEqual(JSON.parse(body), permissionPayload);

    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(expectedPermissionResponse));
  });
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

try {
  const { port } = server.address();
  const { stdout, stderr, exitCode } = await runHook(permissionPayload, {
    ATOLL_HOOK_URL: `http://127.0.0.1:${port}/zcode/hook`,
    ATOLL_HOOK_TOKEN: "env-token",
  });

  assert.equal(stderr, "");
  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), expectedPermissionResponse);
} finally {
  await new Promise((resolve) => server.close(resolve));
}

for (const hookEventName of [
  "PostToolUse",
  "PostToolUseFailure",
  "Stop",
  "SessionStart",
  "UserPromptSubmit",
]) {
  const payload = {
    session_id: "session-zcode-stop-test",
    cwd: "/tmp/project",
    hook_event_name: hookEventName,
  };

  const { stdout, stderr, exitCode } = await runHook(payload, {
    ATOLL_HOOK_URL: "http://127.0.0.1:1/zcode/hook",
  });

  assert.equal(stderr, "", hookEventName);
  assert.equal(exitCode, 0, hookEventName);
  assert.deepEqual(JSON.parse(stdout), {}, hookEventName);
}

{
  // Bridge unreachable: a PermissionRequest spools to ~/.atoll/hook-spool so
  // Atoll can import it as resolved-outside-Atoll history on next start.
  const { stdout, exitCode, home } = await runHook(
    permissionPayload,
    { ATOLL_HOOK_URL: "http://127.0.0.1:1/zcode/hook" },
    { keepTempHome: true },
  );

  assert.equal(exitCode, 0);
  assert.deepEqual(JSON.parse(stdout), {}); // silent fallback still applies

  try {
    const spoolDir = path.join(home, ".atoll", "hook-spool");
    const files = fs.readdirSync(spoolDir).filter((name) => name.endsWith(".json"));
    assert.equal(files.length, 1);
    assert.ok(files[0].startsWith("zcode-"), `unexpected spool name ${files[0]}`);
    const record = JSON.parse(fs.readFileSync(path.join(spoolDir, files[0]), "utf8"));
    assert.equal(record.agent, "zcode");
    assert.ok(Number.isFinite(record.spooledAtSecs) && record.spooledAtSecs > 0);
    assert.equal(JSON.parse(record.payload).hook_event_name, "PermissionRequest");
    assert.equal(JSON.parse(record.payload).tool_input.command, "echo from-zcode-test");
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
}

{
  // Observer events must not spool — only permission requests do.
  const { home } = await runHook(
    {
      session_id: "session-zcode-no-spool",
      cwd: "/tmp/project",
      hook_event_name: "Stop",
    },
    { ATOLL_HOOK_URL: "http://127.0.0.1:1/zcode/hook" },
    { keepTempHome: true },
  );

  try {
    const spoolDir = path.join(home, ".atoll", "hook-spool");
    assert.equal(fs.existsSync(spoolDir), false, "observer events must not spool");
  } finally {
    fs.rmSync(home, { recursive: true, force: true });
  }
}

{
  const stopPayload = {
    session_id: "session-zcode-slow-stop",
    cwd: "/tmp/project",
    hook_event_name: "Stop",
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
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/zcode/hook`,
      ATOLL_ZCODE_HOOK_TIMEOUT_MS: "200",
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

{
  const slowPermissionServer = http.createServer((_request, response) => {
    setTimeout(() => {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify(expectedPermissionResponse));
    }, 2000);
  });
  await new Promise((resolve) =>
    slowPermissionServer.listen(0, "127.0.0.1", resolve),
  );

  try {
    const { port } = slowPermissionServer.address();
    const startedAt = Date.now();
    const { stdout, stderr, exitCode } = await runHook(permissionPayload, {
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/zcode/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
      ATOLL_ZCODE_HOOK_TIMEOUT_MS: "200",
    });
    const elapsedMs = Date.now() - startedAt;

    assert.equal(stderr, "");
    assert.equal(exitCode, 0);
    assert(elapsedMs >= 2000, `expected to wait for approval, took ${elapsedMs}ms`);
    assert.deepEqual(JSON.parse(stdout), expectedPermissionResponse);
  } finally {
    await new Promise((resolve) => slowPermissionServer.close(resolve));
  }
}

async function runHook(payload, env = {}, { keepTempHome = false } = {}) {
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-zcode-hook-test-"));
  try {
    const child = spawn(process.execPath, ["scripts/atoll-zcode-hook.mjs"], {
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

    return { stdout, stderr, exitCode, home: tempHome };
  } finally {
    if (!keepTempHome) {
      fs.rmSync(tempHome, { recursive: true, force: true });
    }
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
