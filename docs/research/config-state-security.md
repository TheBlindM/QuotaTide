# 配置、状态与本地安全模型

> 状态：v1 实施基线
>
> 日期：2026-07-28
>
> 关联：[应用架构](./application-architecture.md) ·
> [平台集成](./platform-integrations.md) ·
> [上游契约](./upstream-contracts.md)

## 结论

v1 使用一个由 Rust 独占的版本化 SQLite 数据库保存非秘密配置、不可变事实、
当前状态和可重建投影。应用不提供需要手工编辑的 `config.json`，WebView
不能直接访问数据库、文件系统或系统凭证库。

唯一需要持久化的应用秘密是 SMTP 密码或应用专用密码。它使用 macOS
Keychain 或 Windows Credential Manager 保存，SQLite 只保存不含密码片段的
opaque credential reference。Codex token 只在每轮刷新期间从用户选择的
`auth.json` 只读载入内存，永不进入本应用的任何持久化介质。

本地数据的边界是：

- 当前账号界面只展示当前 `auth.json` 对应的账号流；
- 切换账号后，旧账号流隐藏保留且永不与新账号合并；
- 额度观测、策略版本、提醒和投递事实一直保留，直到用户明确执行
  “清除全部本地数据”；
- 运行日志使用 `5 × 1 MiB` 滚动上限；
- v1 不引入 SQLCipher，不建立应用主密码；静态数据保护依赖当前用户文件
  权限和设备的 FileVault/BitLocker；
- schema 迁移前生成校验过的滚动备份，损坏时绝不静默清空。

## 数据目录与文件权限

生产路径由 Tauri 的应用数据目录 API 解析，不能由 WebView 或环境变量覆盖：

```text
macOS:
  ~/Library/Application Support/<bundle-id>/

Windows:
  %LOCALAPPDATA%\<publisher>\<app-name>\
```

目录形状固定为：

```text
<app-data>/
├── state.sqlite3
├── state.sqlite3-wal
├── state.sqlite3-shm
├── backups/
│   └── state-v<schema>-<utc>.sqlite3
├── logs/
│   ├── app.log
│   └── app.log.<1..4>
├── recovery/
│   └── state-corrupt-<utc>.sqlite3
└── export-tmp/
```

权限规则：

- macOS 创建应用目录时使用 `0700`，数据库、备份、日志和临时导出文件使用
  `0600`；启动时发现 group/other 权限时收紧，不跟随由其他用户拥有的
  symlink；
- Windows 使用当前用户的 LocalAppData，并为新目录/文件设置只允许当前
  user SID、`SYSTEM` 和 Administrators 的受保护 DACL；不继承可能授予
  `Everyone`/`Users` 写权限的宽泛 ACL；
- 权限无法建立或无法收紧时进入恢复界面，不在不安全位置继续运行；
- 用户选择的 `auth.json` 可以位于其他目录，但本应用只验证并保存规范化
  路径，绝不复制文件内容或修改原文件权限。

SQLite 启用：

```text
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA busy_timeout = 5000;
PRAGMA trusted_schema = OFF;
```

应用只维护一个 `tokio-rusqlite` 连接线程。备份、迁移和恢复期间先停止刷新与
投递 worker，避免另一个连接或任务写入。

## 数值、时间与枚举编码

- 额度使用 `QuotaUnits(i64)`：百万分之一个百分点，
  `100% = 100_000_000`。
- 瞬时时间使用 UTC Unix milliseconds `INTEGER`。
- 自然日使用 `YYYY-MM-DD` `TEXT`，必须与同行保存的 IANA 策略时区一起解释。
- 星期使用 ISO `1..=7`，`1` 为周一。
- 布尔值使用带 `CHECK(value IN (0, 1))` 的 `INTEGER`。
- 领域枚举以稳定的小写 `TEXT` 保存；读取未知值时返回 schema/data
  incompatibility，不把未知值猜成默认项。
- 主键使用 SQLite `INTEGER PRIMARY KEY`；跨表或去重所需的公开 identity
  使用应用生成的 UUIDv7 `TEXT`。

所有来自上游的浮点比例只在 adapter 边界转换一次。数据库不保存 `NaN`、
infinity 或未经范围检查的比例。

## schema 版本与迁移

