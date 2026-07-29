Status: open
Type: wayfinder:map

# Rust 跨平台托盘桌面端路线图

## Destination

形成一份可直接进入 `/to-spec` 与 `/to-tickets` 的 Rust 桌面端 v1 产品与架构决策集。它必须让 macOS 与 Windows 托盘应用的技术栈、交互、数据与安全边界、通知、发布方式都不再存在阻碍实施的关键未知项。

## Notes

- 产品面向最终用户，v1 同等支持 macOS 与 Windows。
- canonical domain vocabulary 位于 [`CONTEXT.md`](../../CONTEXT.md)。
- 开始每个 ticket 前读取 `CONTEXT.md`、`docs/spec.md` 和相关 `docs/research/` 文档。
- 核心能力使用 Rust；允许 WebView 界面层。
- 产品只常驻系统托盘，不提供传统主窗口；紧凑窗口内包含概览与设置。
- 界面采用 Apple 风格的毛玻璃层次，在两端做平台适配。
- v1 只监控一个当前账号；`auth.json` 自动检测、可选择、始终只读，每小时重新读取。
- 七日策略模板默认 `16/16/16/16/16/10/10`，基础额度总和不得超过 100%，工作日可启用未用额度结转。
- 策略修改立即影响今天与未来，历史保留当时快照。
- 系统通知默认开启，邮件按提醒事件配置；SMTP 密码进入系统凭证库，支持多个收件邮箱。
- 不迁移 Node 原型运行数据；当前实现暂作为行为参考，开源前清理 legacy 代码。
- 默认无遥测，数据留在本机，诊断导出自动脱敏。
- 使用 MIT License。
- Wayfinding 只产出 decisions，不实现最终应用。

## Decisions so far

- [选择 Rust 跨平台桌面技术栈](./issues/01-choose-rust-desktop-stack.md) — 采用 Tauri 2 作为桌面壳，业务与系统能力归 Rust 核心，本地 WebView 只承载 UI；Slint 1.17 保留为 fallback。
- [验证 macOS 与 Windows 平台集成](./issues/02-verify-platform-integrations.md) — 托盘与系统能力走 Rust-side Tauri adapters；两端均有不透明视觉降级，macOS 完整毛玻璃使用 private API，不能默认承诺 App Store 兼容。
- [审核 Codex 与重置雷达数据契约](./issues/03-audit-upstream-data-contracts.md) — `auth.json` 每轮只读重开；严格识别当前账号的 604800 秒窗口；账号 stream、quota epoch 与第三方雷达相互隔离；失败保留最后成功值，雷达预测不能确认账号重置。
- [原型化紧凑托盘窗口](./issues/04-prototype-tray-window.md) — 选择 420×680 的 B — Weekly Ledger；概览把当前额度窗口七天作为主结构，设置分额度/账号/通知，原生毛玻璃配不透明降级，light/dark 与空、告警、过期状态均有固定行为。
- [决定桌面应用架构与模块边界](./issues/05-choose-application-architecture.md) — 采用 `quota-core` 深模块、Tauri 生产适配层和轻量 Preact UI；刷新 actor、SQLite 事实与 outbox、独立 delivery worker 形成可测试事务边界，旧 Node 只保留行为契约并在开源前移除。
- [调研跨平台发布与更新链路](./issues/06-research-release-pipeline.md) — 采用 macOS universal Developer ID DMG、Windows x64 per-user NSIS 与条件式 Authenticode、GitHub protected immutable Releases 和强制 Tauri updater 签名；静态 endpoint 仅向前修复，发布凭证与 fork PR 严格隔离。
- [决定配置、状态与本地安全模型](./issues/07-decide-config-state-security.md) — 非秘密配置、事实、投影与 outbox 进入版本化 SQLite，SMTP 密码使用双 slot 系统凭证；历史持续本地保留，设置原子提交，迁移前备份，损坏可恢复，诊断导出严格脱敏。
- [决定安装、更新与开源发布策略](./issues/08-decide-distribution-policy.md) — v1 以 GitHub Releases 发布 `0.x` 未签名预览版：macOS universal DMG、Windows x64 per-user NSIS、强制 updater 验签、用户确认安装、higher-patch roll-forward 与单维护者受控发布。
- [确定产品身份与开源仓库](./issues/09-decide-product-identity-and-repository.md) — 产品名 QuotaTide，作者 TheBlind，永久标识 `dev.theblind.quotatide`，统一本地/安装包/版权与独立项目声明；未确定的 GitHub 远程身份拆为发布前绑定任务。
- [设计应用图标与托盘资产](./issues/10-design-app-icon-and-tray-assets.md) — 用户选择 A — Tide Dial；生产资产采用圆形额度仪表与上升潮水，大图标保留七日刻度，小尺寸使用光学校正版，并交付 macOS template 与 Windows color/high-contrast 多层资产。
- [决定本地化与可访问性范围](./issues/11-decide-localization-and-accessibility.md) — v1 完整支持 `zh-CN`/`en`，分离界面语言、格式区域和策略时区，采用提醒语言快照与 WCAG 2.2 AA 门禁，并固定键盘、读屏和辅助显示降级契约。
- [验证最低系统版本与发布 QA 矩阵](./issues/12-verify-minimum-os-and-release-qa.md) — v1 候选最低正式支持 macOS 15 Sequoia universal（Apple Silicon + Intel）与 Windows 11 25H2 x64；macOS 14、Windows 10 22H2 和 Windows 11 24H2 只作扩大兼容 smoke，并以六类证据覆盖安装、平台集成、数据/安全、更新、双语/可访问性与资源预算。
- [搭建 Rust/Tauri 可运行骨架](./issues/14-bootstrap-rust-tauri-workspace.md) — 已建立 Rust 1.88 的 `quotatide-core`、Tauri/Preact 壳、类型化 BuildInfo seam、双平台 Tide Dial 托盘资产、最小权限/CSP、依赖门禁和无发布秘密的三平台 CI bundle smoke。
- [打通 Weekly Ledger 托盘窗口](./issues/15-build-weekly-ledger-tray-shell.md) — 已交付单实例 420×680 托盘弹层、跨显示器物理定位、双平台毛玻璃与不透明降级、完整七日静态状态、设置/键盘交互，以及刷新冷却和系统对话框失焦 seams。
- [完成当前账号配置流程](./issues/16-configure-current-account.md) — 已交付原生单文件选择、严格只读认证验证、版本化 SQLite 设置、稳定账号流 identity、乐观 revision 冲突处理及端到端秘密 canary 边界测试。
- [接入当前账号真实额度](./issues/17-fetch-live-current-quota.md) — 已交付严格 WHAM 当前七日窗口适配、core-owned single-flight/冷却/token rotation、启动与每小时调度、事务化 last-known-good/来源健康，以及 revision 驱动的实时概览状态。

## Not yet specified

- GitHub `owner/repo`、remote 与 updater endpoint 仍等待用户确认；已由
  [绑定 GitHub 仓库与 updater endpoint](./issues/13-bind-github-repository-and-updater.md)
  独立阻断，不能使用 placeholder 发布。

## Out of scope

- Linux、移动端和 Web 托管版本。
- 多账号、成员管理、云同步和远程后台。
- 代理或拦截 Codex 请求、自动停用账号、修改或刷新 `auth.json`。
- 导入当前 Node/Docker 原型的 SQLite 历史数据。
- 默认遥测、云端崩溃收集或账号数据上传。
