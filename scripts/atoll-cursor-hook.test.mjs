import assert from "node:assert/strict";
import http from "node:http";
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
    assert.deepEqual(JSON.parse(body), payload);

    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify(expectedResponse));
  });
});

await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

try {
  const { port } = server.address();
  const child = spawn(process.execPath, ["scripts/atoll-cursor-hook.mjs"], {
    env: {
      ...process.env,
      ATOLL_HOOK_URL: `http://127.0.0.1:${port}/cursor/hook`,
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
