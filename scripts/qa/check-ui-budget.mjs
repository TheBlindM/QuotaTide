import assert from "node:assert/strict";
import { gzipSync } from "node:zlib";
import { readdir, readFile } from "node:fs/promises";
import { extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const UI_CODE_GZIP_BUDGET = 100 * 1024;
export const UI_VISUAL_GZIP_BUDGET = 7 * 1024 * 1024;

const VISUAL_ASSET_EXTENSIONS = new Set([
  ".avif",
  ".gif",
  ".ico",
  ".jpeg",
  ".jpg",
  ".png",
  ".webp",
]);

export function isVisualAsset(path) {
  return VISUAL_ASSET_EXTENSIONS.has(extname(path).toLowerCase());
}

async function walk(path, files) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) await walk(child, files);
    if (entry.isFile() && !entry.name.endsWith(".map")) files.push(child);
  }
}

export async function measureUiBundle(directory) {
  const files = [];
  await walk(directory, files);
  assert.ok(files.length > 0, "Production UI bundle is missing");

  const entries = [];
  let codeTotal = 0;
  let visualTotal = 0;
  for (const file of files.sort()) {
    const size = gzipSync(await readFile(file), { level: 9 }).byteLength;
    const visual = isVisualAsset(file);
    if (visual) visualTotal += size;
    else codeTotal += size;
    entries.push({ file, size, visual });
  }
  return { codeTotal, entries, visualTotal };
}

export function assertUiBudgets(
  measurement,
  codeBudget = UI_CODE_GZIP_BUDGET,
  visualBudget = UI_VISUAL_GZIP_BUDGET,
) {
  assert.ok(
    measurement.codeTotal <= codeBudget,
    `UI code gzip budget exceeded: ${measurement.codeTotal} bytes`,
  );
  assert.ok(
    measurement.visualTotal <= visualBudget,
    `UI visual asset gzip budget exceeded: ${measurement.visualTotal} bytes`,
  );
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : null;
if (invokedPath === fileURLToPath(import.meta.url)) {
  const directory = resolve(process.argv[2] ?? "ui/dist");
  const measurement = await measureUiBundle(directory);
  for (const entry of measurement.entries) {
    const kind = entry.visual ? "visual" : "code";
    console.log(
      `${entry.size.toString().padStart(7)}  ${kind.padEnd(6)}  ${entry.file.slice(directory.length + 1)}`,
    );
  }
  assertUiBudgets(measurement);
  console.log(
    `PASS PERF-01: code ${measurement.codeTotal} / ${UI_CODE_GZIP_BUDGET}; visual ${measurement.visualTotal} / ${UI_VISUAL_GZIP_BUDGET} gzip bytes`,
  );
}
