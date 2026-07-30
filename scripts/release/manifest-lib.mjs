import assert from "node:assert/strict";

export const PLATFORM_KEYS = [
  "darwin-aarch64",
  "darwin-x86_64",
  "windows-x86_64",
];

export function createManifest({
  version,
  repository,
  notes,
  pubDate,
  macArchiveName,
  macSignature,
  windowsInstallerName,
  windowsSignature,
}) {
  assert.match(version, /^0\.\d+\.\d+(?:-rc\.\d+)?$/);
  assert.match(repository, /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/);
  assert.match(pubDate, /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/);
  assert.match(macArchiveName, /\.app\.tar\.gz$/);
  assert.match(windowsInstallerName, /setup\.exe$/i);
  assert.ok(macSignature.trim().length > 40, "macOS signature is missing");
  assert.ok(
    windowsSignature.trim().length > 40,
    "Windows signature is missing",
  );

  const releaseBase = `https://github.com/${repository}/releases/download/v${version}`;
  const macEntry = {
    signature: macSignature.trim(),
    url: `${releaseBase}/${encodeURIComponent(macArchiveName)}`,
  };
  return {
    version,
    notes,
    pub_date: pubDate,
    platforms: {
      "darwin-aarch64": macEntry,
      "darwin-x86_64": { ...macEntry },
      "windows-x86_64": {
        signature: windowsSignature.trim(),
        url: `${releaseBase}/${encodeURIComponent(windowsInstallerName)}`,
      },
    },
  };
}

export function validateManifest(manifest) {
  assert.deepEqual(Object.keys(manifest.platforms), PLATFORM_KEYS);
  const arm = manifest.platforms["darwin-aarch64"];
  const intel = manifest.platforms["darwin-x86_64"];
  assert.deepEqual(arm, intel, "macOS entries must share one universal artifact");
  for (const platform of PLATFORM_KEYS) {
    const entry = manifest.platforms[platform];
    assert.match(entry.url, /^https:\/\//);
    assert.ok(!entry.signature.endsWith(".sig"));
    assert.ok(!/^https?:\/\//i.test(entry.signature));
  }
}
