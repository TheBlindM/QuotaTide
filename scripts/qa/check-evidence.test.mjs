import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import { releaseArtifactInventory } from "../release/artifacts.mjs";
import {
  REQUIRED_ENVIRONMENTS,
  REQUIRED_RECORDS,
  expectedPlatformIdentity,
  requiredRecordKeys,
} from "./matrix.mjs";

const execFileAsync = promisify(execFile);
const root = fileURLToPath(new URL("../../", import.meta.url));
const checker = fileURLToPath(new URL("check-evidence.mjs", import.meta.url));

function validOsBuild(environmentId) {
  if (environmentId.startsWith("M15")) return "macOS 15.7.8 (build 24G222)";
  if (environmentId === "MC-A") return "macOS 26.6 (build 25G86)";
  if (environmentId === "M14-C") return "macOS 14.8.8 (build 23J123)";
  if (environmentId === "W10-C") {
    return "Windows 10 22H2 (build 19045.6216)";
  }
  if (environmentId.startsWith("W25")) {
    return "Windows 11 25H2 (build 26200.1000)";
  }
  if (environmentId === "W26-X") {
    return "Windows 11 26H1 (build 26300.1000)";
  }
  if (environmentId === "W24-C") {
    return "Windows 11 24H2 (build 26100.4946)";
  }
  throw new Error(`Missing test platform identity for ${environmentId}`);
}

async function createCompleteEvidence(directory) {
  const tauri = JSON.parse(
    await readFile(join(root, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const { stdout: commit } = await execFileAsync(
    "git",
    ["rev-parse", "HEAD"],
    { cwd: root, encoding: "utf8" },
  );
  const version = tauri.version;
  const filenames = releaseArtifactInventory(version);
  const artifacts = [];
  for (const filename of filenames) {
    const bytes = Buffer.from(`test artifact: ${filename}\n`);
    await writeFile(join(directory, filename), bytes);
    artifacts.push({
      filename,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
  await mkdir(join(directory, "evidence"));
  await writeFile(join(directory, "evidence", "shared.txt"), "test evidence\n");
  return {
    schemaVersion: 1,
    release: {
      product: "QuotaTide",
      version,
      commit: commit.trim(),
      generatedAt: "2026-07-30T00:00:00.000Z",
      finalCandidate: true,
    },
    artifacts,
    records: requiredRecordKeys().map((key) => {
      const [environmentId, testId] = key.split("/");
      const requirement = REQUIRED_RECORDS[key];
      const identity = expectedPlatformIdentity(environmentId);
      return {
        environmentId,
        environment: REQUIRED_ENVIRONMENTS[environmentId],
        testId,
        blocking: requirement.blocking,
        requiredEvidenceType: requirement.requiredEvidenceTypes.join(" + "),
        status: "PASS",
        evidenceType: requirement.requiredEvidenceTypes.join(" + "),
        executor: "release-tester",
        executedAt: "2026-07-30T00:00:00.000Z",
        osBuild: validOsBuild(environmentId),
        cpu: identity.cpu,
        webView2Version: environmentId.startsWith("W")
          ? "test-webview2"
          : null,
        evidencePaths: ["evidence/shared.txt"],
        linkedDefect: null,
        approvedReason: null,
      };
    }),
  };
}

async function runChecker(evidence, directory) {
  const evidencePath = join(directory, "release-evidence.json");
  await writeFile(evidencePath, `${JSON.stringify(evidence, null, 2)}\n`);
  return execFileAsync(
    process.execPath,
    [checker, evidencePath, directory],
    { cwd: root, encoding: "utf8" },
  );
}

test("release evidence gate accepts only a complete audited matrix", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "quotatide-evidence-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const evidence = await createCompleteEvidence(directory);

  const { stdout } = await runChecker(evidence, directory);
  assert.match(stdout, /400 audited records/);

  evidence.records[0].osBuild = null;
  await assert.rejects(
    runChecker(evidence, directory),
    /incomplete audit fields/,
  );

  evidence.records[0].osBuild = validOsBuild(evidence.records[0].environmentId);
  evidence.records[1] = structuredClone(evidence.records[0]);
  await assert.rejects(
    runChecker(evidence, directory),
    /duplicate environment\/test records/,
  );
});
