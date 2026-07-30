import assert from "node:assert/strict";
import { copyFile, mkdir, readdir } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

const source = resolve(process.argv[2] ?? "downloaded-artifacts");
const output = resolve(process.argv[3] ?? "release-assets");
const version = process.argv[4];
assert.match(version, /^0\.\d+\.\d+(?:-rc\.\d+)?$/);

async function walk(path) {
  const entries = await readdir(path, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const child = join(path, entry.name);
    if (entry.isDirectory()) files.push(...(await walk(child)));
    if (entry.isFile()) files.push(child);
  }
  return files;
}

const files = await walk(source);
function one(label, predicate) {
  const matches = files.filter((path) => predicate(basename(path)));
  assert.equal(
    matches.length,
    1,
    `Expected one ${label}; found ${matches.length}: ${matches.join(", ")}`,
  );
  return matches[0];
}

const macArchive = one("macOS updater archive", (name) =>
  name.endsWith(".app.tar.gz"),
);
const windowsInstaller = one("Windows NSIS installer", (name) =>
  /setup\.exe$/i.test(name),
);
const assets = [
  [one("macOS DMG", (name) => name.endsWith(".dmg")), `QuotaTide_${version}_universal.dmg`],
  [macArchive, `QuotaTide_${version}_universal.app.tar.gz`],
  [
    one("macOS updater signature", (name) =>
      name.endsWith(".app.tar.gz.sig"),
    ),
    `QuotaTide_${version}_universal.app.tar.gz.sig`,
  ],
  [windowsInstaller, `QuotaTide_${version}_x64-setup.exe`],
  [
    one("Windows updater signature", (name) =>
      /setup\.exe\.sig$/i.test(name),
    ),
    `QuotaTide_${version}_x64-setup.exe.sig`,
  ],
];

await mkdir(output, { recursive: true });
await Promise.all(
  assets.map(([from, to]) => copyFile(from, join(output, to))),
);
console.log(`Collected ${assets.length} final-byte artifacts in ${output}`);
