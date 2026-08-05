# Codex 账号额度与重置雷达：一手来源调研

调研日期：2026-07-24（Asia/Shanghai）

范围：

- 只读检查 `/Volumes/NewSSD/IdeaProjects/new-api-main` 中 Codex 账号用量实现。
- 检查 `https://www.codexrunway.com/` 页面及其公开网络接口。
- 未读取、输出或保存任何真实 `auth.json` 内容或凭证。

## 结论摘要

1. New API 直接向 ChatGPT 上游查询账号额度：`GET https://chatgpt.com/backend-api/wham/usage`。它不是根据请求日志估算周额度。[New API `service/codex_wham_usage.go:15-54`](../../../new-api-main/service/codex_wham_usage.go#L15-L54) [默认基址 `constant/channel.go:57,121`](../../../new-api-main/constant/channel.go#L57)
2. 请求至少需要 `access_token` 和 `account_id`，分别放入 `Authorization: Bearer …` 与 `chatgpt-account-id`；另带 `Accept: application/json` 和 `originator: codex_cli_rs`。[New API `service/codex_wham_usage.go:154-160`](../../../new-api-main/service/codex_wham_usage.go#L154-L160)
3. 上游响应直接提供 `used_percent`、`reset_at`、`reset_after_seconds`、`limit_window_seconds`，以及账号/套餐/是否受限等字段。周窗口不需要从 Codex Resets 推算。[New API UI 类型 `codex-usage-dialog.tsx:69-128`](../../../new-api-main/web/default/src/features/channels/components/dialogs/codex-usage-dialog.tsx#L69-L128)
4. `https://www.codexrunway.com/api/status.json` 是无需认证的公开静态 JSON feed，记录基于 `@thsottiaux` X 公告识别出的计划重置与已完成重置事件；它不是某个账号的额度接口，也不能覆盖个人滚动周窗口。[Codex Runway Reset Watch](https://www.codexrunway.com/) [公开 JSON](https://www.codexrunway.com/api/status.json)
5. 因此产品应有两个独立数据源：
   - **账号事实源**：`wham/usage`，决定周已用量、剩余量、账号是否可用、账号窗口何时重置。
   - **全局事件雷达**：Codex Resets，展示最近一次额外重置公告并辅助识别额度突然回升的原因；不能覆盖 `wham/usage.reset_at`。

## 1. New API 如何查询 Codex 账号用量

### 1.1 上游请求

| 项目 | 值 | 一手来源 |
|---|---|---|
| 默认基址 | `https://chatgpt.com`（Codex channel type 57） | [`constant/channel.go:57,63-123`](../../../new-api-main/constant/channel.go#L57) |
| 用量 URL | `{baseURL}/backend-api/wham/usage` | [`service/codex_wham_usage.go:38`](../../../new-api-main/service/codex_wham_usage.go#L38) |
| HTTP method | `GET` | [`service/codex_wham_usage.go:38`](../../../new-api-main/service/codex_wham_usage.go#L38) |
| Body | 无 | [`service/codex_wham_usage.go:38`](../../../new-api-main/service/codex_wham_usage.go#L38) |
| `Authorization` | `Bearer {access_token}` | [`service/codex_wham_usage.go:155`](../../../new-api-main/service/codex_wham_usage.go#L155) |
| `chatgpt-account-id` | `{account_id}` | [`service/codex_wham_usage.go:156`](../../../new-api-main/service/codex_wham_usage.go#L156) |
| `Accept` | `application/json` | [`service/codex_wham_usage.go:157`](../../../new-api-main/service/codex_wham_usage.go#L157) |
| `originator` | `codex_cli_rs`（未预设时写入） | [`service/codex_wham_usage.go:158-160`](../../../new-api-main/service/codex_wham_usage.go#L158-L160) |
| 单次查询超时 | controller 使用 15 秒 context timeout | [`controller/codex_usage.go:108-115`](../../../new-api-main/controller/codex_usage.go#L108-L115) |

New API 自己暴露给管理前端的路由为 `GET /api/channel/:id/codex/usage`，权限是 `ChannelRead`；controller 再代用户向上述 ChatGPT 上游发请求。[`router/channel-router.go:63-66`](../../../new-api-main/router/channel-router.go#L63-L66)

### 1.2 使用哪些凭证字段

New API 的扁平 OAuth JSON 模型包含：

```json
{
  "id_token": "<optional>",
  "access_token": "<required for usage>",
  "refresh_token": "<optional but needed for automatic refresh>",
  "account_id": "<required for usage>",
  "last_refresh": "<optional>",
  "email": "<optional>",
  "type": "<optional>",
  "expired": "<optional>"
}
```

字段定义来自 [`relay/channel/codex/oauth_key.go:9-29`](../../../new-api-main/relay/channel/codex/oauth_key.go#L9-L29)。用量查询明确拒绝缺少 `access_token` 或 `account_id` 的凭证。[`controller/codex_usage.go:85-100`](../../../new-api-main/controller/codex_usage.go#L85-L100)

重要实现差异：这里描述的是 **New API 渠道 key 的扁平格式**，不能据此断言稍后用户提供路径下的 Codex CLI `auth.json` 一定同构。实现时应单独做 `AuthFileAdapter`：

- 仅读取指定的单一文件路径；
- 把原文件结构映射为内部的 `accessToken`、`refreshToken`、`accountId`；
- 禁止把整个 JSON 传给前端或写入普通日志；
- 若文件中没有直接 `account_id`，可以从 access token 的 JWT claim `https://api.openai.com/auth.chatgpt_account_id` 提取；New API 已采用这一提取方式。[`service/codex_oauth.go:108-134`](../../../new-api-main/service/codex_oauth.go#L108-L134)

### 1.3 Token 过期与刷新

当用量请求返回 `401` 或 `403` 且存在 `refresh_token` 时，New API：

1. 使用 10 秒 timeout 刷新 token；
2. 成功后保存新 access/refresh token；
3. 使用新 access token重试一次原用量查询。

来源：[`controller/codex_usage.go:118-147`](../../../new-api-main/controller/codex_usage.go#L118-L147)。

刷新请求的上游为：

- `POST https://auth.openai.com/oauth/token`
- `Content-Type: application/x-www-form-urlencoded`
- form fields：`grant_type=refresh_token`、`refresh_token`、`client_id`
- 期望响应：`access_token`、`refresh_token`、`expires_in`

来源：[`service/codex_oauth.go:16-20,41-92`](../../../new-api-main/service/codex_oauth.go#L16-L92)。

建议本项目首版保持与 New API 相同的“一次刷新、一次重试”边界；刷新失败后将账号标成 `AUTH_ERROR` 并告警，不无限重试。

## 2. 用量响应结构与重置时间

New API backend 不把上游 JSON反序列化为固定 Go DTO，而是把成功或失败 body 解析为任意 JSON 后透传；因此当前最明确的响应契约来自同仓库管理前端所声明并消费的字段。[`controller/codex_usage.go:150-165`](../../../new-api-main/controller/codex_usage.go#L150-L165) [`codex-usage-dialog.tsx:69-128`](../../../new-api-main/web/default/src/features/channels/components/dialogs/codex-usage-dialog.tsx#L69-L128)

可用字段：

```json
{
  "plan_type": "plus",
  "user_id": "user-…",
  "email": "…",
  "rate_limit": {
    "plan_type": "plus",
    "allowed": true,
    "limit_reached": false,
    "primary_window": {
      "used_percent": 12.3,
      "reset_at": 1785300073,
      "reset_after_seconds": 428297,
      "limit_window_seconds": 604800
    },
    "secondary_window": {
      "used_percent": 0,
      "reset_at": 0,
      "reset_after_seconds": 0,
      "limit_window_seconds": 18000
    }
  },
  "additional_rate_limits": [],
  "rate_limit_reset_credits": {
    "available_count": 0
  },
  "credits": {
    "overage_limit_reached": false
  },
  "spend_control": {
    "reached": false
  }
}
```

上例只表达字段和类型；数值为示意，不是从任何真实凭证读取。

字段语义和解析建议：

- `used_percent`：窗口已使用百分比；剩余百分比可显示为 `max(0, 100 - used_percent)`。
- `reset_at`：Unix timestamp，单位秒；作为该账号窗口的权威重置时刻显示和调度。
- `reset_after_seconds`：上游给出的剩余秒数，可用于交叉校验；不要仅靠客户端本地计算覆盖它。
- `limit_window_seconds`：窗口总长度。New API UI 把 `>= 86400` 秒的窗口识别为 weekly，短于一天识别为 five-hour；若无法按长度识别，才回退为 primary/secondary 顺序。[`codex-usage-dialog.tsx:267-322`](../../../new-api-main/web/default/src/features/channels/components/dialogs/codex-usage-dialog.tsx#L267-L322)
- `allowed && !limit_reached`：New API UI 展示为可用，否则展示受限。[`codex-usage-dialog.tsx:382-397`](../../../new-api-main/web/default/src/features/channels/components/dialogs/codex-usage-dialog.tsx#L382-L397)
- `additional_rate_limits`：附加计费/特性窗口；基础周额度规则不应误用这些窗口。
- `rate_limit_reset_credits.available_count`：可用的“banked reset”数量，不等于普通周额度剩余。

注意：上游提供的是百分比状态，不是逐请求明细。要回答“今天用了多少”，仍需定时保存账号周 `used_percent` 快照，或在所有共享请求经过本服务时同时记录请求前后状态。正确的日使用增量是同一额度 epoch 内 `current_used_percent - previous_used_percent`；发生普通周重置或全局额外重置时要切换 epoch，不能把百分比下降记成负使用量。

## 3. New API 的错误处理边界

| 情况 | New API 行为 | 一手来源 |
|---|---|---|
| nil client、空 base URL/token/account ID | 调用前返回 error | [`service/codex_wham_usage.go:22-36`](../../../new-api-main/service/codex_wham_usage.go#L22-L36) |
| 网络失败/读取 body 失败 | 返回 error；controller 对用户给泛化错误 | [`service/codex_wham_usage.go:44-54`](../../../new-api-main/service/codex_wham_usage.go#L44-L54) [`controller/codex_usage.go:111-115`](../../../new-api-main/controller/codex_usage.go#L111-L115) |
| `401` / `403` 且有 refresh token | 刷新并重试一次 | [`controller/codex_usage.go:118-147`](../../../new-api-main/controller/codex_usage.go#L118-L147) |
| refresh 失败 | 不重试；最后返回原上游状态（实现没有把 refresh error 暴露给用户） | [`controller/codex_usage.go:118-165`](../../../new-api-main/controller/codex_usage.go#L118-L165) |
| 非 2xx | 外层 HTTP 仍为 200，JSON 中 `success=false`、`upstream_status` 为真实状态 | [`controller/codex_usage.go:155-165`](../../../new-api-main/controller/codex_usage.go#L155-L165) |
| body 不是 JSON | 将 body 当字符串透传 | [`controller/codex_usage.go:150-153`](../../../new-api-main/controller/codex_usage.go#L150-L153) |

本项目不必复制“外层永远 HTTP 200”的设计。更适合服务 API 的语义是：

- 上游认证失败：`502` + 内部错误码 `UPSTREAM_AUTH_FAILED`（页面仍显示最后一次成功快照）；
- 上游限流：`503` 或 `429` + `Retry-After`/指数退避；
- timeout/网络失败：保留 last-known-good，不把额度误置为 0 或 100；
- 响应缺少 weekly window：标记 `DATA_INCOMPLETE`，不要猜测；
- 原始上游 body 仅允许脱敏后进入 debug 日志。

## 4. Banked reset 相关接口

New API 还实现了两个上游接口：

| 用途 | Method 与 URL | Body |
|---|---|---|
| 查询 reset credits | `GET https://chatgpt.com/backend-api/wham/rate-limit-reset-credits` | 无 |
| 消耗一个 reset credit | `POST https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume` | `{"redeem_request_id":"<new UUID>"}` |

二者使用与 `wham/usage` 相同的认证 headers；consume 额外使用 `Content-Type: application/json`。[`service/codex_wham_usage.go:57-160`](../../../new-api-main/service/codex_wham_usage.go#L57-L160)

本共享账号额度服务首版应只读展示 `available_count`，不要自动 consume。消耗 reset credit 是有外部副作用的操作，必须单独权限、明确确认、审计记录和幂等 key。

## 5. Codex Runway Reset Watch 可提供什么

该第三方页面跟踪 `@thsottiaux` 的公开 X 动态，以 AI 分类生成结构化事件，并明确
声明与 OpenAI 无关联。公开接口是：

```http
GET https://www.codexrunway.com/api/status.json
Accept: application/json
```

feed 使用 `schemaVersion: 1`，包含生成时间、监控健康状态与事件数组。QuotaTide
只消费：

- `reset_scheduled`：使用 `announcedAt`、`effectiveAt`、`confidence` 与来源链接
  展示尚未到时的预计重置；
- `reset_completed`：展示最近一次重置公告；
- `limit_increase`：不等于重置，忽略。

该 feed 公开且无需凭证。桌面端后端每小时读取一次，不向它发送 Codex Token、
账号 ID、邮箱或 auth.json 内容。接口失败只影响雷达状态，不影响账号额度查询。
详情见 [重置预测接口调研](codex-reset-prediction.md)。
- `announced_at` 是公告时刻。公告文本可能说“未来 30 分钟/1 小时内生效”，因此不能假设额度在该秒已同步完成；最终仍以账号 `wham/usage.used_percent/reset_at` 的变化为准。

## 6. 推荐的数据融合与状态机

### 6.1 定时采集

- `wham/usage`：建议每 2–5 分钟一次；共享账号发生请求后可触发一次带防抖的刷新。
- Codex Resets：建议每 1–5 分钟一次，全系统共享一份缓存。
- 所有时间以 UTC 保存，页面按 `Asia/Shanghai` 显示。

### 6.2 周额度 epoch

为每个账号维护：

- `weekly_used_percent`
- `weekly_reset_at`
- `weekly_window_seconds`
- `last_success_at`
- `epoch_id`
- `last_global_reset_event_id`

切换 epoch 的条件：

1. `reset_at` 发生变化；或
2. `used_percent` 明显下降，且不是数据乱序；或
3. 到达旧 `reset_at` 后首次拿到新窗口。

若降幅附近存在新的 Codex Resets event，可把 reset reason 标成 `GLOBAL_ANNOUNCED_RESET`；否则标成 `SCHEDULED_OR_UPSTREAM_RESET`。雷达只负责解释原因，不负责决定是否已重置。

### 6.3 与每日上限结合

已确认的规范为工作日 16%、非工作日 10%，80% 日阈值预警、100% 日阈值停用。实现时：

- 每日使用量单位采用“周额度百分点”；
- 每日基线取本地日界线后第一份有效 weekly `used_percent`；
- 当日增量累计只在相同 epoch 内相减；
- epoch 中途因全局 reset 归零时，重置前已使用量不能丢失：`daily_used = reset 前累计 + 新 epoch 增量`；
- 上游状态过期时禁止把 last-known-good 当作实时事实；页面展示 stale，执行层按配置进入 fail-open 或 fail-closed。共享账号额度保护建议超过一个采集周期后预警，连续失败达到明确时长后 fail-closed。

## 7. 安全与合规边界

- `auth.json` 只在 server process 中读取；页面、前端 bundle、浏览器 local storage 都不能出现 token。
- 配置中保存文件路径，不复制凭证正文到数据库。
- 启动时校验目标是普通文件、拒绝目录/符号链接越界，并校验文件权限；只允许明确配置的路径。
- API 响应只返回 `user_id/email/account_id` 的脱敏展示值；真实 access/refresh/id token 永不返回。
- 日志不得记录 Authorization header、完整 OAuth JSON、JWT 或上游原始错误 body。
- 自动刷新 token 会改写凭证；在用户提供实际路径与文件结构前，不应实现原地写回。首版可以只读并在认证过期时告警，或经明确确认后采用原子写入与备份。
- `wham` 是 ChatGPT web backend 路径，New API 源码证明其当前用法，但本调研未找到 OpenAI 公开稳定性承诺；应封装在可替换 adapter 中，并为字段缺失/接口变化做契约测试。

## 8. 对实现的直接建议

建议模块边界：

```text
AuthFileAdapter
  -> CodexWhamClient
       -> UsageNormalizer
            -> QuotaEpochTracker
                 -> DailyPolicyEngine
                      -> AlertService

CodexResetsClient
  -> GlobalResetEventStore
       -> QuotaEpochTracker（仅提供 reset reason 佐证）
```

不要让 `DailyPolicyEngine` 解析 auth 文件、调用第三方页面或理解上游原始 JSON。这样以后 `auth.json` 结构或 `wham` 字段变化只影响 adapter/client 层。

## 9. 待拿到 `auth.json` 路径后验证

不读取秘密值，只验证结构与权限：

1. 顶层字段是扁平 OAuth key，还是 `tokens`/其他嵌套结构；
2. `account_id` 是否直接存在；
3. access token JWT 是否含可提取的 `chatgpt_account_id`；
4. token 是否需要自动刷新，是否允许安全原子写回；
5. 一个文件是否只代表一个账号；
6. 文件替换/轮换时如何检测 inode/mtime 变化。

完成这些验证后再固定 `AuthFileAdapter` 契约。
