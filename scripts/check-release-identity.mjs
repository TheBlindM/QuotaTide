import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const cargoToml = await readFile(new URL("Cargo.toml", root), "utf8");
const repository = cargoToml.match(
  /\[workspace\.package\][\s\S]*?^repository = "([^"]+)"/m,
)?.[1];

if (!repository || repository.includes("__GITHUB_REPOSITORY__")) {
  throw new Error(
    "Release blocked: bind the public GitHub owner/repository before publishing artifacts",
  );
}

console.log(`Release identity is bound to ${repository}`);
