#!/usr/bin/env node
/**
 * Download a pinned Node.js LTS binary into src-tauri/generated/ so the Tauri
 * build can bundle it as a resource. Atoll's agent hooks run as .mjs scripts,
 * and fresh machines may have no system Node — the bundled runtime is the
 * last-resort fallback in src-tauri/src/hooks/node.rs.
 *
 * Targets (see runtimeTarget):
 *   darwin-arm64 / linux-x64 → generated/node/bin/node
 *   win-x64                  → generated/node.exe
 *
 * The archive is verified against the official SHASUMS256.txt of the same
 * release before extraction. A previously fetched runtime is reused when the
 * stamp file matches the pinned version and archive checksum; build.rs writes
 * 0-byte placeholders at these paths, and those never pass the non-empty +
 * stamp check, so placeholder builds always (re)fetch.
 *
 * Usage: node scripts/fetch-node-runtime.mjs
 */

import { createHash } from "node:crypto";
import {
  chmodSync,
  copyFileSync,
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { spawn } from "node:child_process";
import { fileURLToPath, pathToFileURL } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const GENERATED_DIR = path.join(__dirname, "..", "src-tauri", "generated");

export const NODE_VERSION = "22.23.3";
export const DIST_BASE = "https://nodejs.org/dist";

// platform+arch → { platformArch, archiveName, archiveMember, targetRelative }
// archiveMember is the archive-internal path of the node binary; targetRelative
// is the destination under src-tauri/generated/ and must match both the
// build.rs placeholder paths and the tauri.conf.json resources map.
const RUNTIME_TARGETS = {
  "darwin-arm64": {
    archiveName: (version) => `node-v${version}-darwin-arm64.tar.gz`,
    archiveMember: (version) => `node-v${version}-darwin-arm64/bin/node`,
    targetRelative: "node/bin/node",
  },
  "linux-x64": {
    archiveName: (version) => `node-v${version}-linux-x64.tar.gz`,
    archiveMember: (version) => `node-v${version}-linux-x64/bin/node`,
    targetRelative: "node/bin/node",
  },
  "win-x64": {
    archiveName: (version) => `node-v${version}-win-x64.zip`,
    archiveMember: (version) => `node-v${version}-win-x64/node.exe`,
    targetRelative: "node.exe",
  },
};

export function runtimeTarget(platform = process.platform, arch = process.arch) {
  const key = platform === "win32" ? `win-${arch}` : `${platform}-${arch}`;
  const target = RUNTIME_TARGETS[key];
  return target ? { platformArch: key, ...target } : null;
}

export function archiveUrl(version, archiveName) {
  return `${DIST_BASE}/v${version}/${archiveName}`;
}

export function shasumsUrl(version) {
  return `${DIST_BASE}/v${version}/SHASUMS256.txt`;
}

export function stampContent(version, archiveSha256) {
  return `${version} ${archiveSha256}\n`;
}

export function parseShasums(text) {
  const map = new Map();
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const spaceAt = trimmed.indexOf("  ");
    if (spaceAt <= 0) continue;
    const checksum = trimmed.slice(0, spaceAt);
    const name = trimmed.slice(spaceAt + 2).trim();
    if (/^[0-9a-f]{64}$/.test(checksum) && name) {
      map.set(name, checksum);
    }
  }
  return map;
}

function sha256File(filePath) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const input = createReadStream(filePath);
    input.on("data", (chunk) => hash.update(chunk));
    input.on("end", () => resolve(hash.digest("hex")));
    input.on("error", reject);
  });
}

async function downloadToFile(url, destPath) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`download failed: ${url} → HTTP ${response.status}`);
  }
  const tempPath = `${destPath}.download`;
  // fetch() returns a web ReadableStream; wrap it for pipeline().
  await pipeline(Readable.fromWeb(response.body), createWriteStream(tempPath));
  return tempPath;
}

function extractArchiveMember(archivePath, member, destDir) {
  // Windows runners ship bsdtar, which reads zip archives too; -xzf is for
  // the .tar.gz targets. Single-member extraction avoids unpacking ~100MB of
  // headers/docs we do not ship.
  const args = archivePath.endsWith(".tar.gz")
    ? ["-xzf", archivePath]
    : ["-xf", archivePath];
  args.push("-C", destDir, member);
  return new Promise((resolve, reject) => {
    const child = spawn("tar", args, { stdio: "inherit" });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`tar exited with ${code}`));
    });
  });
}

async function main() {
  const target = runtimeTarget();
  if (!target) {
    throw new Error(
      `no bundled runtime target for ${process.platform}-${process.arch}`,
    );
  }

  const archiveName = target.archiveName(NODE_VERSION);
  const member = target.archiveMember(NODE_VERSION);
  const targetPath = path.join(GENERATED_DIR, target.targetRelative);
  const stampPath = `${targetPath}.stamp`;
  mkdirSync(path.dirname(targetPath), { recursive: true });

  const shasumsResponse = await fetch(shasumsUrl(NODE_VERSION));
  if (!shasumsResponse.ok) {
    throw new Error(
      `download failed: ${shasumsUrl(NODE_VERSION)} → HTTP ${shasumsResponse.status}`,
    );
  }
  const shasums = parseShasums(await shasumsResponse.text());
  const archiveSha256 = shasums.get(archiveName);
  if (!archiveSha256) {
    throw new Error(`SHASUMS256.txt has no entry for ${archiveName}`);
  }

  if (
    existsSync(stampPath) &&
    existsSync(targetPath) &&
    statSync(targetPath).size > 0 &&
    readFileSync(stampPath, "utf8") === stampContent(NODE_VERSION, archiveSha256)
  ) {
    console.log(
      `fetch-node-runtime: bundled Node ${NODE_VERSION} already present at ${targetPath}`,
    );
    return;
  }

  console.log(
    `fetch-node-runtime: fetching Node ${NODE_VERSION} (${target.platformArch})…`,
  );
  const archiveTemp = await downloadToFile(
    archiveUrl(NODE_VERSION, archiveName),
    path.join(tmpdir(), archiveName),
  );
  const actualSha256 = await sha256File(archiveTemp);
  if (actualSha256 !== archiveSha256) {
    unlinkSync(archiveTemp);
    throw new Error(
      `checksum mismatch for ${archiveName}: expected ${archiveSha256}, got ${actualSha256}`,
    );
  }

  const extractDir = path.join(
    tmpdir(),
    `atoll-node-${NODE_VERSION}-${Date.now()}`,
  );
  mkdirSync(extractDir, { recursive: true });
  try {
    await extractArchiveMember(archiveTemp, member, extractDir);
    const extracted = path.join(extractDir, member);
    if (!existsSync(extracted)) {
      throw new Error(`archive did not contain ${member}`);
    }
    if (existsSync(targetPath)) {
      unlinkSync(targetPath);
    }
    mkdirSync(path.dirname(targetPath), { recursive: true });
    copyFileSync(extracted, targetPath);
    if (process.platform !== "win32") {
      chmodSync(targetPath, 0o755);
    }
    writeFileSync(stampPath, stampContent(NODE_VERSION, archiveSha256));
    console.log(
      `fetch-node-runtime: wrote ${(statSync(targetPath).size / 1024 / 1024).toFixed(1)} MiB to ${targetPath}`,
    );
  } finally {
    rmSync(extractDir, { recursive: true, force: true });
    rmSync(archiveTemp, { force: true });
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    console.error(`fetch-node-runtime: ${error.message}`);
    process.exit(1);
  });
}
