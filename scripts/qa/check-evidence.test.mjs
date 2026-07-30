import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

import {
  REQUIRED_ENVIRONMENTS,
  requiredRecordKeys,
} from "./matrix.mjs";

const execFileAsync = promisify(execFile);
const root = fileURLToPath(new URL("../../", import.meta.url));
const checker = fileURLToPath(new URL("check-evidence.mjs", import.meta.url));

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
  const filenames = [
    `QuotaTide_${version}_universal.dmg`,
    `QuotaTide_${version}_universal.app.tar.gz`,
    `QuotaTide_${version}_universal.app.tar.gz.sig`,
    `QuotaTide_${version}_x64-setup.exe`,
    `QuotaTide_${version}_x64-setup.exe.sig`,
    "latest.json",
    "SHA256SUMS",
  ];
  const artifacts = [];
  for (const filename of filenames) {
    const bytes = Buffer.from(`test artifact: ${filename}\n`);
    await writeFile(join(directory, filename), bytes);
    artifacts.push({
      filename,
      sha256: createHash("sha256").update(bytes).digest("hex"),
    });
  }
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
      return {
        environmentId,
        environment: REQUIRED_ENVIRONMENTS[environmentId],
        testId,
        status: "PASS",
        evidenceType: "AUTO + BUILD",
        executor: "release-tester",
        executedAt: "2026-07-30T00:00:00.000Z",
        osBuild: "test-build",
        cpu: environmentId.endsWith("-A") ? "arm64" : "x86_64",
        webView2Version: environmentId.startsWith("W")
          ? "test-webview2"
          : null,
        evidencePaths: [`evidence/${environmentId}/${testId}.txt`],
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
  assert.match(stdout, /393 audited records/);

  evidence.records[0].osBuild = null;
  await assert.rejects(
    runChecker(evidence, directory),
    /incomplete audit fields/,
  );

  evidence.records[0].osBuild = "test-build";
  evidence.records[1] = structuredClone(evidence.records[0]);
  await assert.rejects(
    runChecker(evidence, directory),
    /duplicate environment\/test records/,
  );
});
