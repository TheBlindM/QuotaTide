import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readdir, readFile, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

const directory = resolve(process.argv[2] ?? "release-assets");
const outputName = "SHA256SUMS";
const names = (await readdir(directory))
  .filter((name) => name !== outputName)
  .sort((left, right) => left.localeCompare(right, "en"));
assert.ok(names.length >= 6, "Release asset set is incomplete");

const lines = [];
for (const name of names) {
  const bytes = await readFile(join(directory, name));
  lines.push(`${createHash("sha256").update(bytes).digest("hex")}  ${basename(name)}`);
}
await writeFile(join(directory, outputName), `${lines.join("\n")}\n`, {
  encoding: "utf8",
  mode: 0o644,
});
console.log(`Wrote ${names.length} checksums`);
