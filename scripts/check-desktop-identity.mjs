import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const [workspace, core, desktop, tauri, ui, license] = await Promise.all([
  readFile(new URL("Cargo.toml", root), "utf8"),
  readFile(new URL("crates/quotatide-core/Cargo.toml", root), "utf8"),
  readFile(new URL("src-tauri/Cargo.toml", root), "utf8"),
  readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(JSON.parse),
  readFile(new URL("ui/package.json", root), "utf8").then(JSON.parse),
  readFile(new URL("LICENSE", root), "utf8"),
]);

const repository = workspace.match(
  /\[workspace\.package\][\s\S]*?^repository = "([^"]+)"/m,
)?.[1];

assert.match(core, /^name = "quotatide-core"$/m);
assert.match(desktop, /^name = "quotatide-desktop"$/m);
assert.match(desktop, /^\[\[bin\]\]\nname = "quotatide"$/m);
assert.equal(tauri.productName, "QuotaTide");
assert.equal(tauri.identifier, "dev.theblind.quotatide");
assert.equal(tauri.mainBinaryName, "quotatide");
assert.equal(ui.repository, repository);
assert.match(
  license,
  /^Copyright \(c\) 2026 TheBlind and QuotaTide contributors$/m,
);

console.log("QuotaTide desktop identity is consistent");
