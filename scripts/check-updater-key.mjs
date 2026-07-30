import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const [tauri, publicKey, fingerprintFile] = await Promise.all([
  readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(JSON.parse),
  readFile(new URL("src-tauri/updater.pubkey", root), "utf8"),
  readFile(new URL("src-tauri/updater.pubkey.sha256", root), "utf8"),
]);
const fingerprint = createHash("sha256").update(publicKey).digest("hex");
const recorded = fingerprintFile.match(/^([a-f0-9]{64})  updater\.pubkey\n?$/)?.[1];

assert.equal(recorded, fingerprint, "Updater public-key fingerprint is stale");
assert.equal(
  tauri.plugins.updater.pubkey,
  publicKey.trim(),
  "Bundled updater public key differs from updater.pubkey",
);
console.log(`Updater public key and fingerprint agree: ${fingerprint}`);
