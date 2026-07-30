import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { access, readFile, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";

import { releaseArtifactInventory } from "./artifacts.mjs";
import { validateManifest } from "./manifest-lib.mjs";

const directory = resolve(process.argv[2] ?? "release-assets");
const version = process.argv[3];
const includeEvidencePackage = process.argv.includes("--evidence-package");
assert.match(version, /^0\.\d+\.\d+(?:-rc\.\d+)?$/);
const expected = releaseArtifactInventory(version);
if (includeEvidencePackage) {
  expected.push(`release-evidence-${version}.tar.gz`);
}
assert.deepEqual(
  (await readdir(directory)).sort(),
  expected.sort(),
  "Release directory contains missing or unexpected assets",
);
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
