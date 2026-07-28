import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = join(repositoryRoot, "assets", "branding");
const outputRoot = join(repositoryRoot, "src-tauri", "icons");
const trayRoot = join(outputRoot, "tray");
const temporaryRoot = mkdtempSync(join(tmpdir(), "quotatide-icons-"));

const sources = {
  app: join(sourceRoot, "app-icon.svg"),
  appSmall: join(sourceRoot, "app-icon-small.svg"),
  trayTemplate: join(sourceRoot, "tray-template.svg"),
  trayTemplateInverse: join(sourceRoot, "tray-template-inverse.svg"),
  trayColor: join(sourceRoot, "tray-color.svg"),
};

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
  });

  if (result.status !== 0) {
    const detail = [result.stdout, result.stderr].filter(Boolean).join("\n");
    throw new Error(`${command} ${arguments_.join(" ")} failed\n${detail}`);
  }
}

function renderPng(source, destination, size) {
  mkdirSync(dirname(destination), { recursive: true });
  run("/usr/bin/sips", [
    "-s",
    "format",
    "png",
    "-z",
    String(size),
    String(size),
    source,
    "--out",
    destination,
  ]);
}

function pngMetadata(path) {
  const buffer = readFileSync(path);
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!buffer.subarray(0, 8).equals(signature)) {
    throw new Error(`${path} is not a PNG`);
  }

  return {
    buffer,
    width: buffer.readUInt32BE(16),
    height: buffer.readUInt32BE(20),
    bitDepth: buffer[24],
    colorType: buffer[25],
  };
}

function verifyPng(path, size) {
  const metadata = pngMetadata(path);
  if (
    metadata.width !== size ||
    metadata.height !== size ||
    metadata.bitDepth !== 8 ||
    metadata.colorType !== 6
  ) {
    throw new Error(
      `${path} must be ${size}x${size} 8-bit RGBA; got ` +
        `${metadata.width}x${metadata.height}, bit depth ${metadata.bitDepth}, ` +
        `color type ${metadata.colorType}`,
    );
  }
}

function writeIco(destination, layers) {
  const images = layers.map(({ path, size }) => {
    const metadata = pngMetadata(path);
    if (metadata.width !== size || metadata.height !== size) {
      throw new Error(`${path} does not match ICO layer ${size}`);
    }
    return { buffer: metadata.buffer, size };
  });

  const headerSize = 6 + images.length * 16;
  const header = Buffer.alloc(headerSize);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);

  let offset = headerSize;
  images.forEach(({ buffer, size }, index) => {
    const entry = 6 + index * 16;
    header[entry] = size === 256 ? 0 : size;
    header[entry + 1] = size === 256 ? 0 : size;
    header[entry + 2] = 0;
    header[entry + 3] = 0;
    header.writeUInt16LE(1, entry + 4);
    header.writeUInt16LE(32, entry + 6);
    header.writeUInt32LE(buffer.length, entry + 8);
    header.writeUInt32LE(offset, entry + 12);
    offset += buffer.length;
  });

  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, Buffer.concat([header, ...images.map(({ buffer }) => buffer)]));
}

function verifyIco(path, expectedSizes) {
  const buffer = readFileSync(path);
  const count = buffer.readUInt16LE(4);
  const sizes = [];

  for (let index = 0; index < count; index += 1) {
    const entry = 6 + index * 16;
    sizes.push(buffer[entry] || 256);
  }

  if (JSON.stringify(sizes) !== JSON.stringify(expectedSizes)) {
    throw new Error(`${path} layers ${sizes.join(",")} do not match ${expectedSizes.join(",")}`);
  }
}

function generateApplicationIcons() {
  const requiredPngs = [
    ["32x32.png", 32, sources.appSmall],
    ["128x128.png", 128, sources.app],
    ["128x128@2x.png", 256, sources.app],
    ["icon.png", 512, sources.app],
  ];

  for (const [name, size, source] of requiredPngs) {
    const destination = join(outputRoot, name);
    renderPng(source, destination, size);
    verifyPng(destination, size);
  }

  const rasterLayers = new Map();
  for (const size of [16, 24, 32, 48, 64, 128, 256, 512, 1024]) {
    const source = size <= 64 ? sources.appSmall : sources.app;
    const destination = join(temporaryRoot, `app-${size}.png`);
    renderPng(source, destination, size);
    verifyPng(destination, size);
    rasterLayers.set(size, destination);
  }

  const icoSizes = [32, 16, 24, 48, 64, 256];
  const icoPath = join(outputRoot, "icon.ico");
  writeIco(
    icoPath,
    icoSizes.map((size) => ({ path: rasterLayers.get(size), size })),
  );
  verifyIco(icoPath, icoSizes);

  const iconset = join(temporaryRoot, "QuotaTide.iconset");
  mkdirSync(iconset);
  const iconsetLayers = [
    ["icon_16x16.png", 16],
    ["icon_16x16@2x.png", 32],
    ["icon_32x32.png", 32],
    ["icon_32x32@2x.png", 64],
    ["icon_128x128.png", 128],
    ["icon_128x128@2x.png", 256],
    ["icon_256x256.png", 256],
    ["icon_256x256@2x.png", 512],
    ["icon_512x512.png", 512],
    ["icon_512x512@2x.png", 1024],
  ];

  for (const [name, size] of iconsetLayers) {
    writeFileSync(join(iconset, name), readFileSync(rasterLayers.get(size)));
  }

  run("/usr/bin/iconutil", [
    "--convert",
    "icns",
    "--output",
    join(outputRoot, "icon.icns"),
    iconset,
  ]);
}

function generateTrayIcons() {
  const pngOutputs = [
    ["tray-template.png", 22, sources.trayTemplate],
    ["tray-template@2x.png", 44, sources.trayTemplate],
    ["tray-template-inverse.png", 22, sources.trayTemplateInverse],
    ["tray-template-inverse@2x.png", 44, sources.trayTemplateInverse],
    ["tray-color.png", 32, sources.trayColor],
    ["tray-color@2x.png", 64, sources.trayColor],
  ];

  for (const [name, size, source] of pngOutputs) {
    const destination = join(trayRoot, name);
    renderPng(source, destination, size);
    verifyPng(destination, size);
  }

  const traySizes = [32, 16, 20, 24, 48, 64];
  for (const [name, source] of [
    ["tray-windows.ico", sources.trayColor],
    ["tray-contrast-dark.ico", sources.trayTemplate],
    ["tray-contrast-light.ico", sources.trayTemplateInverse],
  ]) {
    const layers = traySizes.map((size) => {
      const path = join(temporaryRoot, `${name}-${size}.png`);
      renderPng(source, path, size);
      verifyPng(path, size);
      return { path, size };
    });
    const destination = join(trayRoot, name);
    writeIco(destination, layers);
    verifyIco(destination, traySizes);
  }
}

try {
  mkdirSync(outputRoot, { recursive: true });
  mkdirSync(trayRoot, { recursive: true });
  generateApplicationIcons();
  generateTrayIcons();
  console.log("Generated and verified QuotaTide application and tray icons.");
} finally {
  rmSync(temporaryRoot, { recursive: true, force: true });
}
