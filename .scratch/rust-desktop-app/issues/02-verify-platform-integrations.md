Status: closed
Type: wayfinder:research
Parent: ../map.md
Blocked by: ./01-choose-rust-desktop-stack.md
Assignee: codex

# 验证 macOS 与 Windows 平台集成

## Question

在已选桌面技术栈上，验证托盘图标与锚定小窗口、隐藏 Dock/任务栏、Apple 风格毛玻璃、系统通知、开机启动、文件选择器、macOS Keychain、Windows Credential Manager 的可用实现与降级路径。只依据官方文档和必要的最小验证，在 `docs/research/platform-integrations.md` 记录结论。

## Comments

### 2026-07-28 — Resolution

平台集成采用以下路径：

- Tauri tray + Positioner 负责左击显示、右键菜单与托盘相对定位；Rust 侧再按目标显示器夹取窗口位置。
- macOS 使用 `ActivationPolicy::Accessory` 隐藏 Dock；Windows 使用 `skipTaskbar`。
- macOS 独立分发构建可使用 transparent window + `Popover`/`HudWindow`，但这需要 `macos-private-api`，不能视为 Mac App Store 兼容。若后续选择 App Store，必须提供不启用 private API 的不透明构建。
- Windows 的短暂托盘表面首选 Acrylic，Windows 11 可用 Mica 降级，最终统一降级为不透明语义背景。
- 系统通知、开机启动和文件选择使用 Tauri 官方插件，但只由 Rust adapter 调用，不向 WebView 开放通用插件权限。
- SMTP 密码通过 Rust `keyring` adapter 进入 macOS Keychain 或 Windows Credential Manager；凭证失败时禁用邮件渠道，绝不写明文 fallback。
- 文档只能证明 API 路径存在。毛玻璃效果、多显示器/DPI、通知生命周期和凭证库行为必须由 macOS/Windows 真机原型与安装包 smoke test 验收。

证据、降级矩阵和原型验收项见 [`docs/research/platform-integrations.md`](../../../docs/research/platform-integrations.md)。
