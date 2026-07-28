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
