import assert from "node:assert/strict";
import http from "node:http";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";

const payload = {
  session_id: "session-codex-test",
  cwd: "/tmp/project",
  hook_event_name: "PermissionRequest",
  tool_name: "Bash",
  tool_input: {
    command: "echo from-codex-test",
    description: "Echo from Codex test",
  },
  tool_use_id: "tool-codex-test",
};

const expectedResponse = {
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
    assert.equal(request.url, "/codex/hook");
    assert.equal(request.headers["x-atoll-hook-token"], "env-token");
    assert.deepEqual(JSON.parse(body), payload);

    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(expectedResponse));
  });
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

try {
  const { port } = server.address();
  const tempHome = fs.mkdtempSync(path.join(os.tmpdir(), "atoll-codex-url-test-"));
  const child = spawn(process.execPath, ["scripts/atoll-codex-hook.mjs"], {
    env: {
      ...process.env,
      HOME: tempHome,
      LOCALAPPDATA: tempHome,
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/codex/hook`,
      ATOLL_HOOK_TOKEN: "env-token",
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