`PRAGMA user_version` 是 schema 的唯一机器可读版本。`schema_migrations`
记录审计信息，但不作为决定下一步 migration 的第二来源：

```text
schema_migrations
  version             INTEGER PRIMARY KEY
  applied_at_ms       INTEGER NOT NULL
  app_version         TEXT NOT NULL
  checksum            TEXT NOT NULL
```

规则：

1. migration 以编译进 binary 的顺序 SQL/Rust steps 提供，checksum 固定；
2. 新数据库从空文件按顺序建立，不导入 Node 原型数据库；
3. 已有数据库版本低于 binary 支持版本时，先执行 `quick_check`，再用 SQLite
   online backup API 生成备份并对备份执行 `integrity_check`；
4. 每一步在单独 `BEGIN IMMEDIATE` transaction 中完成，成功后才更新
   `user_version` 与 `schema_migrations`；
5. migration 失败回滚该步，并触发恢复流程；
6. 数据库版本高于当前 binary 时不降级、不写入，显示“需要更新应用”；
7. migration 只能向前。二进制 roll-forward 时必须包含从所有仍受支持 schema
   版本到当前版本的路径。

保留最近三份通过 `integrity_check` 的迁移前备份。新备份验证成功后才删除
第四份；不能因为一次失败备份而删除最后一个有效恢复点。

## 配置 schema

### 应用元数据

```text
app_meta
  singleton_id              INTEGER PRIMARY KEY CHECK (singleton_id = 1)
  app_instance_id           TEXT NOT NULL UNIQUE
  local_hash_salt           BLOB NOT NULL CHECK (length(local_hash_salt) = 32)
  settings_revision         INTEGER NOT NULL CHECK (settings_revision >= 0)
  dashboard_revision        INTEGER NOT NULL CHECK (dashboard_revision >= 0)
  created_at_ms             INTEGER NOT NULL
  updated_at_ms             INTEGER NOT NULL
```

`app_instance_id` 与 `local_hash_salt` 在首次启动生成。盐不是认证秘密，但不
进入诊断导出；它用于产生不可逆的本机 `account_key`、recipient key 和日志
指纹，防止数据库直接暴露 Account ID 或邮箱。

### 当前设置

```text
app_settings
  singleton_id              INTEGER PRIMARY KEY CHECK (singleton_id = 1)
  auth_path                 TEXT
  policy_timezone           TEXT NOT NULL
  autostart_enabled         INTEGER NOT NULL CHECK (autostart_enabled IN (0, 1))
  active_policy_revision_id INTEGER NOT NULL REFERENCES policy_revisions(id)
  active_account_stream_id  INTEGER REFERENCES account_streams(id)
  created_at_ms             INTEGER NOT NULL
  updated_at_ms             INTEGER NOT NULL
```

- 首次运行从系统读取 IANA 时区，识别失败回退 `Asia/Shanghai`。
- `auth_path` 只有在文件选择器返回路径，Rust 成功规范化、只读打开并验证
  Codex auth 结构后才能替换。
- 当前账号由下一次成功 auth/usage refresh 设置；手工路径变化本身不伪造
  账号切换。
- 所有设置 DTO 带 `expected_settings_revision`。保存时不匹配返回
  `SettingsConflict`，UI 重新载入后让用户再次提交，避免两个窗口或异步操作
  覆盖更新。

### 七日策略版本

```text
policy_revisions
  id                        INTEGER PRIMARY KEY
  revision_key              TEXT NOT NULL UNIQUE
  effective_at_ms           INTEGER NOT NULL
  policy_timezone           TEXT NOT NULL
  carry_workdays_enabled    INTEGER NOT NULL CHECK (carry_workdays_enabled IN (0, 1))
  created_at_ms             INTEGER NOT NULL

policy_day_limits
  policy_revision_id        INTEGER NOT NULL REFERENCES policy_revisions(id)
  iso_weekday               INTEGER NOT NULL CHECK (iso_weekday BETWEEN 1 AND 7)
  base_units                INTEGER NOT NULL CHECK (base_units BETWEEN 0 AND 100000000)
  PRIMARY KEY (policy_revision_id, iso_weekday)
```

每个 revision 必须恰有七行，`SUM(base_units) <= 100_000_000`。默认 revision：

