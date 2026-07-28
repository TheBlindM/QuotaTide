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

## Not yet specified

- UI 原型确认后才能明确窗口尺寸、信息密度、导航层级、深浅色与无障碍细节。
- Tauri 2 已确认；仍需结合平台能力研究与 UI 原型明确模块边界、异步运行模型、资源预算、前端框架与数据库方案。
- 发布链路研究后才能明确签名、公证、安装包、自动更新和贡献者发布流程。
- 上述决策完成后，需要判断品牌命名、应用图标、本地化与 QA 矩阵是否仍阻碍 build-ready spec。

## Out of scope

- Linux、移动端和 Web 托管版本。
- 多账号、成员管理、云同步和远程后台。
- 代理或拦截 Codex 请求、自动停用账号、修改或刷新 `auth.json`。
- 导入当前 Node/Docker 原型的 SQLite 历史数据。
- 默认遥测、云端崩溃收集或账号数据上传。
