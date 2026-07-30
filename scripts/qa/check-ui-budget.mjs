import assert from "node:assert/strict";
import { gzipSync } from "node:zlib";
import { readdir, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

const directory = resolve(process.argv[2] ?? "ui/dist");
const files = [];
async function walk(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) await walk(child);
    if (entry.isFile() && !entry.name.endsWith(".map")) files.push(child);
  }
}
await walk(directory);
assert.ok(files.length > 0, "Production UI bundle is missing");

let total = 0;
for (const file of files.sort()) {
  const size = gzipSync(await readFile(file), { level: 9 }).byteLength;
  total += size;
  console.log(`${size.toString().padStart(7)}  ${file.slice(directory.length + 1)}`);
}
assert.ok(total <= 100 * 1024, `UI gzip budget exceeded: ${total} bytes`);
console.log(`PASS PERF-01: ${total} / ${100 * 1024} gzip bytes`);