```text
Mon 16% · Tue 16% · Wed 16% · Thu 16% · Fri 16% · Sat 10% · Sun 10%
carry_workdays_enabled = true
```

修改额度、结转开关或策略时区会追加 revision，绝不覆盖旧 revision：

- 新 revision 从提交时刻生效；
- 提交当天尚未结束的投影可以按新策略重算；
- 已结束日期保留其 `daily_ledgers.policy_revision_id` 和上限快照；
- 工作日结转只在周一至周五间流动，不进入周末、不跨自然周；
- 任意 revision 的七日基础总量都不得超过 100%。

### SMTP 与收件人

```text
smtp_settings
  singleton_id              INTEGER PRIMARY KEY CHECK (singleton_id = 1)
  enabled                   INTEGER NOT NULL CHECK (enabled IN (0, 1))
  host                      TEXT
  port                      INTEGER CHECK (port BETWEEN 1 AND 65535)
  tls_mode                  TEXT CHECK (tls_mode IN ('tls', 'starttls'))
  username                  TEXT
  from_address              TEXT
  from_name                 TEXT
  credential_ref            TEXT
  updated_at_ms             INTEGER NOT NULL

smtp_recipients
  id                        INTEGER PRIMARY KEY
  address                   TEXT
  normalized_address        TEXT
  recipient_key             BLOB NOT NULL UNIQUE
  position                  INTEGER NOT NULL CHECK (position >= 0)
  active                    INTEGER NOT NULL CHECK (active IN (0, 1))
  created_at_ms             INTEGER NOT NULL
  retired_at_ms             INTEGER
  CHECK (active = 0 OR (address IS NOT NULL AND normalized_address IS NOT NULL))
```

- 只允许 TLS relay 或 STARTTLS，不提供 plaintext 模式。
- `credential_ref` 只能是 `slot-a` 或 `slot-b`，不包含 host、username、邮箱
  或密码片段。
- 密码状态由实时 vault lookup 派生为 `configured/missing/unavailable`，不把
  “可恢复掩码”或错误原文写入 SQLite。
- 多个收件人分别创建 delivery，使一个地址失败不影响其他地址。投递事实只
  保存 `recipient_key`；地址只从仍 active 的本地 recipient row 解析。
- 删除收件人后，尚未发送的对应 delivery 标记 `cancelled_by_config`，不会
  向已移除地址补发；完成 attempt 不保存地址正文。
- `normalized_address` 使用 `WHERE active = 1` 的 partial unique index。
  退休且不再被 pending delivery 使用的地址可以覆写为 `NULL`，仅保留
  `recipient_key` 与投递事实；当前设置始终只返回 active rows。

保存邮件设置不要求测试邮件成功。`send_test_email` 是独立显式操作，使用
当前已提交设置并返回脱敏结果，不写额度告警 outbox。

### 提醒偏好

```text
alert_preferences
  event_kind                TEXT NOT NULL
  channel                   TEXT NOT NULL CHECK (channel IN ('system', 'email'))
  enabled                   INTEGER NOT NULL CHECK (enabled IN (0, 1))
  updated_at_ms             INTEGER NOT NULL
  PRIMARY KEY (event_kind, channel)
```

固定 `event_kind`：

```text
daily_80
daily_100
weekly_remaining_20
weekly_remaining_10
radar_chance_70
quota_reset_confirmed
source_failures_3
```

首次运行时 system 渠道全部开启，email 渠道全部关闭。完成 SMTP 配置时 UI
明确让用户选择要开启的 email 事件，不能因为保存密码自动打开全部邮件。
阈值是 v1 领域规则，不进入可编辑设置。

## 事实与当前状态 schema

### 账号流和 quota epoch

```text
account_streams
  id                        INTEGER PRIMARY KEY
  stream_key                TEXT NOT NULL UNIQUE
  account_key               BLOB NOT NULL UNIQUE
  first_seen_at_ms          INTEGER NOT NULL
  last_seen_at_ms           INTEGER NOT NULL

quota_epochs
  id                        INTEGER PRIMARY KEY
  epoch_key                 TEXT NOT NULL UNIQUE
  account_stream_id         INTEGER NOT NULL REFERENCES account_streams(id)
  window_seconds            INTEGER NOT NULL CHECK (window_seconds = 604800)
  baseline_units            INTEGER NOT NULL
  high_water_units          INTEGER NOT NULL
  first_observed_at_ms      INTEGER NOT NULL
  latest_observed_at_ms     INTEGER NOT NULL
  scheduled_reset_at_ms     INTEGER NOT NULL
  closed_at_ms              INTEGER
```

