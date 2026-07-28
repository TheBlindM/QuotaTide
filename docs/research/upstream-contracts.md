# Codex 账号额度与重置雷达数据契约

日期：2026-07-28

## 结论

Rust 桌面端应把两个上游实现为两个彼此独立的防腐层：

1. `CodexUsageSource` 每次采集都只读并重新打开用户选择的 `auth.json`，然后调用 Codex 当前使用的额度接口，产出当前账号的七日窗口快照。
2. `ResetRadarSource` 匿名读取 Codex Resets 的 `watch`，只产出第三方预测。

两者不能相互替代。雷达预测、公开重置消息或雷达历史事件都不能证明当前账号已经重置；只有当前账号连续额度快照形成的新 quota epoch 才能确认重置。

`auth.json` 的所有写操作、OAuth refresh、重置额度消费和代理 Codex 请求都不属于本应用。Codex 软件负责刷新 token，本应用只在下一次采集时重新读取。

## 证据等级

### OpenAI 官方、公开可复查

- OpenAI Codex 的 app-server 文档公开了 `account/rateLimits/read`，并把窗口定义为 `usedPercent`、`windowDurationMins` 和 Unix 秒 `resetsAt`。它证明 Codex 自身的稳定领域模型是“已用百分比 + 窗口长度 + 重置时间”，而不是“最近七天”聚合：[app-server README](https://github.com/openai/codex/blob/3418498f01422f5f650ea645d4bd19e05c3a9616/codex-rs/app-server/README.md#L2018-L2055)。
- Codex 官方 Rust 模型中的 WHAM 窗口字段是 `used_percent`、`limit_window_seconds`、`reset_after_seconds`、`reset_at`：[RateLimitWindowSnapshot](https://github.com/openai/codex/blob/3418498f01422f5f650ea645d4bd19e05c3a9616/codex-rs/codex-backend-openapi-models/src/models/rate_limit_window_snapshot.rs#L15-L47)。
- Codex 官方文件存储实现通过 `File::open`、`read_to_string` 和 Serde 读取 `auth.json`；这支持“每次重新打开并解析”的实现方式：[storage.rs](https://github.com/openai/codex/blob/3418498f01422f5f650ea645d4bd19e05c3a9616/codex-rs/login/src/auth/storage.rs#L1618-L1654)。
- Codex 官方登录代码写入的结构包含 `auth_mode`、`tokens`、`last_refresh`，其中 token 数据包含 access token、refresh token、ID token和可选 account ID：[server.rs](https://github.com/openai/codex/blob/3418498f01422f5f650ea645d4bd19e05c3a9616/codex-rs/login/src/server.rs)。

### OpenAI 实现细节，但不是公开平台 API

- `GET https://chatgpt.com/backend-api/wham/usage` 是 Codex 当前实现使用的内部接口，不是 OpenAI Platform 的公开承诺 API。它可以作为 v1 的适配器上游，但字段、路径、鉴权头和可用性都可能改变。
- 官方代码与 app-server 对 rate-limit 数据做了规范化，证明这些数据确实由 Codex 消费；这不等于 OpenAI 对第三方直接调用 WHAM 提供兼容性保证。
- 因此 WHAM 解析必须严格、隔离、可替换，且失败时不得用猜测值覆盖最后成功快照。

### 非官方消费方证据

- 本地 New API 的实现使用 Bearer token、`chatgpt-account-id`、`Accept: application/json` 和 `originator: codex_cli_rs` 调用 WHAM：[codex_wham_usage.go](../../../new-api-main/service/codex_wham_usage.go)。
- New API 的 15 秒超时与错误透传可作为工程参考：[codex_usage.go](../../../new-api-main/controller/codex_usage.go)。
- New API 会在 401/403 后自行 refresh 并写回自己的渠道凭据。这一行为明确**不复制**到本桌面端；本产品已决定 `auth.json` 永远只读。
- 当前 Node 原型是已有行为参考，不是上游权威：[auth-file.js](../../src/auth-file.js)、[codex-client.js](../../src/codex-client.js)、[resets-client.js](../../src/resets-client.js)。

### 第三方来源

- [Codex Resets](https://codex-resets.com/) 自己声明其数据来自公开推文、由自动分类器处理，并且不隶属于 OpenAI。
- 其 `/api/resets` 是网站自己的未版本化接口。当前响应包含 `generated_at`、`watch`、`events` 和 `stats`；没有稳定性或 SLA 保证。
- `watch.reset_chance_24h` 是第三方 AI 估算结果，不是 OpenAI 对重置的承诺，也不是当前账号的状态。

## `auth.json` 只读契约

### 输入

设置保存一个用户选择的文件路径。每次启动采集、每小时采集和手动刷新都重新打开该路径，不缓存文件描述符。

v1 接受 Codex 当前的嵌套结构：

```json
{
  "auth_mode": "chatgpt",
  "last_refresh": "RFC3339 timestamp",
  "tokens": {
    "access_token": "secret",
    "account_id": "account id",
    "id_token": "JWT",
    "refresh_token": "secret"
  }
}
```

只有以下字段进入适配器：

```rust
struct AuthMaterial {
    access_token: SecretString,
    account_id: AccountId,
    token_fingerprint: TokenFingerprint,
}
```

- `tokens.access_token` 必须是非空字符串。
- `tokens.account_id` 是首选账号标识。
- 仅当 `tokens.account_id` 缺失时，才允许从 `tokens.id_token` 顶层
  `https://api.openai.com/auth` 对象内的 `chatgpt_account_id` claim 读取账号标识。
- JWT fallback 只用于取账号路由提示，不等于签名校验；不得把未验证 claim 当作授权证明。
- `refresh_token`、完整 ID token、access token、邮箱和原始 JSON 不进入 UI、日志、数据库或诊断包。
- 旧工具使用的顶层扁平 token 结构不属于 v1 的正式契约。以后若兼容，必须作为单独的显式 adapter 版本。

### 文件读取

- 只允许读取，不以写模式打开，不改权限，不 rename，不删除，不 refresh。
- 对不存在、无权限、不是普通文件、超过 1 MiB、非 UTF-8、无效 JSON、字段缺失分别返回可区分错误。
- 为容忍 Codex 恰好在替换文件，`NotFound` 或 JSON 截断可在约 250 ms 后重新打开并重试一次；仍失败则结束本轮，不循环等待。
- 文件路径和账号 ID 在诊断中只保留脱敏形式。内部 `account_key` 使用本机随机盐后的哈希，不能由数据库直接恢复账号 ID。
- 成功读取后立即缩短 secret 生命周期；HTTP 请求完成后不保留 token。

### token 更新与 401/403

普通采集开始时始终读一次文件。WHAM 返回 401/403 时：

1. 再次重新打开 `auth.json`；
2. 比较只存在内存中的 token/account 指纹；
3. 只有凭据确实变化时才重试一次请求；
4. 凭据未变化或重试仍失败时，返回 `AuthenticationStale`。

本应用不得调用 OAuth token endpoint，也不得写回任何 token。用户只需等待 Codex 自动刷新，或在 Codex 中重新登录。

### 当前账号变化

文件中的 `account_id` 变化表示“当前账号已切换”，不是额度重置：

- 新账号建立独立的 account stream；
- 不把新账号第一笔用量加到旧账号当天；
- 不把两个账号的历史连成一个七日窗口；
- UI 只展示新账号，旧 stream 是否保留由后续状态/隐私设计决定；
- 账号切换本身不发送“额度已重置”通知。

## Codex 七日窗口契约

### 请求

```text
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer <access token>
chatgpt-account-id: <account id>
Accept: application/json
originator: codex_cli_rs
```

网络边界：

- 固定 HTTPS origin，不接受把 Bearer header 重定向到其他 host；
- 总超时 15 秒；
- 响应体上限 1 MiB；
- 只接受 2xx JSON；
- 不在日志中记录请求 header、响应原文或 secret；
- 除“401/403 且文件凭据已变化”外，本轮不立即重试。

### 最小响应

```json
{
  "plan_type": "plus",
  "rate_limit": {
    "allowed": true,
    "primary_window": {
      "used_percent": 12,
      "limit_window_seconds": 18000,
      "reset_after_seconds": 1200,
      "reset_at": 1780000000
    },
    "secondary_window": {
      "used_percent": 41,
      "limit_window_seconds": 604800,
      "reset_after_seconds": 300000,
      "reset_at": 1780300000
    }
  }
}
```

v1 只关心当前账号的基准七日窗口：

- 在 `primary_window` 和 `secondary_window` 中选择
  `limit_window_seconds == 604800` 的窗口；通常它是 `secondary_window`。
- 不使用 `>= 86400` 这类宽泛判断，以免未来把日窗口或其他长期额度误标为七日窗口。
- 未找到恰好 604800 秒的窗口时，返回 `WeeklyWindowUnavailable`，保留最后成功快照并显示上游结构不受支持；不猜测。
- `used_percent` 必须是有限数值且在 `0..=100`。
- `reset_at` 必须是有效 Unix 秒，`limit_window_seconds` 必须为正。
- `reset_after_seconds` 只作诊断交叉检查；UI 和 epoch 以绝对 `reset_at` 为准，避免本地处理耗时造成漂移。
- `allowed`、`plan_type` 可以缺失或新增枚举值；它们不能阻止合法周窗口被采集。
- 未识别字段全部忽略，以容忍向前扩展。

规范化输出：

```rust
struct WeeklyUsageSnapshot {
    account_key: AccountKey,
    captured_at: DateTime<Utc>,
    used_percent: f64,
    window_seconds: u32, // v1: 604800
    resets_at: DateTime<Utc>,
    plan_type: Option<String>,
    allowed: Option<bool>,
}
```

这里的七天是上游 quota window `[window_start, resets_at)`，不是数据库中的“最近七个自然日”。`window_start` 由 `resets_at - 604800s` 得到，再按用户时区切分展示日。

## quota epoch 与每日增量

持久状态需要同时保存原始合法快照、该 epoch 的高水位和账户 stream。不能只依赖一条 `reset_at` 等值判断。

### 第一笔样本

- 为当前账号创建 epoch；
- 以当前 `used_percent` 建立 baseline 和 high-water；
- 本轮每日增量为 0，因为应用不知道安装/启动前的用量发生在哪一天；
- UI 仍展示上游的当前周使用百分比。

### 同一 epoch

满足以下条件时视为同一 epoch：

- account stream 相同；
- 窗口仍为 604800 秒；
- 没有出现超过 `0.01` 个百分点的用量下降；
- 旧的计划重置边界尚未被可靠跨越。

增量为 `max(0, current_used - high_water_used)`，随后更新 high-water。小于等于 `0.01` 的下降视为舍入噪声，不降低 high-water，避免后续回升被重复计量。

如果 `reset_at` 在旧边界到来前发生变化，但用量没有下降，只更新计划重置时间；这是 schedule correction，不创建 epoch，也不通知重置。

### 确认新 epoch

下列任一强证据创建新 epoch：

1. 同一账号、同一窗口的 `used_percent` 比 high-water 下降超过 `0.01`；
2. 已跨过旧 `resets_at`，且新快照的 `resets_at` 明显推进到下一窗口。

新 epoch 的首笔每日增量为当前 `used_percent`，因为它代表重置后到本次采集之间已经发生的用量。该转换才可发送“当前账号额度已重置”通知。

账号变化、窗口结构变化、适配器升级或本地状态丢失只建立新的 baseline，不算已确认重置。

比最后接受快照更早或相同 `captured_at` 的响应被丢弃，防止慢请求回写旧状态。

## Codex Resets `watch` 契约

请求：

```text
GET https://codex-resets.com/api/resets
Accept: application/json
```

- 不携带 Codex token、account ID、cookie、邮件或其他用户数据。
- 总超时 10 秒，响应体上限 1 MiB。
- 雷达失败不影响账号额度采集，账号额度失败也不阻止雷达更新。

当前 `watch` 形状：

```json
{
  "watch": {
    "level": "string",
    "tweet_id": "string",
    "tweet_url": "https://x.com/...",
    "text": "string",
    "observed_at": "RFC3339 timestamp",
    "expires_at": "RFC3339 timestamp",
    "window_hours": 24,
    "reset_chance_24h": 75,
    "context_tweet_id": null,
    "context_tweet_url": null,
    "context_text": null
  }
}
```

活动预测必须同时满足：

- `watch` 是对象；
- `tweet_id`、`observed_at`、`expires_at` 存在且可解析；
- `observed_at < expires_at` 且当前时间早于 `expires_at`；
- `window_hours` 是有限正数；
- `reset_chance_24h` 是有限数值且在 `0..=100`。

`watch: null`、字段缺失、过期或格式错误都表示没有可用的活动预测。最后成功的雷达快照可以保留作诊断，但一旦 `expires_at` 到达就不得继续显示为活动预测。

`generated_at` 是响应生成时间，不是预测观察时间；预测时序使用 `observed_at` 和 `expires_at`。UI 可保留原始 chance，并按网站风格派生十位档标签，例如 `75 -> >70%`；提醒判断使用原始值 `chance >= 70`。

预测提醒去重键：

```text
reset-radar:<tweet_id>:<observed_at>:70
```

同一个 watch 不重复发送。新 `tweet_id` 或新 `observed_at` 才是新预测事件。雷达中的 `events` 是公开历史，不是当前账号的采集事实；v1 不用它创建 quota epoch。

## 调度、并发与刷新

统一 `RefreshCoordinator`：

- 应用启动后执行一次；
- 正常周期为一小时；
- 手动刷新冷却 30 秒；
- 定时、手动、唤醒恢复和通知触发共用一个 single-flight；
- 已有刷新运行时，新的触发复用同一结果，不再启动第二组请求；
- 电脑睡眠后不补跑每个错过的小时；唤醒时若已过期，只立即运行一次，然后重建一小时节奏；
- 一个周期内 Codex 与雷达可以并行，但分别提交结果；
- 所有持久化都在验证和 epoch 判定完成后以单事务提交。

手动刷新不能绕过文件只读、超时、响应上限或 secret 处理规则。

## 错误、最后成功值与过期

每个 source 独立维护：

```rust
enum SourceFreshness {
    Fresh,
    Stale,
    Unavailable,
}

struct SourceHealth {
    last_attempt_at: DateTime<Utc>,
    last_success_at: Option<DateTime<Utc>>,
    consecutive_failures: u32,
    freshness: SourceFreshness,
    public_error: Option<PublicErrorCode>,
}
```

- 本轮成功且距 `last_success_at` 不超过 90 分钟为 `Fresh`。
- 有最后成功值，但最新一轮失败或数据年龄超过 90 分钟为 `Stale`。
- 从未成功为 `Unavailable`。
- 失败不删除最后成功值，不写入 0%，也不创建每日增量。
- 连续失败三次发一次故障提醒；恢复成功后清零，下一组连续三次可再次提醒。
- UI 始终同时展示“最后成功时间”和当前错误，不能把旧数据冒充实时数据。

建议的公开错误码：

| 分类 | 例子 | 是否保留最后成功值 |
| --- | --- | --- |
| `AuthPath` | 不存在、无权限、不是文件 | 是 |
| `AuthFormat` | JSON 截断、字段缺失、auth mode 不支持 | 是 |
| `AuthenticationStale` | 401/403 且文件凭据未变化 | 是 |
| `RateLimited` | 429 | 是 |
| `UpstreamUnavailable` | 超时、DNS、5xx | 是 |
| `UpstreamSchema` | 无周窗口、数值越界 | 是 |
| `RadarUnavailable` | 雷达超时或非 2xx | 是，仅影响雷达 |
| `RadarSchema` | watch 格式错误 | 是，仅影响雷达 |

日志只记录错误码、HTTP status、耗时、source、尝试时间和脱敏指纹。不得记录响应正文，因为上游错误正文也可能回显敏感信息。

## Rust adapter seam

```rust
#[async_trait]
trait AuthMaterialSource {
    async fn read_current(&self) -> Result<AuthMaterial, AuthReadError>;
}

#[async_trait]
trait CodexUsageSource {
    async fn fetch_weekly(
        &self,
        auth: &AuthMaterial,
    ) -> Result<WeeklyUsageSnapshot, CodexUsageError>;
}

#[async_trait]
trait ResetRadarSource {
    async fn fetch_watch(&self) -> Result<RadarSnapshot, RadarError>;
}
```

领域层只接收规范化 DTO，不知道文件 JSON、HTTP header、WHAM 原始字段或雷达原始 JSON。这样当 WHAM 变化、未来改用 app-server，或雷达接口变化时，不需要改每日策略、通知和 UI。

## 实施前必须固定的契约测试

1. 官方嵌套 `auth.json` 可读取；secret 不出现在 Debug/Serialize 输出。
2. 文件不存在、权限拒绝、超限、截断 JSON 和字段缺失均返回稳定错误码。
3. 读取期间文件被 Codex 替换时只短暂重试一次，绝不写文件。
4. 401 后文件 token 未变不重试；token 已变只重试一次。
5. 5 小时 primary 与 604800 秒 secondary 同时存在时只选择 secondary。
6. 没有 604800 秒窗口时不把其他窗口猜成周窗口。
7. 第一笔快照只建 baseline，不伪造今天用量。
8. 单调上升只记录 high-water 以上的增量；舍入抖动不重复计量。
9. 重置时间提前变化但用量未下降时只修正 schedule。
10. 用量明显下降或跨越旧边界后新 reset 时间推进时创建 epoch。
11. 账号切换隔离 stream，不发送重置提醒。
12. 过期、空或格式错误的 radar watch 不显示活动预测。
13. 雷达 chance 达到 70 的同一 watch 只提醒一次。
14. 雷达预测不能创建 quota epoch；只有账号快照能确认重置。
15. 定时和手动刷新重叠时只有一个 in-flight；手动冷却为 30 秒。
16. 任一 source 失败保留自己的最后成功值，另一 source 仍可成功。

## 对现有原型的修正

- 保留：一小时轮询、15 秒 Codex 超时、只读 auth、单账号、按当前上游窗口展示七天。
- 收紧：周窗口从“任意至少一天”改为恰好 604800 秒；未知结构显式报错。
- 修正：JWT account fallback 必须读取 `https://api.openai.com/auth` 对象内的 `chatgpt_account_id`，不是带点号的扁平 key。
- 修正：`reset_at` 在边界前跳动但用量未下降时不应创建新 epoch。
- 新增：401/403 只在 `auth.json` 凭据已变化时重试一次。
- 新增：账号切换隔离、out-of-order 丢弃、high-water 抗抖动、90 分钟 freshness。
- 明确拒绝：复制 New API 的 refresh/write 行为、使用雷达消息确认账号重置、把“最近七天”代替当前 quota window。
