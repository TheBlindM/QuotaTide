# 16 — 完成当前账号配置流程

**What to build:** 让用户能从设置中选择 Codex 自动维护的 `auth.json`，由
Rust 只读验证并安全保存路径，随后看到当前账号已配置或可操作的错误状态。

**Blocked by:** 15 — 打通 Weekly Ledger 托盘窗口

**Status:** closed

- [x] 设置中的原生文件选择器限制为单个 JSON；取消选择不修改现有设置。
- [x] 原生 dialog 期间启用 modal guard，窗口不会因失焦隐藏并中断选择。
- [x] Rust 对候选路径做规范化、只读打开和严格所需字段解析，成功后才原子替换
  当前路径。
- [x] 每次验证都不写入、移动、删除、chmod 或刷新 `auth.json`；测试使用文件
  hash、权限和修改时间证明只读契约。
- [x] 只接受 access token 与 canonical Account ID；JWT、文件全文和原始解析
  错误不进入 UI、SQLite、普通日志或测试快照。
- [x] 非秘密设置进入版本化 SQLite，应用首次启动、重启和路径修改后状态一致。
- [x] 同一账号生成稳定账号流 identity；切换到不同账号时不合并历史，界面只
  投影当前账号。
- [x] 不可读、无权限、缺字段、错误 JSON 和非 Codex 文件产生稳定错误码、
  本地化 message key 与 allowlisted safe context。
- [x] UI 只显示脱敏账号状态和路径摘要，不显示 token、完整 Account ID 或可
  复制的认证内容。
- [x] command、SQLite 和 UI 测试使用 canary secret 递归扫描，证明认证材料
  没有跨越公开边界。

## Comments

- Resolution: commits `9258495`, `73aa627`, and `8a9f4d1` deliver the native
  single-file picker, modal focus guard, strict read-only Codex auth validation,
  the core-owned `SettingsManager`/application facade, optimistic revision
  checks, stable account-stream identities, redacted IPC DTOs, and versioned
  atomic SQLite persistence.
- Security: macOS rejects foreign-owned or symlinked state targets and enforces
  `0700`/`0600`; Windows constructs and read-back verifies a protected DACL
  containing only the current user, SYSTEM, and Administrators. Auth source
  files are proven unchanged by hash, permissions, and mtime. Command,
  SQLite/WAL/SHM, and UI tests inject and recursively scan token, Account ID,
  and JWT canaries.
- Verification: Rust fmt/check/clippy/full tests, UI lint/typecheck/14 component
  tests/production build, legacy Node checks/27 tests, offline cargo-deny, a
  native macOS launch, and a macOS `.app` bundle build passed. Native macOS
  state permissions were confirmed as current-user `0700`/`0600`; Windows ACL
  behavior received static review and remains an owning release-matrix smoke
  gate for Ticket 27.
- Review: all Standards and Spec findings were fixed; both final targeted
  re-reviews passed.
