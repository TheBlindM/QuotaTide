import assert from "node:assert/strict";

export function releaseArtifactNames(version) {
  assert.match(version, /^\d+\.\d+\.\d+(?:-rc\.\d+)?$/);
  return {
    macDmg: `QuotaTide_${version}_universal.dmg`,
    macArchive: `QuotaTide_${version}_universal.app.tar.gz`,
    macSignature: `QuotaTide_${version}_universal.app.tar.gz.sig`,
    windowsInstaller: `QuotaTide_${version}_x64-setup.exe`,
    windowsSignature: `QuotaTide_${version}_x64-setup.exe.sig`,
    manifest: "latest.json",
    checksums: "SHA256SUMS",
  };
}

export function releaseArtifactInventory(version) {
  return Object.values(releaseArtifactNames(version));
}
