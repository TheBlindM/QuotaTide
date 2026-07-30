import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { basename, resolve } from "node:path";

import { createManifest, validateManifest } from "./manifest-lib.mjs";

const options = Object.fromEntries(
  process.argv
    .slice(2)
    .map((value, index, values) =>
      value.startsWith("--") ? [value.slice(2), values[index + 1]] : null,
    )
    .filter(Boolean),
);
for (const name of [
  "version",
  "repository",
  "mac-archive",
  "mac-signature",
  "windows-installer",
  "windows-signature",
  "output",
]) {
  assert.ok(options[name], `Missing --${name}`);
}

const [macSignature, windowsSignature] = await Promise.all([
  readFile(resolve(options["mac-signature"]), "utf8"),
  readFile(resolve(options["windows-signature"]), "utf8"),
]);
const pubDate =
  options["pub-date"] ??
  new Date(Number(process.env.SOURCE_DATE_EPOCH ?? 0) * 1000).toISOString();
const manifest = createManifest({
  version: options.version,
  repository: options.repository,
  notes: options.notes ?? `QuotaTide ${options.version}`,
  pubDate,
  macArchiveName: basename(options["mac-archive"]),
  macSignature,
  windowsInstallerName: basename(options["windows-installer"]),
  windowsSignature,
});
validateManifest(manifest);
await writeFile(
  resolve(options.output),
  `${JSON.stringify(manifest, null, 2)}\n`,
  { encoding: "utf8", mode: 0o644 },
);
console.log(`Wrote deterministic updater manifest to ${options.output}`);
