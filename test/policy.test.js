import test from "node:test";
import assert from "node:assert/strict";
import {
  calculateDelta,
  dailyLimitFor,
  evaluateDailyPolicy,
} from "../src/policy.js";

test("工作日每日上限为周额度 16%", () => {
  assert.equal(
    dailyLimitFor(new Date("2026-07-24T04:00:00Z"), "Asia/Shanghai"),
    16,
  );
});

test("周末每日上限为周额度 10%", () => {
  assert.equal(
    dailyLimitFor(new Date("2026-07-25T04:00:00Z"), "Asia/Shanghai"),
    10,
  );
});

test("达到当日上限的 80% 时预警，达到 100% 时超额", () => {
  assert.equal(evaluateDailyPolicy(12.79, 16).status, "normal");
  assert.equal(evaluateDailyPolicy(12.8, 16).status, "warning");
  assert.equal(evaluateDailyPolicy(16, 16).status, "exceeded");
});

test("同一额度 epoch 只累计周已用比例的正向增量", () => {
  assert.equal(
    calculateDelta(
      { usedPercent: 20, resetAt: 1000 },
      { usedPercent: 23.5, resetAt: 1000 },
    ),
    3.5,
  );
});

test("重置后把新 epoch 的已用比例加入当天而不是记负数", () => {
  assert.equal(
    calculateDelta(
      { usedPercent: 92, resetAt: 1000 },
      { usedPercent: 4, resetAt: 2000 },
    ),
    4,
  );
});
