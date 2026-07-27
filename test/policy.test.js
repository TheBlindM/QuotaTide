import test from "node:test";
import assert from "node:assert/strict";
import {
  calculateDelta,
  dailyLimitFor,
  dynamicDailyLimitFor,
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

test("未用完的工作日额度平分给本周剩余工作日", () => {
  const usageByDate = new Map([
    ["2026-07-27", 10],
  ]);
  assert.equal(dynamicDailyLimitFor("2026-07-28", usageByDate), 17.5);
});

test("工作日超用不扣减后续工作日的基础额度", () => {
  const usageByDate = new Map([
    ["2026-07-27", 20],
  ]);
  assert.equal(dynamicDailyLimitFor("2026-07-28", usageByDate), 16);
});

test("缺失的历史工作日不能当作未使用额度结转", () => {
  assert.equal(dynamicDailyLimitFor("2026-07-31", new Map()), 16);
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

test("reset_at 抖动 1 秒不应开启新的额度 epoch", () => {
  assert.equal(
    calculateDelta(
      { usedPercent: 12, resetAt: 1785405709 },
      { usedPercent: 12, resetAt: 1785405710 },
    ),
    0,
  );
});
