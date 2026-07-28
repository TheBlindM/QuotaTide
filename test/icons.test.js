import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import test from "node:test";

const repositoryRoot = new URL("../", import.meta.url);

async function read(relativePath) {
  return readFile(new URL(relativePath, repositoryRoot));
}

function pngMetadata(buffer) {
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  assert.deepEqual(buffer.subarray(0, 8), signature);
  return {
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
    bitDepth: buffer[24],
    colorType: buffer[25],
  };
}

function icoSizes(buffer) {
  assert.equal(buffer.readUInt16LE(0), 0);
  assert.equal(buffer.readUInt16LE(2), 1);
  const count = buffer.readUInt16LE(4);
  const sizes = [];

  for (let index = 0; index < count; index += 1) {
    const entry = 6 + index * 16;
    sizes.push(buffer[entry] || 256);
  }
  return sizes;
}

test("Tauri PNG application icons are square 8-bit RGBA assets", async () => {
  const expected = new Map([
    ["src-tauri/icons/32x32.png", 32],
    ["src-tauri/icons/128x128.png", 128],
    ["src-tauri/icons/128x128@2x.png", 256],
    ["src-tauri/icons/icon.png", 512],
  ]);

  for (const [path, size] of expected) {
    const metadata = pngMetadata(await read(path));
    assert.deepEqual(metadata, {
      width: size,
      height: size,
      bitDepth: 8,
      colorType: 6,
    });
  }
});

test("Windows application and tray icons contain the required layer order", async () => {
  assert.deepEqual(icoSizes(await read("src-tauri/icons/icon.ico")), [
    32, 16, 24, 48, 64, 256,
  ]);

  for (const name of [
    "tray-windows.ico",
    "tray-contrast-dark.ico",
    "tray-contrast-light.ico",
  ]) {
    assert.deepEqual(icoSizes(await read(join("src-tauri/icons/tray", name))), [
      32, 16, 20, 24, 48, 64,
    ]);
  }
});

test("macOS application icon is an ICNS container", async () => {
  const buffer = await read("src-tauri/icons/icon.icns");
  assert.equal(buffer.subarray(0, 4).toString("ascii"), "icns");
  assert.equal(buffer.readUInt32BE(4), buffer.length);
});

test("production SVGs contain no text glyphs or OpenAI branding", async () => {
  for (const name of [
    "app-icon.svg",
    "app-icon-small.svg",
    "tray-template.svg",
    "tray-template-inverse.svg",
    "tray-color.svg",
  ]) {
    const svg = (await read(join("assets/branding", name))).toString("utf8");
    assert.doesNotMatch(svg, /<text\b/i);
    assert.doesNotMatch(svg, /openai/i);
    assert.doesNotMatch(svg, /chatgpt/i);
  }
});
