import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const [cargoToml, tauriConfig, uiPackage] = await Promise.all([
  readFile(new URL("Cargo.toml", root), "utf8"),
  readFile(new URL("src-tauri/tauri.conf.json", root), "utf8").then(JSON.parse),
  readFile(new URL("ui/package.json", root), "utf8").then(JSON.parse),
]);

const workspaceVersion = cargoToml.match(
  /\[workspace\.package\][\s\S]*?^version = "([^"]+)"/m,
)?.[1];

if (!workspaceVersion) {
  throw new Error("Cargo workspace version was not found");
}

const versions = new Map([
  ["Cargo workspace", workspaceVersion],
  ["Tauri bundle", tauriConfig.version],
  ["UI package", uiPackage.version],
]);

const mismatches = [...versions].filter(([, version]) => version !== tauriConfig.version);
if (mismatches.length > 0) {
  throw new Error(
    `Desktop versions must match Tauri ${tauriConfig.version}: ${mismatches
      .map(([name, version]) => `${name}=${version}`)
      .join(", ")}`,
  );
}

console.log(`QuotaTide desktop version ${tauriConfig.version} is consistent`);
