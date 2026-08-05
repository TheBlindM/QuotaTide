# Codex Runway 对 QuotaTide 的可借鉴点

调研日期：2026-07-31
对照版本：[`Licoy/codex-runway@4f7ec0c`](https://github.com/Licoy/codex-runway/commit/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8)

## 结论

有值得借鉴的内容，但不应把 Codex Runway 的完整产品范围搬进 QuotaTide。Codex Runway 是 macOS 原生状态栏“额度 + 多账号 + 成本/Token + 会话”工具；QuotaTide 的优势是跨 macOS/Windows、单账号、只读 `auth.json`、围绕当前七日窗口做预算和提醒。最合适的方向是补强“额度快用完之前的决策能力”，而不是扩成 Codex 账号管理器。

QuotaTide 当前已有能力以本仓库的 [README](../../README.md)、[产品规范](../spec.md) 和 [托盘 UI 决策](tray-window-ui.md) 为准。

## 最高价值的 5 个借鉴点

### 1. P0：只读展示 reset credits，并提醒即将到期

Codex Runway 除 `/wham/usage` 外，还读取 `/wham/rate-limit-reset-credits`；其模型保留 available count、状态、到期时间，并把 7 天内到期的可用 credit 单独归类，再生成一次性到期提醒。来源：[QuotaClient.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/QuotaClient.swift)、[Models.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/Models.swift)、[DisplayModels.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/DisplayModels.swift)、[RunwayAlerts.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RunwayAlerts.swift)。

适配 QuotaTide：只在“明细”或通知设置里显示“可用 N 次 / 最近一枚 X 天后到期”，到期临近时提醒；不自动使用、不修改账号、不占据默认概览。它与“避免浪费额度”定位高度一致，也不破坏单账号和只读边界。

风险：这是未作为稳定公共 API 承诺的后端路径；必须允许字段缺失、独立标记来源健康，并在失败时隐藏该功能，不能让它拖垮周额度采集。

### 2. P0：把重置雷达从“一个概率”升级为可验证的事件状态机

Codex Runway 没有把 feed 当成一个裸百分比：它区分 `reset_completed`、`reset_scheduled`、`banked_reset`、`limit_increase` 和 `uncertain`，按本地自然日判断“已生效”与“今日稍后生效”，选择最能解释当前结论的证据事件；feed 降级、最后成功检查超过 30 小时或同日存在不确定事件时返回 `unknown`。来源：[RateLimitResetTodayModels.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RateLimitResetTodayModels.swift)。

它还对 schema 版本、事件数、置信度、来源域名/路径、事件与 `effectiveAt` 的组合做 fail-closed 校验，并用应用自有固定文案解释事件，不直接显示 feed 的自由文本。来源：[RateLimitResetTodayDecoding.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RateLimitResetTodayDecoding.swift)、[status.schema.json](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/api/hasreset/schemas/status.schema.json)。

适配 QuotaTide：当前已经验证来源、时间和置信度，下一步应统一成 `fresh / degraded / stale / unknown`，明确显示“已发生”或“预计于 X 生效”，并保留可点击的原始来源。雷达不能改变账号真实重置时间或每日预算。

### 3. P1：把“平均速率”变成“会不会在重置前用完”

Codex Runway 的 `QuotaBurnProjection` 不只显示 `%/h`，还计算按当前速度预计用尽时间，以及到重置时预计使用百分比；若预计用尽发生在重置之后，则不制造一个“提前耗尽”时间。来源：[DisplayModels.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/DisplayModels.swift)。

适配 QuotaTide：保留现有托盘速率，在周卡片或展开明细增加一句结论：

- `按当前速度，预计周日 18:20 用尽（早于重置 9 小时）`；或
- `按当前速度，到重置预计使用 82%`。

QuotaTide 有小时级本地快照，应优先用最近 6–24 小时的稳健斜率并要求最少样本数，而不是只用“窗口已用 ÷ 窗口已过时间”；数据不足时显示“样本不足”，避免早期单次大任务造成夸张预测。

### 4. P1：提供少量可选的任务栏信息密度

Codex Runway 把状态栏渲染做成独立计划，支持 countdown、battery、meters、rings，并可选择显示剩余百分比、重置倒计时或二者；布局会按 meter 数量调整行列和宽度。来源：[StatusBarRenderPlan.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunway/StatusBarRenderPlan.swift)、[StatusBarContentLayout.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunway/StatusBarContentLayout.swift)。

适配 QuotaTide：不要照搬四套复杂样式，只提供三个跨平台选项即可：

1. 波浪圆环（当前默认）；
2. 圆环 + 周剩余；
3. 圆环 + 重置倒计时。

每日已用/超额继续放 tooltip 或弹层，避免 macOS 菜单栏和 Windows 托盘变得过宽。Windows 托盘不保证持续显示文本，因此文本模式应自然降级为 tooltip。

### 5. P2：可选的、无凭证的本地状态快照

Codex Runway 会把配额窗口、更新时间、成本和会话摘要编码为 `~/.codex-runway/status.json`，采用原子写入，供本机其他工具读取；结构中不含 access token。来源：[RunwayStatusExport.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RunwayStatusExport.swift)。

适配 QuotaTide：作为关闭默认、用户主动开启的“本地集成”功能，只导出周剩余、今日可用、重置时间、来源新鲜度和生成时间，方便 Raycast、PowerShell、桌面小组件或自动化读取。文件应原子写入、限制权限并在设置中明确“用量也是隐私数据”。现有诊断 ZIP 继续用于排障，两者不是同一功能。

## 已经被 QuotaTide 吸收的好思路

- 两者都通过 bearer token 和 `ChatGPT-Account-Id` 请求 `https://chatgpt.com/backend-api/wham/usage`；Codex Runway 的实现见 [QuotaClient.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/QuotaClient.swift)。
- 两者都使用 `https://www.codexrunway.com/api/status.json`；Codex Runway 的客户端见 [RateLimitResetTodayClient.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RateLimitResetTodayClient.swift)。
- QuotaTide 已有来源新鲜度、保留最后成功数据、可删除提醒、系统通知/邮件共享事件身份、诊断导出、更新检测、深浅色和中英文；这些不需要再次照搬。
- Codex Runway 用包含窗口、阈值和重置 epoch 的语义 ID 去重，首次成功加载不补发“早已存在”的重置通知，并只保留最近 200 个 seen IDs；这是 QuotaTide 告警语义可以继续对照的细节。来源：[RunwayAlerts.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RunwayAlerts.swift)。

## 不建议借鉴

### 多账号导入、切号和 token 刷新

Codex Runway 支持浏览器登录、粘贴/导入凭据、保存多账号，并在确认后原子写回 `~/.codex/auth.json`。来源：[README.md](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/README.md)、[AccountStore.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/AccountStore.swift)。这与 QuotaTide 已确认的“单账号、`auth.json` 永远只读、Codex 自己刷新 token”边界直接冲突，也会显著扩大凭证存储和误切账号风险。

### 会话扫描、API 等价成本和全年 Token 热力图

Codex Runway 会扫描本机 Codex JSONL、维护 SQLite 增量索引、显示最近会话、API 等价成本和年度图表。来源：[README.md](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/README.md)、[SessionActivity.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/SessionActivity.swift)、[UsageCostRepository.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/UsageCostRepository.swift)。其 README 也明确说明本机历史日志没有可靠账号归属，可能跨账号，官方统计和本机日志口径不能直接相减。对 QuotaTide 来说，这会破坏紧凑度、增加会话隐私面，并偏离“共享账号七日额度管理”。

### 把 5 小时窗口和所有附加窗口放进主界面

Codex Runway 会同时展示 5 小时、每周和附加窗口。来源：[README.md](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/README.md)、[Models.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/Models.swift)。QuotaTide 的核心是严格当前七日窗口与每日策略，主界面继续只保留这一窗口更清晰；解析层可忽略未知附加窗口，避免上游扩展导致失败。

### 直接复制 Swift 代码或视觉实现

Codex Runway 当前仅声明 macOS 12+，依赖 AppKit/SwiftUI，并以 AGPL-3.0 发布。来源：[Package.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Package.swift)、[LICENSE](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/LICENSE)。QuotaTide 是 Rust/Tauri、MIT、macOS/Windows；可以借鉴产品思想和独立重写算法，但不要复制 AGPL 源码、资源或大段独创表达，否则可能需要按 AGPL 对衍生作品履行许可义务。此项是工程风险提示，不是法律意见。

## 建议路线

1. **先做雷达状态机硬化和耗尽预测**：不增加主界面密度，却直接提升判断质量。
2. **再做 reset credits 只读卡片与到期提醒**：作为明细中的可选能力，独立失败。
3. **随后提供三种托盘信息模式**：保留波浪圆环为默认，Windows 自动降级。
4. **状态 JSON 放在高级设置**：只有明确的本地自动化需求再实现。

## 主要一手来源

- [项目 README](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/README.md)
- [额度与 reset credits 请求](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/QuotaClient.swift)
- [额度、reset credits 与窗口模型](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/Models.swift)
- [耗尽预测与 reset credits 风险分类](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/DisplayModels.swift)
- [提醒判定与去重](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RunwayAlerts.swift)
- [状态栏布局策略](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunway/StatusBarContentLayout.swift)
- [雷达客户端与事件模型](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RateLimitResetTodayClient.swift)、[RateLimitResetTodayModels.swift](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RateLimitResetTodayModels.swift)
- [本地状态导出](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/Sources/CodexRunwayCore/RunwayStatusExport.swift)
- [AGPL-3.0 许可证](https://github.com/Licoy/codex-runway/blob/4f7ec0c423f0c45d3b2b9518e118d90128ca6ac8/LICENSE)
