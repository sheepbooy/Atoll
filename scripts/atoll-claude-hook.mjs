#!/usr/bin/env node

const defaultHookUrl = "http://127.0.0.1:47777/claude/pre-tool-use";
const hookUrl = process.env.ATOLL_HOOK_URL || defaultHookUrl;

try {
  const payload = await readStdin();
  const response = await fetch(hookUrl, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: payload || "{}",
  });

  if (!response.ok) {
    throw new Error(`Atoll hook bridge returned HTTP ${response.status}`);
  }

  const text = await response.text();
  JSON.parse(text);
  process.stdout.write(text);
} catch (error) {
  process.stdout.write(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "ask",
        permissionDecisionReason: `Atoll unavailable: ${error.message}`,
      },
    }),
  );
}

function readStdin() {
  return new Promise((resolve, reject) => {
    let value = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => {
      value += chunk;
    });
    process.stdin.on("end", () => resolve(value));
    process.stdin.on("error", reject);
  });
}
