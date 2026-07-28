import test from "node:test";
import assert from "node:assert/strict";
import { parseAuthJson } from "../src/auth-file.js";
import { normalizeUsage } from "../src/codex-client.js";
import { normalizeRadar } from "../src/resets-client.js";

test("读取 Codex CLI 嵌套 auth.json", () => {
  const auth = parseAuthJson(
    JSON.stringify({
      tokens: {
        access_token: "header.payload.signature",
        account_id: "account-123",
      },
    }),
  );
  assert.equal(auth.accessToken, "header.payload.signature");
  assert.equal(auth.accountId, "account-123");
});

test("读取 New API 扁平凭证格式", () => {
  const auth = parseAuthJson(
    JSON.stringify({
      access_token: "header.payload.signature",
      account_id: "account-456",
    }),
  );
  assert.equal(auth.accountId, "account-456");
});

test("从 access token claim 提取 account_id", () => {
  const payload = Buffer.from(
    JSON.stringify({
      "https://api.openai.com/auth.chatgpt_account_id": "account-from-jwt",
    }),
  ).toString("base64url");
  const auth = parseAuthJson(
    JSON.stringify({ tokens: { access_token: `header.${payload}.signature` } }),
  );
  assert.equal(auth.accountId, "account-from-jwt");
});

test("额度响应按窗口长度识别每周窗口", () => {
  const usage = normalizeUsage({
    plan_type: "plus",
    rate_limit: {
      allowed: true,
      primary_window: {
        used_percent: 30,
        reset_at: 100,
        limit_window_seconds: 18000,
      },
      secondary_window: {
        used_percent: 42.5,
        reset_at: 200,
        reset_after_seconds: 800,
        limit_window_seconds: 604800,
      },
    },
  });
  assert.equal(usage.usedPercent, 42.5);
  assert.equal(usage.remainingPercent, 57.5);
  assert.equal(usage.resetAt, 200);
});

test("重置雷达只保留展示所需的公开字段", () => {
  const radar = normalizeRadar(
    {
      events: [
        {
          tweet_id: "123",
          tweet_url: "https://x.com/example",
          text: "reset",
          announced_at: "2026-07-24T00:00:00Z",
        },
      ],
      stats: { total: 10, avg_interval_days: 8.8 },
      generated_at: "2026-07-24T01:00:00Z",
      watch: {
        level: "strong",
        tweet_id: "456",
        tweet_url: "https://x.com/example/status/456",
        text: "I am feeling like a limit reset.",
        observed_at: "2026-07-28T00:27:37Z",
        expires_at: "2026-07-29T00:27:37Z",
        window_hours: 24,
        reset_chance_24h: 75,
      },
    },
    Date.parse("2026-07-28T01:00:00Z"),
  );
  assert.deepEqual(radar.latest, {
    id: "123",
    url: "https://x.com/example",
    text: "reset",
    announcedAt: "2026-07-24T00:00:00Z",
  });
  assert.deepEqual(radar.watch, {
    level: "strong",
    chancePercent: 75,
    observedAt: "2026-07-28T00:27:37Z",
    expiresAt: "2026-07-29T00:27:37Z",
    windowHours: 24,
    source: {
      id: "456",
      url: "https://x.com/example/status/456",
      text: "I am feeling like a limit reset.",
    },
  });
});

test("过期或无效的重置预测不进入页面状态", () => {
  const expired = normalizeRadar(
    {
      watch: {
        reset_chance_24h: 75,
        expires_at: "2026-07-28T00:27:37Z",
      },
    },
    Date.parse("2026-07-28T01:00:00Z"),
  );
  const invalid = normalizeRadar(
    {
      watch: {
        reset_chance_24h: 101,
        expires_at: "2026-07-29T00:27:37Z",
      },
    },
    Date.parse("2026-07-28T01:00:00Z"),
  );

  assert.equal(expired.watch, null);
  assert.equal(invalid.watch, null);
});
