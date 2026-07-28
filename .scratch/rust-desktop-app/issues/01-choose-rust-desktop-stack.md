Status: closed
Type: wayfinder:research
Parent: ../map.md
Blocked by: none
Assignee: codex

# 选择 Rust 跨平台桌面技术栈

## Question

基于官方一手资料比较 Tauri 2 与仍具现实可行性的纯 Rust UI 方案，决定哪个技术栈最适合同时交付 macOS 与 Windows 托盘应用。结论必须覆盖 Rust 核心占比、WebView/渲染能力、托盘窗口支持、包体与资源、插件成熟度、测试能力、维护风险，并在 `docs/research/desktop-stack.md` 留下带引用的研究资产和明确推荐。

## Comments

### 2026-07-28 — Resolution

选择 **Tauri 2 + Rust-owned core + 本地轻量 Web UI**：

- `auth.json`、额度采集、策略、SQLite、调度、提醒、SMTP、凭证库和通知由 Rust 持有。
- WebView 只负责展示和配置，通过最小化、强类型的 commands/events 与 Rust 通信。
- 使用 Tauri 官方 tray、positioner、notification、dialog、autostart 和 updater 能力；具体平台行为由后续原型验证。
- Slint 1.17 保留为唯一纯 Rust UI fallback。它已有 macOS/Windows 系统托盘，但 tray-relative positioning、原生毛玻璃和多项系统集成仍需自行拼装，且托盘能力较新。
- iced 和 egui/eframe 不进入 v1 候选。

完整证据、矩阵、风险与后续待验证假设见 [`docs/research/desktop-stack.md`](../../../docs/research/desktop-stack.md)。
