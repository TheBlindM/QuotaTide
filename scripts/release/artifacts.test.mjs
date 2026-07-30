import assert from "node:assert/strict";
import test from "node:test";

import {
  releaseArtifactInventory,
  releaseArtifactNames,
} from "./artifacts.mjs";

test("release artifact catalog is unique and complete", () => {
  const names = releaseArtifactNames("0.1.0");
  const inventory = releaseArtifactInventory("0.1.0");
  assert.equal(inventory.length, 7);
  assert.equal(new Set(inventory).size, inventory.length);
  assert.equal(names.macDmg, "QuotaTide_0.1.0_universal.dmg");
  assert.equal(names.windowsInstaller, "QuotaTide_0.1.0_x64-setup.exe");
  assert.ok(inventory.includes("latest.json"));
  assert.ok(inventory.includes("SHA256SUMS"));
});
