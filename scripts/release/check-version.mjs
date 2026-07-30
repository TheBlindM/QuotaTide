import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const root = new URL("../../", import.meta.url);
const expectedTag = process.argv[2] ?? process.env.GITHUB_REF_NAME;
assert.ok(expectedTag, "Pass the release tag, for example v0.1.0");
assert.match(
  expectedTag,
  /^v0\.\d+\.\d+(?:-rc\.\d+)?$/,
  "Release tag must follow v0.MINOR.PATCH or v0.MINOR.PATCH-rc.N",
);

const [workspace, tauri, ui] = await Promise.all([
  readFile(new URL("Cargo.toml", root), "utf8"),
  readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(JSON.parse),
  readFile(new URL("ui/package.json", root), "utf8").then(JSON.parse),
]);
const workspaceVersion = workspace.match(
  /\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m,
)?.[1];
const version = expectedTag.slice(1);

assert.equal(workspaceVersion, version, "Cargo workspace version differs");
assert.equal(tauri.version, version, "Tauri version differs");
assert.equal(ui.version, version, "UI version differs");
console.log(`Release version ${version} matches tag ${expectedTag}`);
