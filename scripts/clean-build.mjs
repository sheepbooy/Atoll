#!/usr/bin/env node
/**
 * Delete local build artifacts produced by Atoll builds.
 *
 * Usage: node scripts/clean-build.mjs [--all] [--keep-bundle]
 *
 * Removed by default:
 *   src-tauri/target        Rust/Cargo build cache (the multi-GB one)
 *   dist                    Vite frontend build output
 *   src-tauri/gen/schemas   Tauri CLI-generated capability schemas
 *
 * --all also removes src-tauri/generated (the fetched Node runtime used as a
 * bundled resource). Builds recreate 0-byte placeholders without it; run
 * `npm run fetch:node-runtime` to restore the real runtime.
 *
 * --keep-bundle (used by `npm run build:app`) moves the freshly built
 * target/release/bundle/{macos,dmg} outputs to release-out/ first, so the
 * Atoll.app / .dmg you just built survive the cleanup.
 */

import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  ".."
);

function dirSize(target) {
  let total = 0;
  const stack = [target];
  while (stack.length > 0) {
    const current = stack.pop();
    let entries;
    try {
      entries = readdirSync(current, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const child = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(child);
      } else {
        try {
          total += statSync(child).size;
        } catch {
          // best-effort; the file may vanish mid-walk
        }
      }
    }
  }
  return total;
}

function preserveBundle(root) {
  const bundleDir = path.join(root, "src-tauri/target/release/bundle");
  if (!existsSync(bundleDir)) {
    return [];
  }
  const outDir = path.join(root, "release-out");
  const moved = [];
  for (const kind of ["macos", "dmg"]) {
    const source = path.join(bundleDir, kind);
    if (!existsSync(source)) {
      continue;
    }
    const dest = path.join(outDir, kind);
    rmSync(dest, { recursive: true, force: true });
    mkdirSync(outDir, { recursive: true });
    renameSync(source, dest);
    moved.push(path.relative(root, dest));
  }
  return moved;
}

export function cleanBuild(root, { all = false, keepBundle = false } = {}) {
  const moved = keepBundle ? preserveBundle(root) : [];
  const targets = [
    "src-tauri/target",
    "dist",
    "src-tauri/gen/schemas",
    ...(all ? ["src-tauri/generated"] : []),
  ];
  const removed = [];
  let freed = 0;
  for (const relative of targets) {
    const absolute = path.join(root, relative);
    if (!existsSync(absolute)) {
      continue;
    }
    freed += dirSize(absolute);
    rmSync(absolute, { recursive: true, force: true });
    removed.push(relative);
  }
  return { removed, moved, freed };
}

function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value >= 100 ? Math.round(value) : value.toFixed(1)} ${units[index]}`;
}

function main() {
  const flags = new Set(process.argv.slice(2));
  const result = cleanBuild(repoRoot, {
    all: flags.has("--all"),
    keepBundle: flags.has("--keep-bundle"),
  });
  for (const moved of result.moved) {
    console.log(`moved ${moved}/`);
  }
  if (result.removed.length === 0) {
    console.log("nothing to clean");
    return;
  }
  for (const removed of result.removed) {
    console.log(`removed ${removed}/`);
  }
  console.log(`freed ${formatBytes(result.freed)}`);
  if (result.removed.includes("src-tauri/generated")) {
    console.log(
      "note: bundled Node runtime removed — run `npm run fetch:node-runtime` to restore it"
    );
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main();
}
