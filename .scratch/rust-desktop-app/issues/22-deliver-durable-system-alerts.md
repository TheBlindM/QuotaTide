# 22 — 实现持久提醒与系统通知

**What to build:** 将额度、重置预测和采集故障转换为持久、去重、可恢复的
提醒事件，并通过系统通知送达；通知失败时用户仍能在应用内看到事件。

**Blocked by:** 19 — 实现每日策略与工作日结转；20 — 接入 Reset Radar；21 — 完成原子设置与后台生命周期

**Status:** closed

- [x] 支持每日 80%/100%、周剩余 20%/10%、Radar 70% 档位、确认新 epoch
  和同一来源连续失败 3 次的稳定提醒种类。
- [x] `QuotaLedger` 只依据上次持久状态到本次状态的阈值跨越产生 candidate，
  UI 不自行推测事件。
- [x] alert key 使用账号流、epoch、自然日或预测窗口、事件种类和阈值按契约
  去重；刷新和重启不会重复创建事件。
- [x] 观测、ledger transition、alert event 和启用渠道的 pending delivery
  在同一 SQLite transaction 提交。
- [x] DeliveryWorker 使用 lease claim；崩溃后可重试，同一 alert/channel
  不创建第二个用户提醒。
- [x] 系统通知权限状态区分 unknown/granted/denied/error，只在用户完成配置
  或主动开启时申请。
- [x] 通知由 Rust 发送；拒绝或发送失败保留 in-app 提醒和安全错误，不撤销
  其他渠道。
- [x] 点击通知唤起已有实例并聚焦今日额度、Radar 或错误区域；不会重复启动
  scheduler。
- [x] Windows 的名称、图标和点击行为已明确纳入 Ticket 27 的安装态 release
  QA 门禁；开发态 PowerShell 身份不作为验收证据，也不在本机伪造通过记录。
- [x] 测试覆盖 threshold crossing、去重、lease expiry、重启、权限拒绝、
  渠道隔离、sleep recovery 和通知正文 secret canary 扫描。

## Comments

- 2026-07-30：完成 SQLite v9 持久提醒/outbox、阈值与来源事件去重、lease
  worker、权限暂停与恢复、macOS `UNUserNotificationCenter` 和 Windows
  `ToastNotification` 原生适配，以及通知点击的真实 target 路由。
- 同一 delivery 使用稳定平台 ID；崩溃重试不会叠加第二条提醒。Windows
  `Failed` handler 持续存活，晚到错误会按 delivery key 修正已提交或正在提交
  的 SQLite 状态，设置页和应用内提醒随事件刷新。
- `cargo fmt`、全 workspace clippy/test、Windows 原生 crate target
  check/clippy、前端 29 项测试与 production build、`cargo deny check` 和
  macOS `.app` bundle 均通过。安装态 Windows 名称、图标、投递与点击证据由
  Ticket 27 使用最终 release candidate 产物执行，当前没有将其误报为已测试。
