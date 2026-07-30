# QuotaTide

QuotaTide is an independent, open-source macOS and Windows tray app that
monitors the active seven-day Codex quota window for one local account.

QuotaTide 是一款独立开源的 macOS / Windows 托盘应用，用于在本机监控单个
Codex 账号当前额度周期中的七个自然日。

Author / 作者：TheBlind

License / 许可证：[MIT](LICENSE)

Application ID / 应用标识：`dev.theblind.quotatide`

> QuotaTide is not affiliated with or endorsed by OpenAI. Codex and OpenAI are
> trademarks of their respective owners.
>
> QuotaTide 与 OpenAI 没有官方关系，也未获得其背书。Codex 和 OpenAI 是其
> 各自所有者的商标。

## Features / 功能

- Read-only access to a user-selected `auth.json`; QuotaTide never rewrites,
  copies, or displays its tokens.
- Current-account quota, reset time, and exactly the seven dates in the active
  quota window—not a rolling “last seven days” chart.
- Editable daily limits, policy timezone, and dynamic weekday carry-forward.
- Hourly background refresh, reset-radar estimate, native notifications, and
  optional TLS SMTP alerts.
- Local SQLite history, encrypted OS credential storage, diagnostics export,
  recovery mode, and local-data removal.
- System/interface language selection for Simplified Chinese and English.
- Explicit, signature-verified updates with a 60-second first check, daily
  checks thereafter, manual checking, and an automatic-check toggle.

- 只读访问用户选择的 `auth.json`；不改写、不复制、也不显示其中的令牌。
- 展示当前账号的额度、重置时间，以及当前额度周期内的七个日期，而不是滚动
  的“最近七天”。
- 可编辑每日额度与策略时区，支持工作日未用额度动态结转。
- 每小时后台刷新、重置雷达预测、原生通知，以及可选的 TLS SMTP 邮件告警。
- 本地 SQLite 历史、系统凭证库、诊断导出、恢复模式和本地数据清理。
- 支持简体中文、英文及跟随系统。
- 更新必须由用户确认并通过 Tauri 签名验证；首次启动 60 秒后检查，之后每
  24 小时检查，也可手动检查或关闭自动检查。

## Preview distribution / 预览版分发

The `0.x` direct-download builds are unsigned previews:

- macOS: one universal DMG for Apple Silicon and Intel, minimum macOS 15.0.
- Windows: one x64 current-user NSIS installer for Windows 11 25H2+, using the
  Evergreen WebView2 bootstrapper.
- No MSI, per-machine installer, fixed WebView2 runtime, telemetry service, or
  browser/server edition is distributed.

`0.x` 直接下载版本属于未签名预览版：

- macOS：同一个 universal DMG 支持 Apple Silicon 与 Intel，最低 macOS 15.0。
- Windows：Windows 11 25H2+ x64 当前用户 NSIS 安装包，使用 Evergreen
  WebView2 bootstrapper。
- 不提供 MSI、全机器安装、fixed WebView2 runtime、遥测服务或浏览器/服务器版。

Gatekeeper or SmartScreen can warn because this project currently has no
Developer ID or Authenticode publisher certificate. Do not globally disable
operating-system protections. Follow the scoped steps and verify SHA-256 in:

由于项目目前没有 Developer ID 或 Authenticode 发布者证书，Gatekeeper 或
SmartScreen 可能提示风险。请勿全局关闭系统保护；请按文档进行单应用确认并
校验 SHA-256：

- [Install, update, and uninstall (English)](docs/en/install-update-uninstall.md)
- [安装、更新与卸载（简体中文）](docs/zh-CN/install-update-uninstall.md)
- [Release verification (English)](docs/en/release-verification.md)
- [发布校验（简体中文）](docs/zh-CN/release-verification.md)

Production release generation is intentionally blocked until the public GitHub
repository and final updater public key are bound. The committed key is a
development-only key whose private half was destroyed.

在公开 GitHub 仓库与最终 updater 公钥确认前，生产发布门禁会按设计失败。仓库
当前提交的仅是开发公钥，其私钥已销毁，不能用于正式发布。

## Privacy and security / 隐私与安全

- [Privacy (English)](docs/en/privacy.md)
- [隐私说明（简体中文）](docs/zh-CN/privacy.md)
- [Security policy / 安全策略](SECURITY.md)
- [Third-party notices / 第三方声明](THIRD_PARTY_NOTICES.md)

## Development / 开发

Requirements: Rust 1.88.0, Node.js 22.13+, Xcode Command Line Tools on macOS,
Microsoft C++ Build Tools and WebView2 on Windows, Tauri CLI 2.11.4, and
cargo-deny 0.20.2.

```bash
npm --prefix ui ci
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
npm --prefix ui run check
npm run check
git diff --exit-code -- ui/src/bindings
```

Run the desktop app:

```bash
cargo tauri dev
```

The old Node server, Docker image, browser page, plaintext SMTP environment
path, and legacy database have been removed. Node remains only as the Tauri
frontend and deterministic release-tool runtime.

旧 Node 服务、Docker 镜像、浏览器页面、明文 SMTP 环境变量入口和旧数据库均
已移除。Node 仅用于 Tauri 前端和确定性的发布工具。

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and release-boundary
rules.
