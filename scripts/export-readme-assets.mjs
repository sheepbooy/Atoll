#!/usr/bin/env node
/**
 * Export README raster assets from SVG sources.
 * Usage: node scripts/export-readme-assets.mjs
 */

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const ASSETS = join(ROOT, "docs/assets");

async function exportSvgPng(name, width, height) {
  const svg = readFileSync(join(ASSETS, `${name}.svg`), "utf8");
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width, height },
    deviceScaleFactor: 1,
  });

  try {
    await page.setContent(
      `<!doctype html><html><head><meta charset="utf-8"/></head><body style="margin:0;background:#0a0b0d;overflow:hidden">${svg}</body></html>`,
      { waitUntil: "networkidle" },
    );
    await page.waitForTimeout(200);
    const buffer = await page.screenshot({ type: "png", fullPage: false });
    writeFileSync(join(ASSETS, `${name}.png`), buffer);
    console.log(`Wrote docs/assets/${name}.png (${(buffer.length / 1024).toFixed(1)} KB)`);
  } finally {
    await browser.close();
  }
}

async function exportLogoPng() {
  const svg = readFileSync(join(ASSETS, "logo.svg"), "utf8");
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage({
    viewport: { width: 128, height: 144 },
    deviceScaleFactor: 4,
  });

  try {
    await page.setContent(
      `<!doctype html><html><head><meta charset="utf-8"/></head><body style="margin:0;background:transparent;display:flex;align-items:center;justify-content:center;width:128px;height:144px">${svg}</body></html>`,
      { waitUntil: "networkidle" },
    );
    const buffer = await page.screenshot({
      type: "png",
      omitBackground: true,
      clip: { x: 0, y: 0, width: 128, height: 144 },
    });
    writeFileSync(join(ASSETS, "logo.png"), buffer);
    console.log(`Wrote docs/assets/logo.png (${(buffer.length / 1024).toFixed(1)} KB)`);
  } finally {
    await browser.close();
  }
}

async function main() {
  await exportSvgPng("hero", 1200, 420);
  await exportSvgPng("compact-bar", 900, 120);
  await exportSvgPng("architecture", 960, 360);
  await exportLogoPng();
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
