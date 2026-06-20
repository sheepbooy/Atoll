#!/usr/bin/env node
/**
 * Capture README screenshots and demo GIF from the real Tauri app window.
 * macOS only. Uses CGWindow capture via /capture/screenshot (not screencapture).
 *
 * Usage: node scripts/capture-app-media.mjs
 */

import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { PNG } from "pngjs";
import gifenc from "gifenc";

const { GIFEncoder, quantize, applyPalette } = gifenc;

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const ASSETS = join(ROOT, "docs/assets");
const FRAMES_DIR = join(ASSETS, ".gif-frames");
const BRIDGE = "http://127.0.0.1:47777";

const ANIM_MS = 420;
const FRAME_MS = 35;
const ANIM_FRAMES = Math.ceil(ANIM_MS / FRAME_MS);
const SETTLE_MS = ANIM_MS + 250;

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
  if (!res.ok) {
    throw new Error(`Capture POST ${path} failed (${res.status})`);
  }
}

async function shotApp(output) {
  const res = await fetch(`${BRIDGE}/capture/screenshot`, { method: "POST", body: "{}" });
  if (!res.ok) throw new Error("Failed to capture window screenshot");
  const payload = await res.json();
  if (payload.error) throw new Error(payload.error);
  writeFileSync(output, Buffer.from(payload.png_base64, "base64"));
}

function frameFromPng(png) {
  const palette = quantize(png.data, 128);
  const index = applyPalette(png.data, palette);
  return { width: png.width, height: png.height, palette, index, data: png.data };
}

function padFrame(png, canvasW, canvasH) {
  const out = new PNG({ width: canvasW, height: canvasH });
  for (let i = 0; i < out.data.length; i += 4) {
    out.data[i] = 10;
    out.data[i + 1] = 11;
    out.data[i + 2] = 13;
    out.data[i + 3] = 255;
  }
  const ox = Math.floor((canvasW - png.width) / 2);
  const oy = Math.floor((canvasH - png.height) / 2);
  PNG.bitblt(png, out, 0, 0, png.width, png.height, ox, oy);
  return frameFromPng(out);
}

function frameFromFile(path) {
  return frameFromPng(PNG.sync.read(readFileSync(path)));
}

function normalizeGifFrames(rawFrames) {
  const maxW = Math.max(...rawFrames.map((frame) => frame.width));
  const maxH = Math.max(...rawFrames.map((frame) => frame.height));
  return rawFrames.map((frame) => {
    if (frame.width === maxW && frame.height === maxH) {
      const { data: _drop, ...rest } = frame;
      return rest;
    }
    const png = new PNG({ width: frame.width, height: frame.height });
    png.data = Buffer.from(frame.data);
    return padFrame(png, maxW, maxH);
  });
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

async function captureGifFrames(tmpDir) {
  const frames = [];
  let index = 0;

  async function snap(delay, count = 1) {
    for (let i = 0; i < count; i += 1) {
      const file = join(tmpDir, `frame-${String(index).padStart(3, "0")}.png`);
      await shotApp(file);
      frames.push({ ...frameFromFile(file), delay });
      index += 1;
    }
  }

  await capturePost("/capture/collapse");
  await sleep(ANIM_MS + 100);
  await snap(110, 6);

  await capturePost("/capture/expand");
  await sleep(40);
  for (let i = 0; i < ANIM_FRAMES; i += 1) {
    await sleep(FRAME_MS);
    await snap(FRAME_MS);
  }

  await sleep(200);
  await snap(110, 12);

  await capturePost("/capture/collapse");
  await sleep(40);
  for (let i = 0; i < ANIM_FRAMES; i += 1) {
    await sleep(FRAME_MS);
    await snap(FRAME_MS);
  }

  await sleep(200);
  await snap(110, 5);

  return frames;
}

async function main() {
  rmSync(FRAMES_DIR, { recursive: true, force: true });
  mkdirSync(FRAMES_DIR, { recursive: true });

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
    await shotApp(join(ASSETS, "compact-bar.png"));
    console.log("Wrote docs/assets/compact-bar.png");

    await capturePost("/capture/expand");
    await sleep(SETTLE_MS);
    await shotApp(join(ASSETS, "approval.png"));
    console.log("Wrote docs/assets/approval.png");

    await capturePost("/capture/collapse");
    await sleep(SETTLE_MS);

    await capturePost("/capture/hooks");
    await sleep(SETTLE_MS);
    await shotApp(join(ASSETS, "idle.png"));
    console.log("Wrote docs/assets/idle.png");

    await capturePost("/capture/collapse");
    await sleep(ANIM_MS + 150);
    await capturePost("/capture/approval");
    await sleep(300);

    const gifFrames = normalizeGifFrames(await captureGifFrames(FRAMES_DIR));
    const gif = encodeGif(gifFrames);
    writeFileSync(join(ASSETS, "demo.gif"), gif);
    console.log(
      `Wrote docs/assets/demo.gif (${gifFrames.length} frames, ${(gif.length / 1024).toFixed(1)} KB)`,
    );
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