`account_key = SHA-256(local_hash_salt || canonical_account_id)`。原始 Account
ID、邮箱、JWT 和 token 不保存。`stream_key`/`epoch_key` 是本机 UUID，与
上游标识无可逆关系。

同一时刻每个账号流只能有一个未关闭 epoch；以 partial unique index 保证：

```text
UNIQUE(account_stream_id) WHERE closed_at_ms IS NULL
```

旧账号流持续保留但不出现在当前 dashboard。再次切回同一账号时恢复它的
stream；只有符合上游契约的新窗口证据才能开启新 epoch。

### 不可变观测

```text
usage_observations
  id                        INTEGER PRIMARY KEY
  observation_key           TEXT NOT NULL UNIQUE
  account_stream_id         INTEGER NOT NULL REFERENCES account_streams(id)
  quota_epoch_id            INTEGER NOT NULL REFERENCES quota_epochs(id)
  captured_at_ms            INTEGER NOT NULL
  used_units                INTEGER NOT NULL CHECK (used_units BETWEEN 0 AND 100000000)
  reset_at_ms               INTEGER NOT NULL
  plan_type                 TEXT
  allowed                   INTEGER CHECK (allowed IN (0, 1))
  UNIQUE(account_stream_id, captured_at_ms)

radar_observations
  id                        INTEGER PRIMARY KEY
  observation_key           TEXT NOT NULL UNIQUE
  captured_at_ms            INTEGER NOT NULL UNIQUE
  watch_key                 TEXT
  observed_at_ms            INTEGER
  expires_at_ms             INTEGER
  chance_units              INTEGER CHECK (chance_units BETWEEN 0 AND 100000000)
  source_url                TEXT
  validity                  TEXT NOT NULL
```

`usage_observations` 只接收经过严格 604800 秒窗口验证的结果。
`radar_observations` 不含 Codex identity，也不能引用或创建 quota epoch。
上游原始 JSON、HTTP headers、错误 body 和完整推文正文均不保存。

### 每日账本

```text
daily_ledgers
  id                        INTEGER PRIMARY KEY
  account_stream_id         INTEGER NOT NULL REFERENCES account_streams(id)
  local_date                TEXT NOT NULL
  policy_timezone           TEXT NOT NULL
  policy_revision_id        INTEGER NOT NULL REFERENCES policy_revisions(id)
  base_units                INTEGER NOT NULL
  carry_units               INTEGER NOT NULL CHECK (carry_units >= 0)
  used_units                INTEGER NOT NULL CHECK (used_units >= 0)
  status                    TEXT NOT NULL
  finalized_at_ms           INTEGER
  updated_at_ms             INTEGER NOT NULL
  UNIQUE(account_stream_id, local_date, policy_timezone)
```

`daily_ledgers` 是可由观测与策略 timeline 重建的投影，但结束日期的
`policy_revision_id/base_units/carry_units` 是历史快照，常规重建不得用当前
策略替换。只有显式的 schema/bug repair migration 可重建，并且必须有 fixture
证明修复不修改不相关历史。

没有足够观测的日期保留 `unknown`，不能按 0% 生成结转。状态枚举固定为
`unknown/normal/warning/exceeded/finalized`。

### 不可变性约束

SQLite triggers 在正常运行时拒绝以下表的 `UPDATE` 和 `DELETE`：

```text
policy_revisions
policy_day_limits
usage_observations
radar_observations
alert_events
delivery_attempts
```

`daily_ledgers` 只允许更新未结束日期；`finalized_at_ms IS NOT NULL` 后 trigger
拒绝更新和删除。`quota_epochs`、`source_health` 和 `alert_deliveries` 是明确
的当前状态/工作队列表，可以按状态机更新。

修复历史的 migration 必须在事务内显式替换相关 trigger、记录 migration
checksum，并在 commit 前重新建立约束；普通 repository API 不暴露删除事实
的方法。

### 来源健康

