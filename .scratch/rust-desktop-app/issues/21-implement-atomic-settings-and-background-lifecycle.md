# 21 — 完成原子设置与后台生命周期

**What to build:** 把账号、额度、时区、提醒偏好和开机启动统一为可验证、可
回滚的设置操作，并让 QuotaTide 在登录启动、睡眠恢复和多次启动时保持一个
安静可靠的后台实例。

**Blocked by:** 15 — 打通 Weekly Ledger 托盘窗口；19 — 实现每日策略与工作日结转

**Status:** closed

- [x] 设置查询返回 revision 和全部非秘密公开值；保存要求匹配
  `expected_settings_revision`，冲突不会覆盖新状态。
- [x] 路径、策略、时区、提醒偏好和开机启动先完整验证，再作为一个用户操作
  提交，任一失败不留下部分更新。
- [x] 开机启动 adapter 支持 query/enable/disable；macOS 使用 LaunchAgent，
  Windows 使用当前用户启动入口。
- [x] 外部状态变更通过可恢复 journal 与 SQLite commit 协调；每个 crash point
  重启后收敛到完整旧设置或完整新设置。
- [x] 开机启动写入失败会回滚 UI 开关与普通配置，不影响用户手动启动应用。
- [x] 登录启动只创建 tray、scheduler 和 delivery workers，不显示窗口。
- [x] 第二次启动不会创建第二个 scheduler、窗口或 tray，而是安全唤起已有实例。
- [x] 睡眠恢复跳过过期任务洪峰，tray/window 状态仍可操作，后台 worker 不
  重复创建。
- [x] native dialog、通知权限和未来凭证提示共享 modal guard；guard 期间
  focus loss 不隐藏窗口。
- [x] 集成测试覆盖 revision conflict、validation failure、autostart failure、
  SQLite commit failure、journal crash recovery、single instance 和 login
  launch。

## Comments

- 2026-07-30：完成 SQLite v8 原子设置、可恢复外部变更 journal、LaunchAgent/
  Windows 当前用户自启、登录隐藏启动、单实例唤醒、scheduler 与 delivery
  worker 单启动保护、共享 modal guard 和统一设置页。固定点 `5be493f` 的
  Standards/Spec 双轴审查已通过；`cargo fmt`、全 workspace clippy/test、
  `cargo deny check`、前端 check、版本/身份及 bindings 门禁全部通过。
