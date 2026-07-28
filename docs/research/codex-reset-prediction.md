# Codex Resets「24h reset chance」调研

调研时间：2026-07-28 09:47–09:49（Asia/Shanghai）

范围：只检查 `codex-resets.com` 自身页面、该站公开 JavaScript、公开 API，以及 API 返回的原始 X 帖子链接；未依据截图猜测字段。

## 结论

页面上的预测不是从单独的 prediction endpoint 获取的，而是包含在现有公开接口：

```http
GET https://codex-resets.com/api/resets
Accept: application/json
```

前端 `loadData()` 以同源相对地址 `fetch("/api/resets")` 请求它，并只验证响应成功且 `events` 为数组。[来源：app.js](https://codex-resets.com/app.js) [来源：实时 API](https://codex-resets.com/api/resets)

截至本次调研，API 的顶层结构是：

```json
{
  "events": [],
  "watch": {},
  "stats": {},
  "generated_at": "2026-07-28T01:47:11.732Z"
}
```

预测数据位于 `watch`；当前 live response 为：

```json
{
  "level": "strong",
  "tweet_id": "2081899343091843463",
  "tweet_url": "https://x.com/thsottiaux/status/2081899343091843463",
  "text": "…",
  "observed_at": "2026-07-28T00:27:37.000Z",
  "expires_at": "2026-07-29T00:27:37.000Z",
  "window_hours": 24,
  "reset_chance_24h": 75,
  "context_tweet_id": null,
  "context_tweet_url": null,
  "context_text": null
}
```

[来源：实时 API](https://codex-resets.com/api/resets)

## 字段契约

| 字段 | 当前类型 | 含义与使用方式 |
|---|---:|---|
| `level` | string | 当前值 `strong`。前端只认识 `elevated`、`strong`；其他值按 `elevated` 渲染。 |
| `tweet_id` | string | 触发预测的帖子 ID。 |
| `tweet_url` | string | “View on X”链接。 |
| `text` | string | 触发预测的帖子正文，预测卡片直接引用。 |
| `observed_at` | ISO 8601 string | 网站观察到信号的时间；页面显示为“Seen … ago”。 |
| `expires_at` | ISO 8601 string | 预测卡片过期时间。 |
| `window_hours` | number | 预测窗口；页面标题使用 `${window_hours}h reset chance`，非法/缺失时回退为 24。 |
| `reset_chance_24h` | number | 原始概率，单位为百分数、范围意图为 0–100；不是 0–1 小数。 |
| `context_tweet_id` | string/null | 被回复帖 ID；当前为 `null`。 |
| `context_tweet_url` | string/null | 被回复帖 URL；当前为 `null`。 |
| `context_text` | string/null | 被回复帖文本；有值时页面增加“In reply to”上下文，当前为 `null`。 |

字段值来自[实时 API](https://codex-resets.com/api/resets)；字段的渲染/回退逻辑来自[render.js](https://codex-resets.com/render.js)。

## 概率不能按原值直接展示

API 当前返回 `reset_chance_24h: 75`，但官网展示 `>70%`，不是 `75%`。前端先把数值限制到 `0..100`，再对不小于 10 的数值按十位向下取整，并把最大展示档限制为 90：

```js
Math.min(90, Math.floor(rawChance / 10) * 10)
```

因此推荐本项目同时保留：

- `rawChance = 75`，用于数据记录和规则判断；
- `displayChance = ">70%"`，用于忠实复现官网语义。

若概率不是有限数字或小于 10，官网不显示百分比，而显示文本 “Tibo might be hinting at a reset”。[来源：render.js](https://codex-resets.com/render.js)

## 预测依据和引用

官网能证明的预测依据是 `watch.text` 指向的单条 @thsottiaux 帖子，以及可选的 `context_text` 回复上下文。当前触发帖表达了 “I’m feeling like a limit reset”，来源链接为 [@thsottiaux 的原帖](https://x.com/thsottiaux/status/2081899343091843463)；API 当前未提供上下文帖。[来源：实时 API](https://codex-resets.com/api/resets)

官网把该值标为 “AI estimate”，页脚又说明数据来自 @thsottiaux 的帖子并由机器人分类；但公开页面、`app.js`、`render.js` 和 API **没有披露**模型供应商、prompt、特征、训练/校准方法或 75 分的计算公式。因此实现中应把它标注为“第三方 AI 估算”，不能描述成 OpenAI 官方概率或统计保证。[来源：首页](https://codex-resets.com/) [来源：render.js](https://codex-resets.com/render.js)

## 时间和更新语义

- 当前 `observed_at` 到 `expires_at` 正好为 24 小时，与 `window_hours: 24` 一致。[来源：实时 API](https://codex-resets.com/api/resets)
- `generated_at` 是整个 API 响应生成时间，也是页脚 “Last updated …” 的数据源。[来源：render.js](https://codex-resets.com/render.js)
- 连续三次 GET 得到的 `generated_at` 分别为 `01:48:07.572Z`、`01:48:08.791Z`、`01:48:10.039Z`，而 `watch.observed_at`、`expires_at` 与概率不变。因此 `generated_at` 不能当作预测产生时间；预测信号时间应使用 `observed_at`。
- 页面载入后会在 `expires_at` 到达时删除预测卡片；若载入时已经过期则立即删除。[来源：app.js](https://codex-resets.com/app.js)

## 无预测与故障行为

- `watch` 为 `null` 或缺失时，`renderResetWatch()` 返回空字符串，即页面不显示预测卡片。[来源：render.js](https://codex-resets.com/render.js)
- `watch` 存在但概率缺失/非法/小于 10 时，仍展示引用帖和观察时间，只把数字预测替换为文字提示。[来源：render.js](https://codex-resets.com/render.js)
- `expires_at` 无法解析时，客户端不会自动移除已经渲染的卡片；因此本项目消费接口时应自己校验时间并拒绝过期/非法预测。[来源：app.js](https://codex-resets.com/app.js)
- 官网前端遇到网络错误、非 2xx、JSON 解析错误或缺少 `events` 数组时，会记录 warning 并回退到内置 demo 数据；demo 页面会带 `demo data` 标记。[来源：app.js](https://codex-resets.com/app.js) [来源：demo-data.js](https://codex-resets.com/demo-data.js) [来源：render.js](https://codex-resets.com/render.js)
- 本监控服务不应采用官网的 demo fallback，因为那会制造虚假预测；失败时应保留最后一次成功数据并明确标记 stale/error。

## HTTP、缓存与 CORS

2026-07-28 实测：

- `GET /api/resets` 返回 `200 application/json`；`HEAD` 也返回 200。
- `POST` 与 CORS 预检 `OPTIONS` 均返回 `404 Not Found`，所以应只使用 GET。
- 响应头为 `Cache-Control: public, max-age=60`，消费者应允许最多 60 秒缓存；本项目每小时采集一次不会造成压力。
- 带跨域 `Origin` 的 GET 响应没有 `Access-Control-Allow-Origin`，预检也失败。因此浏览器页面无法从另一 origin 直接读取它；应由本项目后端拉取，再通过本项目自己的 API 传给前端。

可复查端点：[公开 API](https://codex-resets.com/api/resets)。

## 集成建议

1. 后端每次正常的一小时采集周期同时 GET `/api/resets`，设置合理超时，失败不影响 Codex 账号额度采集。
2. 严格解析 `watch`：必须存在、`observed_at`/`expires_at` 可解析、`expires_at > now`，才标记为 active。
3. 保存 raw response 所需字段：`level`、`reset_chance_24h`、`window_hours`、`text`、`tweet_url`、`observed_at`、`expires_at`、三项 context 字段、`generated_at`。
4. 页面展示官网语义的档位（例如 75 → `>70%`），同时可在详情中说明“接口原始估算 75%”。
5. 明确标注“来源：codex-resets.com，第三方 AI 估算，非 OpenAI 官方”；附来源帖子链接和数据更新时间。
6. 没有 active watch 时显示“当前无重置预测”；拉取失败时显示“预测数据暂不可用”，不要显示 demo 或把旧值当当前值。

