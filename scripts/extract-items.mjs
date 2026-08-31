#!/usr/bin/env node
// Generic top-level item extractor for src-tauri/src/lib.rs.
// Usage: node extract-items.mjs <ModuleFile> <ItemName...>
// For each ItemName, finds its definition (fn/async fn/struct/enum/const/static
// or #[tauri::command] fn), walks over preceding attrs/doc comments, brace-counts
// to its end, extracts all items into <ModuleFile> (created with a header),
// pub(crate)-izes private fns/consts, deletes them from lib.rs, and registers
// `mod <name>;` + a glob re-export.
import { readFileSync, writeFileSync, existsSync } from "node:fs";

const [modFile, ...names] = process.argv.slice(2);
if (!modFile || names.length === 0) {
  console.error("usage: node extract-items.mjs <module-file> <ItemName...>");
  process.exit(1);
}
const modName = modFile.split("/").pop().replace(/\.rs$/, "");

const p = "src-tauri/src/lib.rs";
const lines = readFileSync(p, "utf8").split("\n");

function findItem(name) {
  const defRe = new RegExp(
    `^(pub\\(crate\\) )?(async fn|fn|struct|enum|const|static) ${name}\\b`,
  );
  const candidates = [];
  for (let i = 0; i < lines.length; i++) {
    if (defRe.test(lines[i])) candidates.push(i);
    // #[tauri::command] followed by fn NAME
    if (
      lines[i].trim() === "#[tauri::command]" &&
      lines[i + 1] &&
      new RegExp(`^(pub\\(crate\\) )?(async fn|fn) ${name}\\b`).test(lines[i + 1])
    ) {
      candidates.push(i);
    }
  }
  // dedup: attr line directly above its fn counts once
  const dedup = candidates.filter((idx, i) => {
    const next = candidates[i + 1];
    return !(next === idx + 1 && lines[idx].trim() === "#[tauri::command]");
  });
  candidates.length = 0;
  candidates.push(...dedup);
  if (candidates.length === 0) throw new Error(`item not found: ${name}`);
  if (candidates.length > 1) throw new Error(`item ambiguous: ${name} at ${candidates.map((c) => c + 1).join(",")}`);
  return candidates[0];
}

function itemRegion(startIdx) {
  // walk back over attributes / doc comments / blank-free连续
  let s = startIdx;
  while (s > 0) {
    const l = lines[s - 1];
    if (l.trim().startsWith("#[") || l.trim().startsWith("///") || l.trim().startsWith("//")) s--;
    else break;
  }
  // brace count forward
  let depth = 0, seen = false, e = startIdx;
  while (e < lines.length) {
    for (const ch of lines[e]) {
      if (ch === "{") { depth++; seen = true; }
      else if (ch === "}") depth--;
    }
    if (seen && depth === 0) break;
    e++;
  }
  return [s, e]; // inclusive 0-based
}

const regions = names.map((n) => {
  const idx = findItem(n);
  const [s, e] = itemRegion(idx);
  return { name: n, s, e };
});
// overlap check
regions.sort((a, b) => a.s - b.s);
for (let i = 1; i < regions.length; i++) {
  if (regions[i].s <= regions[i - 1].e) {
    throw new Error(`overlap between ${regions[i - 1].name} and ${regions[i].name}`);
  }
}

let body = "";
for (const { s, e } of regions) {
  let seg = lines.slice(s, e + 1).join("\n");
  seg = seg.replace(/^(async fn|fn|const|static) /gm, "pub(crate) $1 ");
  body += seg + "\n";
}
const header = existsSync(modFile)
  ? readFileSync(modFile, "utf8")
  : `//! Part of the lib.rs decomposition. See crate root for re-exports.\n\n`;
writeFileSync(modFile, header + body);

// delete from lib.rs bottom-up
const drop = new Set();
for (const { s, e } of regions) for (let i = s; i <= e; i++) drop.add(i);
const kept = [];
for (let i = 0; i < lines.length; i++) if (!drop.has(i)) kept.push(lines[i]);
let s2 = kept.join("\n");
// collapse triple blank lines introduced by removals
s2 = s2.replace(/\n{3,}/g, "\n\n");
if (!s2.includes(`mod ${modName};`)) {
  const anchor = /pub\(crate\) use monitors::\*;\n/;
  if (anchor.test(s2)) s2 = s2.replace(anchor, `$&\nmod ${modName};\n\npub(crate) use ${modName}::*;\n`);
  else throw new Error("no anchor for module registration");
}
writeFileSync(p, s2);
console.log(`extracted ${regions.length} items -> ${modFile}`);
for (const r of regions) console.log(`  ${r.name}: lines ${r.s + 1}-${r.e + 1}`);
