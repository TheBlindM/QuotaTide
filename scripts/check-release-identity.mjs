import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const DEVELOPMENT_KEY_FINGERPRINT =
  "d6d6db4341f9c138b5a41c2bf47dba68002a8d1d3b8b435d4a76ee8c61ad69d1";
const root = new URL("../", import.meta.url);

const [
  workspace,
  rootPackage,
  ui,
  tauri,
  releaseConfig,
  publicKey,
  fingerprintFile,
] = await Promise.all([
    readFile(new URL("Cargo.toml", root), "utf8"),
    readFile(new URL("package.json", root), "utf8").then(JSON.parse),
    readFile(new URL("ui/package.json", root), "utf8").then(JSON.parse),
    readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(
      JSON.parse,
    ),
    readFile(new URL("src-tauri/tauri.release.conf.json", root), "utf8").then(
      JSON.parse,
    ),
    readFile(new URL("src-tauri/updater.pubkey", root), "utf8"),
    readFile(new URL("src-tauri/updater.pubkey.sha256", root), "utf8"),
  ]);

const repository = workspace.match(
  /\[workspace\.package\][\s\S]*?^repository = "([^"]+)"/m,
)?.[1];
assert.ok(repository, "Release blocked: workspace repository is missing");
assert.doesNotMatch(
  repository,
  /__GITHUB_REPOSITORY__/,
  "Release blocked: bind the public GitHub owner/repository first",
);
assert.match(
  repository,
  /^https:\/\/github\.com\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/,
  "Release blocked: repository must be a canonical HTTPS GitHub URL",
);

const repositorySlug = new URL(repository).pathname.slice(1);
assert.equal(
  rootPackage.repository,
  repository,
  "Release blocked: release-tool repository identity differs from Cargo",
);
assert.equal(
  ui.repository,
  repository,
  "Release blocked: frontend repository identity differs from Cargo",
);
assert.deepEqual(tauri.bundle.targets, ["dmg", "nsis"]);
assert.equal(tauri.bundle.macOS.minimumSystemVersion, "15.0");
assert.equal(tauri.bundle.windows.nsis.installMode, "currentUser");
assert.equal(
  tauri.bundle.windows.webviewInstallMode.type,
  "embedBootstrapper",
);
assert.equal(releaseConfig.bundle.createUpdaterArtifacts, true);
assert.equal(tauri.plugins.updater.windows.installMode, "passive");
assert.deepEqual(tauri.plugins.updater.endpoints, [
  `https://github.com/${repositorySlug}/releases/latest/download/latest.json`,
]);
assert.equal(
  tauri.plugins.updater.pubkey,
  publicKey.trim(),
  "Release blocked: bundled updater key differs from updater.pubkey",
);

const fingerprint = createHash("sha256").update(publicKey).digest("hex");
const recordedFingerprint = fingerprintFile.match(
  /^([a-f0-9]{64})  updater\.pubkey\n?$/,
)?.[1];
assert.equal(
  recordedFingerprint,
  fingerprint,
  "Release blocked: updater public-key fingerprint is stale",
);
assert.notEqual(
  fingerprint,
  DEVELOPMENT_KEY_FINGERPRINT,
  "Release blocked: replace the development-only updater key and complete the two-copy recovery drill",
);

console.log(
  `Release identity is bound to ${repositorySlug}; updater key ${fingerprint}`,
);
