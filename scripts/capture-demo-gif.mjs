#!/usr/bin/env node
/**
 * Capture a README demo GIF that mirrors the real Tauri window behavior.
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
const OUTPUT = join(ROOT, "docs/assets/demo.gif");
const BASE_URL = "http://127.0.0.1:1420/?demo=gif";

const EXPANDED_W = 560;
const EXPANDED_H = 320;
const COMPACT_H = 36;
const MENU_BAR_H = 28;
const ANIM_MS = 420;
const FRAME_MS = 35;
const ANIM_FRAMES = Math.ceil(ANIM_MS / FRAME_MS);
const CANVAS_PAD_X = 48;
const CANVAS_PAD_BOTTOM = 32;

function easeOutCubic(t) {
  return 1 - (1 - t) ** 3;
}

function lerp(a, b, t) {
  return a + (b - a) * t;
}

async function waitForServer(page) {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    try {
      await page.goto(BASE_URL, { waitUntil: "domcontentloaded", timeout: 4000 });
      await page.waitForSelector("section.island", { timeout: 4000 });
      await page.waitForFunction(
        () => document.documentElement.dataset.gifCompactWidth,
        { timeout: 4000 },
      );
      await page.waitForTimeout(500);
      return;
    } catch (error) {
      if (attempt === 19) {
        throw new Error(`Failed to load GIF capture UI: ${error.message}`);
      }
      await page.waitForTimeout(250);
    }
  }
}

async function readCompactWidth(page) {
  return Number(
    await page.evaluate(
      () => document.documentElement.dataset.gifCompactWidth ?? "132",
    ),
  );
}

async function setWindowSize(page, w, h) {
  const viewportW = Math.ceil(w + CANVAS_PAD_X * 2);
  const viewportH = Math.ceil(MENU_BAR_H + h + CANVAS_PAD_BOTTOM);
  await page.setViewportSize({ width: viewportW, height: viewportH });
  await page.evaluate(
    ({ width, height }) => {
      document.documentElement.style.setProperty("--gif-window-w", `${width}px`);
      document.documentElement.style.setProperty("--gif-window-h", `${height}px`);
    },
    { width: w, height: h },
  );
}

async function focusIsland(page) {
  const island = page.locator("section.island");
  const box = await island.boundingBox();
  if (!box) return;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await page.mouse.move(x, y);
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

async function captureFrame(page, delay) {
  const buffer = await page.screenshot({ type: "png", fullPage: false });
  return { ...frameFromPng(buffer), delay };
}

async function animateWindow(page, fromW, fromH, toW, toH, frames, delayMs) {
  const shots = [];
  for (let i = 0; i < frames; i += 1) {
    const t = easeOutCubic((i + 1) / frames);
    const w = Math.round(lerp(fromW, toW, t));
    const h = Math.round(lerp(fromH, toH, t));
    await setWindowSize(page, w, h);
    await focusIsland(page);
    await page.waitForTimeout(FRAME_MS);
    shots.push(await captureFrame(page, delayMs));
  }
  return shots;
}

async function triggerExpand(page) {
  const island = page.locator("section.island");
  await island.dispatchEvent("pointerenter");
  await island.hover();
  await page.waitForSelector(
    "section.island.is-opening, section.island.is-expanded",
    { timeout: 4000 },
  );
}

async function captureFrames(page) {
  const frames = [];
  const compactW = await readCompactWidth(page);
  const island = page.locator("section.island");

  async function hold(count, delay, w, h) {
    await setWindowSize(page, w, h);
    await focusIsland(page);
    await page.waitForTimeout(120);
    for (let i = 0; i < count; i += 1) {
      frames.push(await captureFrame(page, delay));
    }
  }

  await hold(8, 100, compactW, COMPACT_H);

  await triggerExpand(page);
  frames.push(await captureFrame(page, 40));

  frames.push(
    ...(await animateWindow(
      page,
      compactW,
      COMPACT_H,
      EXPANDED_W,
      EXPANDED_H,
      ANIM_FRAMES,
      FRAME_MS,
    )),
  );

  await page.waitForSelector("section.island.is-expanded", { timeout: 4000 });
  await setWindowSize(page, EXPANDED_W, EXPANDED_H);
  await focusIsland(page);
  await hold(12, 110, EXPANDED_W, EXPANDED_H);

  await page.getByRole("button", { name: "Collapse Atoll" }).click();
  await page.waitForSelector(
    "section.island.is-closing, section.island.is-compact",
    { timeout: 4000 },
  );
  frames.push(await captureFrame(page, 40));

  frames.push(
    ...(await animateWindow(
      page,
      EXPANDED_W,
      EXPANDED_H,
      compactW,
      COMPACT_H,
      ANIM_FRAMES,
      FRAME_MS,
    )),
  );

  await page.waitForSelector("section.island.is-compact", { timeout: 4000 });
  await hold(6, 100, compactW, COMPACT_H);

  return frames;
}

async function main() {
  rmSync(join(ROOT, "docs/assets/.gif-frames"), { recursive: true, force: true });
  mkdirSync(join(ROOT, "docs/assets/.gif-frames"), { recursive: true });

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({ deviceScaleFactor: 1.5 });

  try {
    await waitForServer(page);
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
