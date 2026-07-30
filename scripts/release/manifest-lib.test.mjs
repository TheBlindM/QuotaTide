import assert from "node:assert/strict";
import test from "node:test";

import { createManifest, validateManifest } from "./manifest-lib.mjs";

const signature = "A".repeat(80);

test("manifest has exactly three entries and shares the universal archive", () => {
  const manifest = createManifest({
    version: "0.1.0",
    repository: "TheBlind/QuotaTide",
    notes: "Preview",
    pubDate: "2026-07-30T00:00:00.000Z",
    macArchiveName: "QuotaTide_0.1.0_universal.app.tar.gz",
    macSignature: signature,
    windowsInstallerName: "QuotaTide_0.1.0_x64-setup.exe",
    windowsSignature: signature,
  });
  validateManifest(manifest);
  assert.deepEqual(
    manifest.platforms["darwin-aarch64"],
    manifest.platforms["darwin-x86_64"],
  );
});

test("signature paths are rejected", () => {
  const manifest = createManifest({
    version: "0.1.0",
    repository: "TheBlind/QuotaTide",
    notes: "Preview",
    pubDate: "2026-07-30T00:00:00.000Z",
    macArchiveName: "QuotaTide_0.1.0_universal.app.tar.gz",
    macSignature: `release/${"a".repeat(60)}.sig`,
    windowsInstallerName: "QuotaTide_0.1.0_x64-setup.exe",
    windowsSignature: signature,
  });
  assert.throws(() => validateManifest(manifest));
});
