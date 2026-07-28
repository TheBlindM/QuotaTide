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

## Not yet specified

- 仍需明确非秘密配置与事实表的精确 schema、凭证库 key、诊断导出、恢复和保留策略。
- 仍需在发行策略票中正式确认支持矩阵、Windows 签名供应商、安装包、自动更新和贡献者发布权限。
- 上述配置与发行策略完成后，需要判断品牌命名、应用图标、本地化与 QA 矩阵是否仍阻碍 build-ready spec。

## Out of scope

- Linux、移动端和 Web 托管版本。
- 多账号、成员管理、云同步和远程后台。
- 代理或拦截 Codex 请求、自动停用账号、修改或刷新 `auth.json`。
- 导入当前 Node/Docker 原型的 SQLite 历史数据。
- 默认遥测、云端崩溃收集或账号数据上传。
