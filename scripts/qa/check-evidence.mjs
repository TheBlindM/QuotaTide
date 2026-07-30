import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { readFile, stat } from "node:fs/promises";
import {
  dirname,
  isAbsolute,
  join,
  normalize,
  relative,
  resolve,
} from "node:path";
import { promisify } from "node:util";

import { releaseArtifactInventory } from "../release/artifacts.mjs";
import {
  PLATFORM_BASELINE_AS_OF,
  REQUIRED_ENVIRONMENTS,
  REQUIRED_RECORDS,
  expectedPlatformIdentity,
  requiredRecordKeys,
} from "./matrix.mjs";

const input = resolve(process.argv[2] ?? "release-evidence.json");
const artifactDirectory = resolve(process.argv[3] ?? "release-assets");
const evidenceDirectory = resolve(process.argv[4] ?? dirname(input));
const execFileAsync = promisify(execFile);
const evidence = JSON.parse(await readFile(input, "utf8"));
const root = new URL("../../", import.meta.url);
const tauri = JSON.parse(
  await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"),
);
const { stdout: currentCommit } = await execFileAsync(
  "git",
  ["rev-parse", "HEAD"],
  { cwd: new URL(root), encoding: "utf8" },
);
assert.equal(evidence.schemaVersion, 1);
assert.equal(evidence.release.product, "QuotaTide");
assert.match(evidence.release.version, /^\d+\.\d+\.\d+(?:-rc\.\d+)?$/);
assert.match(evidence.release.commit, /^[a-f0-9]{40}$/);
assert.ok(!Number.isNaN(Date.parse(evidence.release.generatedAt)));
assert.equal(
  evidence.release.platformBaselineAsOf,
  PLATFORM_BASELINE_AS_OF,
  "Release evidence uses a stale platform baseline",
);
assert.equal(evidence.release.version, tauri.version);
assert.equal(evidence.release.commit, currentCommit.trim());
assert.equal(
  evidence.release.finalCandidate,
  true,
  "Release evidence must identify the exact final candidate",
);
const version = evidence.release.version;
const requiredArtifacts = releaseArtifactInventory(version);
assert.deepEqual(
  evidence.artifacts.map((artifact) => artifact.filename).sort(),
  requiredArtifacts.sort(),
  "Final artifact inventory is incomplete or contains unexpected files",
);
for (const artifact of evidence.artifacts) {
  assert.match(artifact.sha256, /^[a-f0-9]{64}$/);
  assert.ok(artifact.filename && !artifact.filename.includes("/"));
  const bytes = await readFile(join(artifactDirectory, artifact.filename));
  assert.equal(
    createHash("sha256").update(bytes).digest("hex"),
    artifact.sha256,
    `Evidence hash differs from final bytes: ${artifact.filename}`,
  );
}

const records = new Map(
  evidence.records.map((record) => [
    `${record.environmentId}/${record.testId}`,
    record,
  ]),
);
const requiredKeys = requiredRecordKeys();
const blockers = [];
if (evidence.records.length !== requiredKeys.length) {
  blockers.push(
    `record count: expected ${requiredKeys.length}, got ${evidence.records.length}`,
  );
}
if (records.size !== evidence.records.length) {
  blockers.push("duplicate environment/test records");
}
for (const key of requiredKeys) {
  const record = records.get(key);
  const requirement = REQUIRED_RECORDS[key];
  if (!record) {
    blockers.push(`${key}: MISSING`);
    continue;
  }
  const allowedStatuses = requirement.blocking
    ? ["PASS", "N/A"]
    : ["PASS", "FAIL", "N/A"];
  if (!allowedStatuses.includes(record.status)) {
    blockers.push(`${key}: ${record.status}`);
  }
  if (record.status === "N/A" && !record.approvedReason?.trim()) {
    blockers.push(`${key}: N/A without approvedReason`);
  }
  if (
    !requirement.blocking &&
    record.status === "FAIL" &&
    !record.linkedDefect?.trim()
  ) {
    blockers.push(`${key}: compatibility FAIL without linkedDefect`);
  }
  const expectedEnvironment = REQUIRED_ENVIRONMENTS[record.environmentId];
  const expectedIdentity = expectedPlatformIdentity(record.environmentId);
  if (
    !record.executor?.trim() ||
    !record.executedAt ||
    Number.isNaN(Date.parse(record.executedAt)) ||
    !record.osBuild?.trim() ||
    !expectedIdentity.osBuild.test(record.osBuild) ||
    record.cpu !== expectedIdentity.cpu ||
    record.environment !== expectedEnvironment ||
    record.blocking !== requirement.blocking ||
    record.requiredEvidenceType !==
      requirement.requiredEvidenceTypes.join(" + ") ||
    !Object.hasOwn(record, "linkedDefect")
  ) {
    blockers.push(`${key}: incomplete audit fields`);
  }
  const evidenceTypes =
    typeof record.evidenceType === "string"
      ? record.evidenceType.split("+").map((value) => value.trim())
      : [];
  if (
    evidenceTypes.sort().join("+") !==
    [...requirement.requiredEvidenceTypes].sort().join("+")
  ) {
    blockers.push(
      `${key}: evidenceType must be ${requirement.requiredEvidenceTypes.join(" + ")}`,
    );
  }
  const invalidEvidencePaths =
    !Array.isArray(record.evidencePaths) ||
    record.evidencePaths.length === 0 ||
    record.evidencePaths.some(
      (path) =>
        typeof path !== "string" ||
        !path.trim() ||
        isAbsolute(path) ||
        path.includes("\\") ||
        normalize(path).startsWith(".."),
    );
  if (invalidEvidencePaths) {
    blockers.push(`${key}: evidencePaths must be non-empty relative paths`);
  } else {
    for (const path of record.evidencePaths) {
      const resolvedPath = resolve(evidenceDirectory, path);
      const fromEvidenceRoot = relative(evidenceDirectory, resolvedPath);
      if (fromEvidenceRoot.startsWith("..") || isAbsolute(fromEvidenceRoot)) {
        blockers.push(`${key}: evidence path escapes package`);
        continue;
      }
      try {
        const evidenceFile = await stat(resolvedPath);
        if (!evidenceFile.isFile()) {
          blockers.push(`${key}: evidence path is not a file ${path}`);
        }
      } catch {
        blockers.push(`${key}: missing evidence file ${path}`);
      }
    }
  }
  if (record.environmentId.startsWith("W") && !record.webView2Version?.trim()) {
    blockers.push(`${key}: Windows WebView2 version is missing`);
  }
}
if (blockers.length > 0) {
  throw new Error(
    `Release blocked by ${blockers.length} matrix record(s):\n${blockers
      .slice(0, 30)
      .join("\n")}${blockers.length > 30 ? "\n…" : ""}`,
  );
}
console.log(`Release evidence is complete: ${records.size} audited records`);
