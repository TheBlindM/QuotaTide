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

test("reset_at 小幅抖动不会重复累计，且可修复历史汇总与错误告警", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "codex-jitter-test-"));
  const database = new QuotaDatabase(
    path.join(directory, "test.sqlite"),
    "Asia/Shanghai",
  );
  try {
    const first = Date.parse("2026-07-25T01:00:00Z");
    database.recordSnapshot(usage(12, 1785405709), first);
    database.recordSnapshot(usage(12, 1785405710), first + 60 * 60 * 1000);
    assert.equal(database.status(first + 60 * 60 * 1000).today.used, 0);

    database.db
      .prepare(
        `UPDATE daily_usage
         SET used_percent = 24, status = 'exceeded'
         WHERE local_date = '2026-07-25'`,
      )
      .run();
    database.createAlert({
      key: "2026-07-25:daily-exceeded",
      type: "daily_exceeded",
      date: "2026-07-25",
      detail: "legacy incorrect alert",
    });

    database.reconcileDerivedData();
    const repaired = database.status(first + 60 * 60 * 1000);
    assert.equal(repaired.today.used, 0);
    assert.equal(repaired.today.status, "normal");
    assert.equal(repaired.alerts.length, 0);
  } finally {
    database.close();
    rmSync(directory, { recursive: true, force: true });
  }
});

test("数据库状态把前一工作日结余平分给剩余工作日", () => {
  const directory = mkdtempSync(path.join(os.tmpdir(), "codex-carry-test-"));
  const database = new QuotaDatabase(
    path.join(directory, "test.sqlite"),
    "Asia/Shanghai",
  );
  try {
    const mondayMorning = Date.parse("2026-07-27T01:00:00Z");
    const mondayEvening = Date.parse("2026-07-27T10:00:00Z");
    const tuesdayMorning = Date.parse("2026-07-28T01:00:00Z");
    database.recordSnapshot(usage(0), mondayMorning);
    database.recordSnapshot(usage(10), mondayEvening);
    database.recordSnapshot(usage(10), tuesdayMorning);

    const monday = database.status(mondayEvening);
    const tuesday = database.status(tuesdayMorning);
    assert.equal(monday.today.limit, 16);
    assert.equal(tuesday.today.limit, 17.5);
    assert.equal(tuesday.today.used, 0);
  } finally {
    database.close();
    rmSync(directory, { recursive: true, force: true });
  }
});
