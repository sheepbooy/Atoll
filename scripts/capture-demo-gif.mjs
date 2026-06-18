#!/usr/bin/env node
/**
 * Capture a README demo GIF from the Vite preview server.
 * Requires: dev server on 127.0.0.1:1420 (npm run dev)
 *
 * Usage: node scripts/capture-demo-gif.mjs
 */

import { mkdirSync, writeFileSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import gifenc from "gifenc";
import { chromium } from "playwright";

const { GIFEncoder, quantize, applyPalette } = gifenc;

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const FRAMES_DIR = join(ROOT, "docs/assets/.gif-frames");
const OUTPUT = join(ROOT, "docs/assets/demo.gif");
const BASE_URL = "http://127.0.0.1:1420/?demo=compact";

const VIEWPORT = { width: 640, height: 420 };
const CLIP = { x: 40, y: 36, width: 560, height: 360 };

async function waitForServer(page) {
  for (let attempt = 0; attempt < 30; attempt += 1) {
    try {
      await page.goto(BASE_URL, { waitUntil: "networkidle", timeout: 5000 });
      await page.waitForSelector("section.island", { timeout: 5000 });
      await page.waitForTimeout(500);
      return;
    } catch (error) {
      if (attempt === 29) {
        const html = await page.content().catch(() => "");
        throw new Error(
          `Failed to load demo UI: ${error.message}\nSnippet: ${html.slice(0, 400)}`,
        );
      }
      await page.waitForTimeout(300);
    }
  }
}

function frameFromPng(buffer) {
  const png = PNG.sync.read(buffer);
  const { width, height, data } = png;
  const palette = quantize(data, 128);
  const index = applyPalette(data, palette);
  return { width, height, palette, index };
}

function encodeGif(frames) {
  const enc = GIFEncoder();
  for (const frame of frames) {
    enc.writeFrame(frame.index, frame.width, frame.height, {
      palette: frame.palette,
      delay: frame.delay,
      dispose: 1,
    });
  }
  enc.finish();
  return Buffer.from(enc.bytes());
}

async function captureIsland(page, delay) {
  const buffer = await page.screenshot({ clip: CLIP, type: "png" });
  return { ...frameFromPng(buffer), delay };
}

async function captureFrames(page) {
  const frames = [];

  async function shot(delay) {
    frames.push(await captureIsland(page, delay));
  }

  // Compact idle
  for (let i = 0; i < 6; i += 1) await shot(110);

  // Expand
  await page.locator(".island").click({ position: { x: 80, y: 18 } });
  for (let i = 0; i < 12; i += 1) {
    await page.waitForTimeout(40);
    await shot(40);
  }

  // Hold expanded approval panel
  for (let i = 0; i < 10; i += 1) await shot(120);

  // Collapse
  await page.getByRole("button", { name: "Collapse Atoll" }).click();
  for (let i = 0; i < 12; i += 1) {
    await page.waitForTimeout(40);
    await shot(40);
  }

  // Compact again
  for (let i = 0; i < 5; i += 1) await shot(110);

  return frames;
}

async function main() {
  rmSync(FRAMES_DIR, { recursive: true, force: true });
  mkdirSync(FRAMES_DIR, { recursive: true });

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ viewport: VIEWPORT, deviceScaleFactor: 1 });

  try {
    await waitForServer(page);
    await page.waitForTimeout(400);
    const frames = await captureFrames(page);
    const gif = encodeGif(frames);
    writeFileSync(OUTPUT, gif);
    console.log(`Wrote ${OUTPUT} (${frames.length} frames, ${(gif.length / 1024).toFixed(1)} KB)`);
  } finally {
    await browser.close();
  }
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
