# 23 — 实现安全邮件投递

**What to build:** 让用户配置一个 SMTP 发件邮箱和多个收件邮箱，把已有提醒
事件安全地通过 TLS 邮件送达，同时确保密码只存在于系统凭证库且所有保存、
测试和重试操作可恢复。

**Blocked by:** 21 — 完成原子设置与后台生命周期；22 — 实现持久提醒与系统通知

**Status:** closed

- [x] 邮件设置支持 enabled、host、port、TLS mode、username、from 和多个
  可启停 recipient，完整校验后才保存。
- [x] SMTP 密码使用 `Keep/Set/Delete` 语义，保存到
  `dev.theblind.quotatide.smtp` 下两个固定系统凭证 slot。
- [x] 新 secret 先写入并回读确认，再通过 journal 与 SQLite 设置提交；失败时
  清理 staged slot 并保持旧设置和旧 secret。
- [x] macOS Keychain、Windows Credential Manager 的 NoEntry、locked、
  denied 和 system error 映射为安全状态，不创建文件、环境变量或 SQLite
  明文 fallback。
- [x] SMTP transport 复用连接，使用 rustls，只提供 implicit TLS relay 或
  required STARTTLS；明文和 opportunistic downgrade 不可配置。
- [x] “发送测试邮件”使用相同校验、TLS、超时和脱敏规则，但不进入额度提醒
  outbox，也不回滚已经保存的设置。
- [x] 每个 active recipient 创建独立 delivery；部分成功不会撤销成功项或
  重复创建 alert event。
- [x] transient/permanent 错误分类、bounded exponential backoff、lease 和
  idempotency 在重启后保持。
- [x] UI 只显示密码“已配置/未配置”，保存成功后清空输入，不提供可恢复掩码。
- [x] 自动测试覆盖 TLS 模式约束、超时与 transient/permanent 分类、逐收件人
  部分失败、凭据缺失、outbox 隔离和 canary secret 扫描；真实 SMTP
  invalid-certificate、认证、4xx/5xx 与双平台凭据库安装态证据纳入 Ticket 27
  的 release-candidate QA，不以个人邮箱或伪造服务器结果冒充通过。

## Comments

- 2026-07-30：完成 SQLite v10 SMTP 非秘密配置、active recipient 与逐收件人
  delivery，密码使用 `Keep/Set/Delete` 和 `slot-a`/`slot-b` 双槽 journal
  提交；新密码必须先写入并回读，崩溃恢复会清理未提交或旧 slot。
- macOS 使用 Keychain、Windows 使用 Credential Manager；凭据不可用只暂停
  邮件，系统通知与应用内提醒独立继续。SMTP 仅提供 implicit TLS 和 required
  STARTTLS，使用 rustls、连接池、20 秒 transport timeout 与 30 秒总超时。
- 设置页支持多个可启停收件地址、凭据状态和独立测试邮件；测试邮件不进入
  outbox。后台系统通知与 SMTP sweep 并发运行，慢邮件不会阻塞本地通知。
- workspace 全量测试、严格 clippy、前端 31 项测试与 production build、
  离线依赖审计和 macOS `.app` bundle 均通过。Windows cross-check 到
  `ring` 原生编译时因本机缺少 Windows/MSVC C 头文件停止，最终 Windows
  Credential Manager 与 SMTP live matrix 由 Ticket 27 在 Windows runner
  和安装态 release candidate 上执行。
