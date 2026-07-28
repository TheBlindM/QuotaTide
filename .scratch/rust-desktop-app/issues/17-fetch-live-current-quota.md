# 17 — 接入当前账号真实额度

**What to build:** 使用当前账号的只读认证材料查询真实 Codex 七日额度，让
概览展示周已用、剩余、计划重置、最后成功时间和来源健康，并按启动、每小时
和用户手动操作可靠刷新。

**Blocked by:** 16 — 完成当前账号配置流程

**Status:** ready-for-agent

- [ ] Codex adapter 使用已确认的 WHAM endpoint、Authorization、
  `chatgpt-account-id`、originator 与 Accept 契约，固定 HTTPS origin。
- [ ] 只选择严格 604800 秒的 current window；更短窗口、缺失字段、非有限数、
  非法时间和歧义多窗口均安全失败。
- [ ] 启动触发一次刷新，之后每小时一次；睡眠期间跳过积压 tick，唤醒只执行
  一次过期刷新。
- [ ] 手动刷新有 30 秒冷却，启动/定时/设置触发与手动触发通过 single-flight
  合并为同一轮结果。
- [ ] 每轮在发请求前重新只读打开 `auth.json`；401/403 只有在磁盘 token
  已变化时才重试一次。
- [ ] HTTP client 复用连接并使用 15 秒超时；错误按认证、权限、限流、超时、
  上游和契约分类且不回显原始响应 body。
- [ ] 成功观测与来源健康在同一 SQLite transaction 中提交；失败保留最后成功
  数据并标记 stale，不把额度改成 0% 或 100%。
- [ ] 概览显示周已用/剩余、相对与绝对重置时间、最后成功时间、连续失败次数
  和刷新中/新鲜/过期状态。
- [ ] fake clock/source 集成测试覆盖调度、并发触发、token rotation、超时、
  部分写入回滚和 last-known-good。
- [ ] 真实账号验证只能是显式 ignored/manual 测试，不进入默认 CI、fixture、
  日志、快照或 artifact。
