import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, existsSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

import { cleanBuild } from "./clean-build.mjs";

function makeRoot() {
  const root = mkdtempSync(path.join(tmpdir(), "atoll-clean-"));
  return root;
}

function seedArtifacts(root) {
  for (const relative of [
    "src-tauri/target/release/build",
    "src-tauri/generated/node/bin",
    "src-tauri/gen/schemas",
    "dist",
  ]) {
    mkdirSync(path.join(root, relative), { recursive: true });
  }
  writeFileSync(path.join(root, "src-tauri/target/release/build/touch"), "x");
  writeFileSync(path.join(root, "src-tauri/generated/node/bin/node"), "x");
  writeFileSync(path.join(root, "src-tauri/gen/schemas/capabilities.json"), "x");
  writeFileSync(path.join(root, "dist/index.html"), "x");
}

test("default clean removes target, dist and gen/schemas but keeps generated", () => {
  const root = makeRoot();
  try {
    seedArtifacts(root);
    const result = cleanBuild(root);
    assert.deepEqual(result.removed, [
      "src-tauri/target",
      "dist",
      "src-tauri/gen/schemas",
    ]);
    assert.ok(result.freed > 0);
    assert.ok(existsSync(path.join(root, "src-tauri/generated/node/bin/node")));
    assert.ok(!existsSync(path.join(root, "src-tauri/target")));
    assert.ok(!existsSync(path.join(root, "dist")));
    assert.ok(!existsSync(path.join(root, "src-tauri/gen/schemas")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("--all also removes the fetched node runtime", () => {
  const root = makeRoot();
  try {
    seedArtifacts(root);
    cleanBuild(root, { all: true });
    assert.ok(!existsSync(path.join(root, "src-tauri/generated")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("--keep-bundle moves macos and dmg bundles to release-out before cleaning", () => {
  const root = makeRoot();
  try {
    mkdirSync(path.join(root, "src-tauri/target/release/bundle/macos"), {
      recursive: true,
    });
    mkdirSync(path.join(root, "src-tauri/target/release/bundle/dmg"), {
      recursive: true,
    });
    writeFileSync(
      path.join(root, "src-tauri/target/release/bundle/macos/Atoll.app"),
      "x"
    );
    writeFileSync(
      path.join(root, "src-tauri/target/release/bundle/dmg/Atoll.dmg"),
      "x"
    );
    const result = cleanBuild(root, { keepBundle: true });
    assert.deepEqual(result.moved, ["release-out/macos", "release-out/dmg"]);
    assert.ok(existsSync(path.join(root, "release-out/macos/Atoll.app")));
    assert.ok(existsSync(path.join(root, "release-out/dmg/Atoll.dmg")));
    assert.ok(!existsSync(path.join(root, "src-tauri/target")));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("missing artifact paths are skipped without error", () => {
  const root = makeRoot();
  try {
    const result = cleanBuild(root, { all: true, keepBundle: true });
    assert.deepEqual(result.removed, []);
    assert.deepEqual(result.moved, []);
    assert.equal(result.freed, 0);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