```text
source_health
  source                    TEXT PRIMARY KEY CHECK (source IN ('codex', 'radar'))
  last_attempt_at_ms        INTEGER
  last_success_at_ms        INTEGER
  consecutive_failures      INTEGER NOT NULL CHECK (consecutive_failures >= 0)
  freshness                 TEXT NOT NULL
  public_error_code         TEXT
  error_fingerprint         BLOB
  updated_at_ms             INTEGER NOT NULL
```

这里只保存 allowlisted error code 与本机 salted fingerprint。不得保存
`reqwest`、keyring、SMTP 或操作系统的原始错误字符串。

## 提醒 outbox schema

```text
alert_events
  id                        INTEGER PRIMARY KEY
  event_key                 TEXT NOT NULL UNIQUE
  event_kind                TEXT NOT NULL
  account_stream_id         INTEGER REFERENCES account_streams(id)
  quota_epoch_id            INTEGER REFERENCES quota_epochs(id)
  local_date                TEXT
  watch_key                 TEXT
  threshold_units           INTEGER
  created_at_ms             INTEGER NOT NULL

alert_deliveries
  id                        INTEGER PRIMARY KEY
  delivery_key              TEXT NOT NULL UNIQUE
  alert_event_id            INTEGER NOT NULL REFERENCES alert_events(id)
  channel                   TEXT NOT NULL CHECK (channel IN ('system', 'email'))
  recipient_key             BLOB
  state                     TEXT NOT NULL
  attempt_count             INTEGER NOT NULL CHECK (attempt_count >= 0)
  next_attempt_at_ms        INTEGER
  lease_until_ms            INTEGER
  public_error_code         TEXT
  created_at_ms             INTEGER NOT NULL
  updated_at_ms             INTEGER NOT NULL

delivery_attempts
  id                        INTEGER PRIMARY KEY
  delivery_id               INTEGER NOT NULL REFERENCES alert_deliveries(id)
  attempted_at_ms           INTEGER NOT NULL
  outcome                   TEXT NOT NULL
  public_error_code         TEXT
  duration_ms               INTEGER NOT NULL CHECK (duration_ms >= 0)
```

`event_key` 编码既有去重规则：日期事件包含 stream/date/threshold，周事件包含
stream/epoch/threshold，Radar 包含 watch/threshold，来源失败包含 source 与
failure streak generation。

系统通知只有一条 delivery；邮件为当时每个 active recipient 建一条 delivery，
只保存 recipient key。渠道未配置时 delivery 使用 `paused_config`，而不是
删除 alert event。凭证恢复或设置保存后可以重新激活；永久凭证错误不做无限
网络重试。

## 系统凭证库模型

service 固定为最终 bundle identifier 加 `.smtp`：

```text
<bundle-id>.smtp
```

user 使用两个固定、不含身份信息的 slot：

```text
sender-slot-a
sender-slot-b
```

secret 是 SMTP 密码或应用专用密码。SQLite 的 `credential_ref` 只保存当前
active slot。更新密码时写另一个 slot，提交指针后再删除旧 slot；这允许在
SQLite 与外部凭证库之间实现可恢复的两阶段更新，而不原地覆盖唯一一份旧
secret。两个固定 user 名也保证数据库完全损坏时仍能删除本应用的全部 SMTP
凭证。

非秘密恢复 journal：

```text
external_change_journal
  id                        INTEGER PRIMARY KEY
  operation_key             TEXT NOT NULL UNIQUE
  kind                      TEXT NOT NULL
  phase                     TEXT NOT NULL
  old_credential_ref        TEXT
  new_credential_ref        TEXT
  old_autostart_enabled     INTEGER
  new_autostart_enabled     INTEGER
  created_at_ms             INTEGER NOT NULL
  updated_at_ms             INTEGER NOT NULL
```

设置提交顺序：

1. 完整验证 draft、auth 路径、策略总量、时区、SMTP/TLS 和邮箱；
2. 校验 `expected_settings_revision`；
3. 在 SQLite 写入 `prepared` journal；
4. 若密码操作是 `Set`，写入当前 active slot 的另一个 slot，并立即回读验证；
5. 若 autostart 改变，调用系统 adapter 并回读确认；
6. 在一个 SQLite transaction 中追加 policy revision（如有）、替换当前
   非秘密设置/收件人/偏好、切换 credential ref、增加 settings revision，
   将 journal 标为 `committed`；
