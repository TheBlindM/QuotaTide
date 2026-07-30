import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { access, readFile } from "node:fs/promises";
import { join, resolve } from "node:path";

import { validateManifest } from "./manifest-lib.mjs";

const directory = resolve(process.argv[2] ?? "release-assets");
const version = process.argv[3];
assert.match(version, /^0\.\d+\.\d+(?:-rc\.\d+)?$/);
const expected = [
  `QuotaTide_${version}_universal.dmg`,
  `QuotaTide_${version}_universal.app.tar.gz`,
  `QuotaTide_${version}_universal.app.tar.gz.sig`,
  `QuotaTide_${version}_x64-setup.exe`,
  `QuotaTide_${version}_x64-setup.exe.sig`,
  "latest.json",
  "SHA256SUMS",
];
await Promise.all(expected.map((name) => access(join(directory, name))));

const manifest = JSON.parse(
  await readFile(join(directory, "latest.json"), "utf8"),
);
validateManifest(manifest);
assert.equal(manifest.version, version);

const checksumLines = (
  await readFile(join(directory, "SHA256SUMS"), "utf8")
)
  .trim()
  .split("\n");
for (const line of checksumLines) {
  const match = line.match(/^([a-f0-9]{64})  ([^/\\]+)$/);
  assert.ok(match, `Invalid checksum line: ${line}`);
  const bytes = await readFile(join(directory, match[2]));
  assert.equal(createHash("sha256").update(bytes).digest("hex"), match[1]);
}
console.log("Release assets, manifest, and final-byte checksums are complete");
