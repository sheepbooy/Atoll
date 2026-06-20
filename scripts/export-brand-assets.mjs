#!/usr/bin/env node
/**
 * Export Atoll logo states + agent mascot PNGs for README.
 * Usage: node scripts/export-brand-assets.mjs
 */

import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const ASSETS = join(ROOT, "docs/assets");
const PORT = 1421;
const URL = `http://127.0.0.1:${PORT}/?export=brand`;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitForServer(timeoutMs = 60_000) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    try {
      const res = await fetch(URL);
      if (res.ok) return;
    } catch {
      // not ready
    }
    await sleep(300);
  }
  throw new Error(`Vite dev server not ready at ${URL}`);
}

async function main() {
  mkdirSync(ASSETS, { recursive: true });

  const child = spawn("npm", ["run", "dev", "--", "--host", "127.0.0.1", "--port", String(PORT)], {
    cwd: ROOT,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env },
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
    await waitForServer();
    await sleep(500);

    const browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({
      viewport: { width: 960, height: 480 },
      deviceScaleFactor: 2,
    });

    await page.goto(URL, { waitUntil: "networkidle" });
    await page.waitForTimeout(400);

    const items = await page.$$("[data-export-item]");
    for (const item of items) {
      const name = await item.getAttribute("data-export-item");
      const box = await item.boundingBox();
      if (!name || !box) continue;
      const buffer = await page.screenshot({
        type: "png",
        clip: {
          x: box.x - 8,
          y: box.y - 8,
          width: box.width + 16,
          height: box.height + 16,
        },
      });
      writeFileSync(join(ASSETS, `${name}.png`), buffer);
      console.log(`Wrote docs/assets/${name}.png`);
    }

    for (const group of ["atoll-states", "agent-mascots"]) {
      const el = await page.$(`[data-export="${group}"]`);
      if (!el) continue;
      const box = await el.boundingBox();
      if (!box) continue;
      const buffer = await page.screenshot({
        type: "png",
        clip: {
          x: box.x - 16,
          y: box.y - 40,
          width: box.width + 32,
          height: box.height + 56,
        },
      });
      writeFileSync(join(ASSETS, `${group}.png`), buffer);
      console.log(`Wrote docs/assets/${group}.png`);
    }

    await browser.close();
  } finally {
    cleanup();
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
