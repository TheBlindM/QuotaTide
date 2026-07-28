# 23 — 实现安全邮件投递

**What to build:** 让用户配置一个 SMTP 发件邮箱和多个收件邮箱，把已有提醒
事件安全地通过 TLS 邮件送达，同时确保密码只存在于系统凭证库且所有保存、
测试和重试操作可恢复。

**Blocked by:** 21 — 完成原子设置与后台生命周期；22 — 实现持久提醒与系统通知

**Status:** ready-for-agent

- [ ] 邮件设置支持 enabled、host、port、TLS mode、username、from 和多个
  可启停 recipient，完整校验后才保存。
- [ ] SMTP 密码使用 `Keep/Set/Delete` 语义，保存到
  `dev.theblind.quotatide.smtp` 下两个固定系统凭证 slot。
- [ ] 新 secret 先写入并回读确认，再通过 journal 与 SQLite 设置提交；失败时
  清理 staged slot 并保持旧设置和旧 secret。
- [ ] macOS Keychain、Windows Credential Manager 的 NoEntry、locked、
  denied 和 system error 映射为安全状态，不创建文件、环境变量或 SQLite
  明文 fallback。
- [ ] SMTP transport 复用连接，使用 rustls，只提供 implicit TLS relay 或
  required STARTTLS；明文和 opportunistic downgrade 不可配置。
- [ ] “发送测试邮件”使用相同校验、TLS、超时和脱敏规则，但不进入额度提醒
  outbox，也不回滚已经保存的设置。
- [ ] 每个 active recipient 创建独立 delivery；部分成功不会撤销成功项或
  重复创建 alert event。
- [ ] transient/permanent 错误分类、bounded exponential backoff、lease 和
  idempotency 在重启后保持。
- [ ] UI 只显示密码“已配置/未配置”，保存成功后清空输入，不提供可恢复掩码。
- [ ] 集成与受控 live tests 覆盖 TLS relay、required STARTTLS、无效证书、
  认证失败、超时、4xx/5xx、多收件人部分失败和 canary secret 扫描。
