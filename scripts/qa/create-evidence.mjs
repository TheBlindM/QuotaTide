import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { promisify } from "node:util";

import {
  REQUIRED_ENVIRONMENTS,
  REQUIRED_RECORDS,
  requiredRecordKeys,
} from "./matrix.mjs";

const execFileAsync = promisify(execFile);
const output = resolve(process.argv[2] ?? "release-evidence.json");
const root = new URL("../../", import.meta.url);
const tauri = JSON.parse(
  await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"),
);
const { stdout: commit } = await execFileAsync("git", ["rev-parse", "HEAD"], {
  cwd: new URL(root),
  encoding: "utf8",
});
assert.match(tauri.version, /^\d+\.\d+\.\d+(?:-rc\.\d+)?$/);

const generatedAt = new Date().toISOString();
const evidence = {
  schemaVersion: 1,
  release: {
    product: "QuotaTide",
    version: tauri.version,
    commit: commit.trim(),
    generatedAt,
    finalCandidate: false,
  },
  artifacts: [],
  records: requiredRecordKeys().map((key) => {
    const [environmentId, testId] = key.split("/");
    return {
      environmentId,
      environment: REQUIRED_ENVIRONMENTS[environmentId],
      testId,
      blocking: REQUIRED_RECORDS[key].blocking,
      requiredEvidenceType:
        REQUIRED_RECORDS[key].requiredEvidenceTypes.join(" + "),
      status: "BLOCKED",
      evidenceType: null,
      executor: null,
      executedAt: null,
      osBuild: null,
      cpu: null,
      webView2Version: null,
      evidencePaths: [],
      linkedDefect:
        "Final signed release-candidate artifact and required platform are not supplied",
      approvedReason: null,
    };
  }),
};
await writeFile(output, `${JSON.stringify(evidence, null, 2)}\n`, {
  encoding: "utf8",
  mode: 0o644,
});
console.log(`Created explicit BLOCKED matrix with ${evidence.records.length} records`);