7. commit 后删除旧 credential，清理 journal；删除失败保留 cleanup 状态并
   向 UI 显示可重试动作。

失败与崩溃恢复：

- SQLite commit 前失败：恢复旧 autostart，删除 staged new credential，旧
  配置继续有效；
- 启动时发现 `prepared`：以已提交 settings 为权威，恢复外部状态并清理 staged
  credential；
- 启动时发现 `committed` cleanup：重试删除旧 credential；
- vault `NoEntry/locked/denied/system` 只映射到稳定 public error code；
- 密码操作必须显式为 `Keep/Set/Delete`，空密码字符串不能隐式表示删除；
- 任何 DTO、Debug 输出、panic、日志或 crash context 都不得包含 secret。

## 日志与脱敏

生产日志只允许以下字段：

```text
timestamp
level
component
operation
public_error_code
http_status
duration_ms
attempt_count
settings_revision
dashboard_revision
local salted fingerprint
```

禁止字段：

```text
Authorization / Cookie headers
access_token / refresh_token / ID token
auth.json 原文或完整路径
Account ID / User ID / 邮箱
SMTP username/password
SMTP server response body
Codex/Radar 原始响应或错误正文
第三方推文全文
SQLite row dump
```

日志写入前使用结构化 allowlist，而不是写完整错误字符串后再用正则替换。
Rust error chain 只保留在内存，跨 IPC 和日志只输出稳定 error code。文件按
大小滚动，最多五个文件、每个最多 1 MiB；轮转失败时停止文件日志，不让磁盘
无界增长。

## 脱敏诊断导出

诊断导出只能从设置页由用户明确发起。UI 在创建文件前显示内容摘要，原生保存
对话框选择目标位置。

ZIP 内容：

```text
manifest.json
app.json
safe-settings.json
source-health.json
current-epoch-observations.json
logs/
```

允许内容：

- 应用版本、Rust/Tauri/WebView 版本、OS family/version/architecture；
- schema version、migration checksums、数据库完整性结果；
- settings revision、策略时区、七日额度、结转/提醒开关；
- auth 路径是否已配置/可读/格式有效，但不是路径本身；
- SMTP host 只保留“已配置”与 TLS mode，不含 host、username、from/recipient；
- source health、稳定 error code、最近 attempt/success 时间；
- 当前账号当前 quota epoch 的规范化时间戳、used units、reset time、每日投影
  和 event keys；
- 已经通过同一 allowlist 的滚动日志。

明确排除：

- 旧账号流及其历史；
- `app_instance_id`、salt、account/recipient fingerprints；
- Token、JWT、cookie、credential ref、secret；
- auth 路径、邮箱地址、SMTP host/username；
- 原始上游、SMTP 或系统错误 body；
- 数据库、备份或 Keychain/Credential Manager dump。

导出先写入权限受限的随机临时目录，逐文件重新序列化 allowlisted DTO，再压缩
到用户路径；不能直接压缩数据库或日志目录。成功、失败或取消后都删除临时
目录。诊断导出不上传、不自动附加邮件，也不产生网络请求。

## 损坏检测、备份与恢复

启动顺序：

1. 建立并验证应用目录权限；
2. 检查遗留 `.wal/.shm`，由 SQLite 正常 recovery；
3. 打开数据库并执行 `quick_check`；
4. 如需 migration，创建和验证滚动备份后迁移；
5. 运行关键不变量检查：singleton、七日 policy rows、active epoch unique、
   foreign keys、credential journal；
6. 恢复未完成的 external changes；
7. 重建可重建投影并启动 scheduler/delivery workers。

如果 open、integrity check 或 migration 失败：

1. 停止所有写入；
2. 把原数据库及 WAL/SHM 移到 `recovery/` 的时间戳目录，不覆盖任何旧文件；
3. 从新到旧验证三份备份；
4. 自动复制最近有效备份到新的 `state.sqlite3`，重新迁移并做完整性检查；
5. 成功后进入应用并显示持久恢复通知，保留隔离原库供用户导出诊断；
6. 没有有效备份则进入专用恢复界面，不能创建空库假装正常。

恢复界面只提供：

