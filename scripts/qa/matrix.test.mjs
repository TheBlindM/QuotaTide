import assert from "node:assert/strict";
import test from "node:test";

import {
  REQUIRED_RECORDS,
  REQUIRED_TEST_IDS,
  requiredRecordKeys,
  testAppliesTo,
} from "./matrix.mjs";

test("release matrix includes every documented gate group", () => {
  for (const id of [
    "PKG-09",
    "SHELL-06",
    "FX-04",
    "NOTIFY-04",
    "START-03",
    "FILE-03",
    "VAULT-05",
    "SMTP-05",
    "DB-08",
    "CORE-06",
    "UPDATE-08",
    "SEC-04",
    "L10N-03",
    "A11Y-07",
    "PERF-07",
  ]) {
    assert.ok(REQUIRED_TEST_IDS.includes(id), `${id} missing`);
  }
  assert.equal(requiredRecordKeys().length, 401);
  assert.equal(REQUIRED_RECORDS["W25-B/WEBVIEW-02"].blocking, true);
  assert.equal(REQUIRED_RECORDS["M14-A/COMPAT-01"].blocking, false);
  assert.equal(REQUIRED_RECORDS["M14-I/COMPAT-01"].blocking, false);
});

test("architecture and screen-reader gates target the correct platforms", () => {
  assert.equal(testAppliesTo("PKG-01", "M15-A"), true);
  assert.equal(testAppliesTo("PKG-01", "W25-X"), false);
  assert.equal(testAppliesTo("A11Y-05", "M15-A"), false);
  assert.equal(testAppliesTo("A11Y-05", "W25-X"), true);
});

test("manual and live gates cannot be replaced with automated evidence", () => {
  assert.deepEqual(
    REQUIRED_RECORDS["MC-A/A11Y-04"].requiredEvidenceTypes,
    ["MANUAL"],
  );
  assert.deepEqual(
    REQUIRED_RECORDS["W25-X/SMTP-01"].requiredEvidenceTypes,
    ["LIVE"],
  );
});
