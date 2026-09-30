import assert from "node:assert/strict";
import { test } from "node:test";

import {
  NODE_VERSION,
  archiveUrl,
  parseShasums,
  runtimeTarget,
  shasumsUrl,
  stampContent,
} from "./fetch-node-runtime.mjs";

test("runtimeTarget maps darwin-arm64 to the tar.gz layout", () => {
  const target = runtimeTarget("darwin", "arm64");
  assert.equal(target.platformArch, "darwin-arm64");
  assert.equal(target.archiveName(NODE_VERSION), `node-v${NODE_VERSION}-darwin-arm64.tar.gz`);
  assert.equal(
    target.archiveMember(NODE_VERSION),
    `node-v${NODE_VERSION}-darwin-arm64/bin/node`,
  );
  assert.equal(target.targetRelative, "node/bin/node");
});

test("runtimeTarget maps win32-x64 to the zip layout with node.exe", () => {
  const target = runtimeTarget("win32", "x64");
  assert.equal(target.platformArch, "win-x64");
  assert.equal(target.archiveName(NODE_VERSION), `node-v${NODE_VERSION}-win-x64.zip`);
  assert.equal(target.archiveMember(NODE_VERSION), `node-v${NODE_VERSION}-win-x64/node.exe`);
  assert.equal(target.targetRelative, "node.exe");
});

test("runtimeTarget returns null for unsupported platforms", () => {
  assert.equal(runtimeTarget("sunos", "x64"), null);
  assert.equal(runtimeTarget("darwin", "x64"), null);
  assert.equal(runtimeTarget("win32", "arm64"), null);
});

test("urls point at the pinned nodejs.org release", () => {
  assert.equal(
    archiveUrl("22.23.3", "node-v22.23.3-darwin-arm64.tar.gz"),
    "https://nodejs.org/dist/v22.23.3/node-v22.23.3-darwin-arm64.tar.gz",
  );
  assert.equal(
    shasumsUrl("22.23.3"),
    "https://nodejs.org/dist/v22.23.3/SHASUMS256.txt",
  );
});

test("stampContent pairs the version with the archive checksum", () => {
  assert.equal(
    stampContent("22.23.3", "abc123"),
    "22.23.3 abc123\n",
  );
});

test("parseShasums extracts checksums and skips malformed lines", () => {
  const map = parseShasums(
    [
      "23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53  node-v22.23.3-darwin-arm64.tar.gz",
      "2b0ff57b049cda1bbcea2240eec20467018713c1efe1f7360c2681859b90ed71  node-v22.23.3-win-x64.zip",
      "",
      "not-a-checksum  node-v22.23.3-headers.tar.gz",
      "-----BEGIN PGP SIGNATURE-----",
    ].join("\n"),
  );
  assert.equal(map.size, 2);
  assert.equal(
    map.get("node-v22.23.3-darwin-arm64.tar.gz"),
    "23b25245dcfb9af7262f8ff142e9e2e0af025368117329e7a7458a51e5922f53",
  );
  assert.equal(
    map.get("node-v22.23.3-win-x64.zip"),
    "2b0ff57b049cda1bbcea2240eec20467018713c1efe1f7360c2681859b90ed71",
  );
  assert.equal(map.has("node-v22.23.3-headers.tar.gz"), false);
});