- 导出脱敏诊断；
- 打开应用数据目录；
- 重试恢复；
- 经二次确认执行“清除全部本地数据”。

v1 不提供任意 SQLite 文件导入，也不尝试从 Node 原型数据库迁移。

## 清除全部本地数据

这是显式、不可撤销操作，必须二次确认。流程：

1. 停止 scheduler、delivery worker 和更新中的设置事务；
2. 删除本应用 service 下固定的 `sender-slot-a` 与 `sender-slot-b` 并确认结果；
4. 凭证库拒绝或失败时停止，显示重试/系统凭证库打开指引，不能声称已清除；
5. 关闭 SQLite 并删除数据库、WAL/SHM、备份、恢复副本、日志与导出临时目录；
6. 清除 autostart，重新创建空数据目录并回到首次运行；
7. 不删除、修改或 chmod 用户的 `auth.json`。

清除操作不删除应用 binary，也不触碰其他应用的 Keychain/Credential Manager
items。完成后旧账号、邮件、策略和提醒均不可恢复。

## 必须实现的约束测试

### schema 与历史

1. 空库建立完整 schema、默认策略与默认提醒偏好。
2. 每个 migration checksum 固定，旧版本逐级迁移且中途失败完全回滚。
3. schema 较新时只读失败，不执行降级。
4. policy revision 恰有七天且总量不超过 100%。
5. 策略/时区修改不改变已结束 daily ledger。
6. 账号切换隐藏旧流但不合并；切回时恢复同一 stream。
7. usage/radar observation、alert event 和 delivery attempt 不可更新或删除。

### 原子设置与凭证

8. `expected_settings_revision` 冲突不会覆盖已保存设置。
9. 无效 auth 路径、时区、SMTP TLS、邮箱或额度不会产生部分更新。
10. vault Set/回读失败保持旧设置和旧 secret。
11. SQLite commit 失败会恢复 autostart 并删除 staged secret。
12. 每个 journal crash point 重启后都收敛到旧配置或完整新配置。
13. `Keep/Set/Delete` 行为互不混淆，Public DTO 永不回显 password。
14. 缺失/锁定/拒绝 vault 时邮件暂停，系统通知和事件记录不受影响。
15. 测试邮件失败不回滚已保存配置。

### 权限、恢复与脱敏

16. macOS mode 与 Windows DACL 不安全时拒绝继续写入。
17. 备份在删除旧备份前通过完整性检查。
18. 主库损坏可恢复最近有效备份；全部备份损坏时不会静默建空库。
19. 日志轮转总量不超过 5 MiB，并对禁止值运行 canary secret 测试。
20. 诊断 ZIP 的每个文件通过 forbidden-field scanner，且不包含数据库文件。
21. “清除全部本地数据”删除所有已知 vault refs 与本应用数据，但不触碰
    `auth.json`。

测试使用带 canary token、邮箱、路径、账号 ID 和 SMTP 密码的 fixture；CI
对所有 Public DTO、日志和诊断 ZIP 做递归扫描，任何 canary 出现都失败。

## 交给实施的固定接口

`SettingsManager` 对 UI 暴露的 draft 形状：

```text
SettingsDraft
  expected_settings_revision
  auth_path
  policy_timezone
  seven_day_base_units[7]
  carry_workdays_enabled
  autostart_enabled
  smtp { enabled, host, port, tls_mode, username, from, recipients[] }
  alert_preferences[event_kind][channel]

SecretUpdate = Keep | Set(SecretString) | Delete
```

返回：

```text
PublicSettings
  settings_revision
  normalized non-secret settings
  auth_path_status
  smtp_credential_status
  notification_permission_status
  last_test_email_result
```

`last_test_email_result` 只保存在内存/UI session；关闭应用后不需要恢复。永久
投递事实仍由 outbox 表保存。

此模型已经固定配置、事实、状态、凭证、权限、迁移、备份、恢复、诊断和清除
边界。实现可以调整内部列名或拆分查询投影，但不得改变以下不变量：

- secrets 不进入 SQLite、日志、IPC 或诊断；
- `auth.json` 永远只读；
- 历史事实与策略 revision 不回写；
- 设置要么完整提交，要么保持旧状态；
- 损坏恢复不静默丢数据；
- UI 始终只展示当前账号。
