import assert from "node:assert/strict";
import { mkdir, rename, rm, stat } from "node:fs/promises";
import { dirname, resolve } from "node:path";

import sharp from "sharp";

const root = resolve(import.meta.dirname, "../..");
const sourceRoot = resolve(root, "assets/story-source");
const publicRoot = resolve(root, "ui/public/assets");

const assets = [
  {
    source: "tide/chamber-background.png",
    output: "tide/chamber-background-2x.webp",
    width: 931,
    height: 423,
    alpha: false,
    maxBytes: 48_000,
  },
  {
    source: "tide/spritesheet.webp",
    output: "tide/spritesheet-2x.webp",
    width: 768,
    height: 936,
    alpha: true,
    maxBytes: 280_000,
  },
  ...[
    ["rpg-effects.webp", 48_000],
    ["supply-props.webp", 75_000],
    ["survivor-actions.webp", 95_000],
    ["survivor-rpg-actions.webp", 95_000],
    ["zombie-actions.webp", 100_000],
  ].map(([filename, maxBytes]) => ({
    source: `siege-v2/${filename}`,
    output: `siege-v2/${filename.replace(".webp", "-2x.webp")}`,
    width: 768,
    height: 512,
    alpha: true,
    maxBytes,
  })),
];

async function writeAsset(asset) {
  const source = resolve(sourceRoot, asset.source);
  const output = resolve(publicRoot, asset.output);
  const temporary = `${output}.tmp`;
  await mkdir(dirname(output), { recursive: true });
  await sharp(source)
    .resize(asset.width, asset.height, { fit: "fill", kernel: "lanczos3" })
    .webp({
      quality: 88,
      alphaQuality: 100,
      effort: 6,
      smartSubsample: true,
    })
    .toFile(temporary);
  await rm(output, { force: true });
  await rename(temporary, output);
}

async function checkAsset(asset) {
  const output = resolve(publicRoot, asset.output);
  const [metadata, file] = await Promise.all([
    sharp(output).metadata(),
    stat(output),
  ]);
  assert.equal(metadata.format, "webp", `${asset.output} must be WebP`);
  assert.equal(metadata.width, asset.width, `${asset.output} width drifted`);
  assert.equal(metadata.height, asset.height, `${asset.output} height drifted`);
  assert.equal(metadata.hasAlpha, asset.alpha, `${asset.output} alpha drifted`);
  assert.ok(
    file.size <= asset.maxBytes,
    `${asset.output} is ${String(file.size)} bytes; limit ${String(asset.maxBytes)}`,
  );
  return file.size;
}

const write = process.argv.includes("--write");
const check = process.argv.includes("--check");
assert.ok(write || check, "Pass --write or --check");

if (write) {
  for (const asset of assets) {
    await writeAsset(asset);
  }
}

const sizes = await Promise.all(assets.map(checkAsset));
const total = sizes.reduce((sum, size) => sum + size, 0);
assert.ok(total <= 700_000, `Story atlases total ${String(total)} bytes`);
console.log(
  `PASS STORY-ASSETS: ${String(assets.length)} optimized assets, ${String(total)} bytes`,
);
