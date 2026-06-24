#!/usr/bin/env node
/**
 * Capture compact-bar.png from the real Tauri app (macOS).
 * Usage: node scripts/capture-compact-bar.mjs
 */

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const OUTPUT = join(ROOT, "docs/assets/compact-bar.png");
const BRIDGE = "http://127.0.0.1:47777";

const SETTLE_MS = 670;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForBridge(timeoutMs = 180_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(`${BRIDGE}/capture/screenshot`, { method: "POST", body: "{}" });
      if (res.ok) {
        const payload = await res.json();
        if (payload.width > 0) return;
      }
    } catch {
      // not ready
    }
    await sleep(500);
  }
  throw new Error("Atoll hook bridge not reachable on 127.0.0.1:47777");
}

async function capturePost(path) {
  const res = await fetch(`${BRIDGE}${path}`, { method: "POST", body: "{}" });
  if (!res.ok) throw new Error(`Capture POST ${path} failed (${res.status})`);
}

async function shotApp(output) {
  const res = await fetch(`${BRIDGE}/capture/screenshot`, { method: "POST", body: "{}" });
  if (!res.ok) throw new Error("Failed to capture window screenshot");
  const payload = await res.json();
  if (payload.error) throw new Error(payload.error);
  writeFileSync(output, Buffer.from(payload.png_base64, "base64"));
}

async function main() {
  console.log("Starting Atoll (ATOLL_CAPTURE=1)…");
  const child = spawn("npm", ["run", "tauri", "dev"], {
    cwd: ROOT,
    env: { ...process.env, ATOLL_CAPTURE: "1" },
    stdio: ["ignore", "pipe", "pipe"],
  });

  let stderr = "";
  child.stderr?.on("data", (chunk) => {
    stderr += chunk.toString();
  });

  const cleanup = () => {
    if (!child.killed) child.kill("SIGTERM");
  };
  process.on("exit", cleanup);
  process.on("SIGINT", () => {
    cleanup();
    process.exit(1);
  });

  try {
    await waitForBridge();
    await sleep(800);
    await capturePost("/capture/approval");
    await sleep(400);
    await capturePost("/capture/collapse");
    await sleep(SETTLE_MS);
    await shotApp(OUTPUT);
    console.log(`Wrote ${OUTPUT}`);
  } catch (error) {
    if (stderr) console.error(stderr.slice(-4000));
    throw error;
  } finally {
    cleanup();
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
