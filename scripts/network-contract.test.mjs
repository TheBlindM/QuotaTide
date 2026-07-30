import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const desktopManifest = await readFile(
  new URL("../src-tauri/Cargo.toml", import.meta.url),
  "utf8",
);

test("desktop HTTP clients inherit Windows and macOS system proxies", () => {
  const reqwestDependency = desktopManifest.match(
    /^reqwest\s*=\s*\{([^}]+)\}/m,
  )?.[1];
  assert.ok(reqwestDependency, "reqwest dependency is missing");
  assert.match(
    reqwestDependency,
    /features\s*=\s*\[[^\]]*"system-proxy"/s,
    "reqwest/system-proxy must remain enabled for native desktop traffic",
  );
});
