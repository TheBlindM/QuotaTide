Status: in_progress
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./02-verify-platform-integrations.md, ./03-audit-upstream-data-contracts.md, ./05-choose-application-architecture.md
Assignee: codex

# 决定配置、状态与本地安全模型

## Question

决定七日策略、账号路径、收发件配置、提醒偏好、历史快照、凭证引用和诊断日志的持久化模型；明确配置版本迁移、原子写入、凭证缺失、文件权限、脱敏导出与损坏恢复行为。用户已经决定 SMTP 密码不进入普通配置文件、历史策略不回算。

## Comments

- 2026-07-28：开始收敛配置、SQLite 事实、系统凭证库、诊断与恢复语义。
  先以现有产品约束生成候选模型，再只向用户确认无法从既有决定推导的安全与
  数据保留取舍。
- 2026-07-28：用户确认切换 `auth.json` 所指账号后，旧账号流继续保存在本机
  SQLite，但 UI 只展示当前账号；不同账号流永不合并。设置中提供“清除全部
  本地数据”，在用户明确确认后删除历史、配置与凭证引用。
- 2026-07-28：用户确认不提供需要手工编辑的 `config.json`。所有非秘密设置
  通过桌面界面提交并写入版本化 SQLite；SMTP 密码只写系统凭证库。开源用户
  可导出脱敏诊断，但不能通过普通文件绕过设置校验或原子提交。
