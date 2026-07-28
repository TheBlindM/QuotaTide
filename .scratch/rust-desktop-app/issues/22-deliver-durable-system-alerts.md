# 22 — 实现持久提醒与系统通知

**What to build:** 将额度、重置预测和采集故障转换为持久、去重、可恢复的
提醒事件，并通过系统通知送达；通知失败时用户仍能在应用内看到事件。

**Blocked by:** 19 — 实现每日策略与工作日结转；20 — 接入 Reset Radar；21 — 完成原子设置与后台生命周期

**Status:** ready-for-agent

- [ ] 支持每日 80%/100%、周剩余 20%/10%、Radar 70% 档位、确认新 epoch
  和同一来源连续失败 3 次的稳定提醒种类。
- [ ] `QuotaLedger` 只依据上次持久状态到本次状态的阈值跨越产生 candidate，
  UI 不自行推测事件。
- [ ] alert key 使用账号流、epoch、自然日或预测窗口、事件种类和阈值按契约
  去重；刷新和重启不会重复创建事件。
- [ ] 观测、ledger transition、alert event 和启用渠道的 pending delivery
  在同一 SQLite transaction 提交。
- [ ] DeliveryWorker 使用 lease claim；崩溃后可重试，同一 alert/channel
  不创建第二个用户提醒。
- [ ] 系统通知权限状态区分 unknown/granted/denied/error，只在用户完成配置
  或主动开启时申请。
- [ ] 通知由 Rust 发送；拒绝或发送失败保留 in-app 提醒和安全错误，不撤销
  其他渠道。
- [ ] 点击通知唤起已有实例并聚焦今日额度、Radar 或错误区域；不会重复启动
  scheduler。
- [ ] Windows 的名称、图标和点击行为必须由安装后的应用验证，开发态
  PowerShell 身份不作为验收证据。
- [ ] 测试覆盖 threshold crossing、去重、lease expiry、重启、权限拒绝、
  渠道隔离、sleep recovery 和通知正文 secret canary 扫描。
