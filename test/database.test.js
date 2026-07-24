import test from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { QuotaDatabase } from "../src/database.js";

function usage(usedPercent, resetAt = 2_000_000_000) {
  return {
    usedPercent,
    remainingPercent: 100 - usedPercent,
    resetAt,
    windowSeconds: 604800,
    planType: "plus",
    allowed: true,
    resetCredits: 0,
    userId: "user-1",
    email: "person@example.com",
  };
}

test("每日使用跨普通快照和中途重置持续累计", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "codex-quota-test-"));
  const database = new QuotaDatabase(
    path.join(directory, "test.sqlite"),
    "Asia/Shanghai",
  );
  try {
    const first = Date.parse("2026-07-24T01:00:00Z");
    database.recordSnapshot(usage(20), first);
    database.recordSnapshot(usage(25), first + 60 * 60 * 1000);
    database.recordSnapshot(
      usage(2, 2_000_604_800),
      first + 2 * 60 * 60 * 1000,
    );
    const status = database.status(first + 2 * 60 * 60 * 1000);
    assert.equal(status.today.used, 7);
    assert.equal(status.today.limit, 16);
    assert.equal(status.today.status, "normal");
  } finally {
    database.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("首次读取雷达建立基线，之后不同事件才算新公告", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "codex-radar-test-"));
  const database = new QuotaDatabase(
    path.join(directory, "test.sqlite"),
    "Asia/Shanghai",
  );
  try {
    const first = database.recordRadar({ latest: { id: "event-1" } });
    const same = database.recordRadar({ latest: { id: "event-1" } });
    const next = database.recordRadar({ latest: { id: "event-2" } });
    assert.equal(first.isNew, false);
    assert.equal(same.isNew, false);
    assert.equal(next.isNew, true);
  } finally {
    database.close();
    rmSync(directory, { recursive: true, force: true });
  }
});
