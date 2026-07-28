# 最低系统版本一手来源笔记

> 日期：2026-07-28
> 范围：Ticket 12 的 OS/CPU 候选基线；不是实现完成证明。

## 建议

v1 的可辩护候选承诺是：

| 平台 | 产品最低版本候选 | CPU | 最低版本 release smoke |
|---|---|---|---|
| macOS | **macOS 15 Sequoia 及以上** | Apple Silicon + Intel，发布 universal binary | macOS 15 最新补丁各一台 Apple Silicon 与 Intel |
| Windows | **Windows 11 25H2 及以上** | x64 | 25H2 最新累计更新的干净 x64 VM |

macOS 14 Sonoma 仍可作为实现期的扩大覆盖候选，但它是 Apple 当前仍修补的
最老一代，且 Apple 没有承诺其未来支持期限；面向尚未发布的 v1，macOS 15
是更耐久的产品 floor。

Windows 11 24H2 仍可作为实现期的扩大覆盖候选，但 Home/Pro 将于
2026-10-13 停止更新；如果 v1 的支持承诺需要跨过该日期，就不应把 24H2
写成长期最低版本。Microsoft 当前列出的 Home/Pro 25H2 支持截止日期是
2027-10-12。[Windows 11 release information](https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information)

## 四种“最低版本”不能混用

### 1. 编译/打包下限

- Tauri 的 macOS bundle 配置默认 `minimumSystemVersion = "10.13"`，同时写入
  `LSMinimumSystemVersion` 并设置 `MACOSX_DEPLOYMENT_TARGET`；这只是默认编译
  声明，不是产品支持承诺。[Tauri configuration](https://v2.tauri.app/reference/config/#minimumsystemversion)
- Tauri 当前仓库的支持表写的是 macOS 10.15+、Windows 7+，而当前稳定
  Xcode 26 工具链只列出 macOS 11+ deployment target。由当前工具链构建时，
  “Tauri 默认 10.13”因此不能单独证明真实产物仍能在 10.13 启动。
  [Tauri platform table](https://github.com/tauri-apps/tauri#platforms) ·
  [Xcode SDK and system requirements](https://developer.apple.com/xcode/system-requirements/)
- 本项目应显式设置 `bundle.macOS.minimumSystemVersion = "15.0"`，不要依赖
  Tauri 默认值。Windows x64 target 为 `x86_64-pc-windows-msvc`；Tauri/WRY
  能编译或声称可运行在旧 Windows，不代表 Evergreen WebView2 和 OS 本身仍受
  Microsoft 支持。[WRY platform notes](https://github.com/tauri-apps/wry#platform-specific-notes)
- Tauri 官方插件目前统一要求 Rust 1.77.2，但所选 `keyring` 4.1.5 的 Windows
  native store 依赖声明 `rust-version = "1.88"`；所以依赖锁定后项目的实际
  MSRV 至少要通过 `cargo msrv`/CI 重新求交，当前候选应先按 **Rust 1.88+**
  构建。[Tauri plugin support table](https://v2.tauri.app/plugin/#support-table) ·
  [`windows-native-keyring-store` 1.1.0 Cargo.toml](https://docs.rs/crate/windows-native-keyring-store/1.1.0/source/Cargo.toml)

### 2. 厂商仍支持的 OS

- Apple 没有在这些资料中给出固定的 “最近三代”生命周期保证；能确认的是
  2026-07-27 仍同时发布 macOS Tahoe 26.6、Sequoia 15.7.8 和 Sonoma 14.8.8
  安全更新。Sequoia 的兼容机型同时包含 Apple Silicon 和多款 Intel Mac。
  [Apple security releases](https://support.apple.com/en-us/100100) ·
  [macOS Sequoia compatible computers](https://support.apple.com/en-us/120282)
- Windows 10 22H2 的普通支持已于 2025-10-14 结束。Microsoft 会让 Edge 和
  WebView2 Runtime 在 Windows 10 22H2 至少更新到 2028-10，但明确把这种状态
  称为迁移到受支持 OS 的临时桥梁；**WebView2 仍更新不等于 Windows 10 仍受
  支持**。[Windows 10 end of support](https://learn.microsoft.com/en-us/lifecycle/announcements/windows-10-end-of-support) ·
  [Microsoft Edge supported operating systems](https://learn.microsoft.com/en-us/deployedge/microsoft-edge-supported-operating-systems)

### 3. 产品支持承诺

- macOS 15+ 是有当前安全更新、仍覆盖 Intel/Apple Silicon 且不取当前最老
  修补代际的保守交集。这是基于当前发布时点的产品取舍，不是 Apple 生命周期
  保证。每次发布前仍应复查 Apple 安全更新页，不能把本笔记当作永久承诺。
- Windows 11 25H2+ x64 避开已经普通 EOL 的 Windows 10，也避免 v1 发布后很快
  遇到 24H2 Home/Pro EOL。Windows 10 22H2、Windows 11 24H2、LTSC/ESU 可在
  issue 中记录为 best-effort/未承诺，不应混进正式 release gate。

### 4. 实现 smoke gate

只有真实 release bundle 在上述最低版本完成安装、启动和完整平台路径后，
候选才能升级为正式承诺。编译成功、API 文档存在、开发模式运行都不够。

## WebView2 与 Windows 安装假设

- 采用既定 NSIS per-user x64 installer 和 `embedBootstrapper`。Tauri 说明该
  模式约增加 1.8 MB、仍需要网络；缺少 Runtime 时由 installer 运行
  bootstrapper。`offlineInstaller` 约增加 127 MB，fixed runtime 约增加
  180 MB，本项目均不采用。[Tauri Windows installer](https://v2.tauri.app/distribute/windows-installer/#webview2-installation-options)
- Windows 11 通常预装 Evergreen Runtime，但 Microsoft 要求应用仍检查
  Runtime 是否存在；Server/LTSC 或干净系统可能缺失。Evergreen 自动更新并由
  Edge 共享，适合 consumer app。[Microsoft WebView2 runtime guidance](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/webview2#prerequisites)
- 不在没有成品 UI 的情况下杜撰 `minimumWebview2Version`。Tauri 提供该配置，
  用于应用确实依赖新版 WebView2 API 时让 installer 检查并更新。实现时应先
  审计所用 Web API/WRY API，再在 25H2 干净 VM 上记录实际 Runtime、验证
  bootstrapper 失败提示与重试，最后决定是否钉最低 Runtime。
  [Tauri minimum WebView2 version](https://v2.tauri.app/distribute/windows-installer/#minimum-webview2-version)

## 插件与所选依赖的约束

| 能力 | 一手来源能证明什么 | 仍需 smoke 的边界 |
|---|---|---|
| updater | Tauri updater 支持 macOS/Windows，要求 Rust 1.77.2；静态 manifest 的 platform key、URL 与 signature 必须完整。[Tauri updater](https://v2.tauri.app/plugin/updater/) | 从前一真实版本更新 universal macOS app 和 NSIS x64；失败后旧版本仍可启动；两个 macOS arch key 指向同一 universal artifact 的行为 |
| notification | 插件支持 macOS/Windows；Windows 只有 installed app 才能可靠显示正确身份，开发态会显示 PowerShell 名称/图标。[Tauri notifications](https://v2.tauri.app/plugin/notification/) | 允许/拒绝/撤销权限、安装后名称图标、睡眠恢复与去重 |
| autostart | 插件支持 macOS/Windows，macOS 有 `LaunchAgent` 路径。[Tauri autostart](https://v2.tauri.app/plugin/autostart/) | per-user NSIS 安装路径、macOS app 移动/替换、更新后启动项、禁用与卸载清理 |
| dialog | 官方 dialog 插件支持 macOS/Windows 并提供 native open/save dialog。[Tauri dialog](https://v2.tauri.app/plugin/dialog/) | 文件选择期间失焦隐藏 guard、取消、不合法/不可读 `auth.json`、多显示器与本地化 |
| keyring | `keyring` 4.1.5 的 `v1` feature 在 macOS/Windows 使用 native secure store；当前后端是 Apple Keychain 与 Windows Credential Store。[keyring 4.1.5](https://docs.rs/keyring/latest/keyring/) | 首次写/读/删、锁定/拒绝、升级覆盖、卸载残留；文档没有证明本应用的 service/user 命名和回滚流程 |
| SQLite | `tokio-rusqlite` 0.7 每个连接使用一个后台线程，并提供映射到 `rusqlite/bundled` 的 `bundled` feature。[tokio-rusqlite design](https://docs.rs/tokio-rusqlite/latest/tokio_rusqlite/) · [features](https://docs.rs/crate/tokio-rusqlite/latest/source/Cargo.toml.orig) | 应启用 `bundled` 避免系统 SQLite 版本成为隐藏 OS floor；真实验证 WAL、migration、崩溃恢复、损坏提示与备份恢复 |
| SMTP/TLS | `lettre` 0.11.22 的 `tokio1-rustls` 提供 Tokio async TLS；`relay` 是 implicit TLS，`starttls_relay` 要求升级失败即终止且不会发送凭证/邮件。[lettre features](https://docs.rs/lettre/latest/lettre/) · [Async SMTP](https://docs.rs/lettre/latest/lettre/transport/smtp/struct.AsyncSmtpTransport.html) | 锁定 rustls crypto provider 与 root store 后，在两端最低 OS 验证公共 CA、代理/拦截证书、IPv4/IPv6、TLS relay 和 required STARTTLS；禁止 opportunistic/明文模式 |

## 材质与降级

- Tauri 当前效果表把 `Popover` 标为 macOS 10.11+、`HudWindow` 标为 10.14+；
  因此它们不会把建议的 macOS 15 floor 再抬高。透明 WebView/private API、
  Reduce Transparency、Intel GPU 和实际对比度仍只能靠截图与真机验证。
  [Tauri `Effect`](https://docs.rs/tauri/latest/tauri/window/enum.Effect.html)
- Tauri 将 Mica 标为 Windows 11 only、Acrylic 标为 Windows 10/11，并记录部分
  build 在拖动/resize 时的性能问题。Microsoft 把 Acrylic 推荐给 transient
  flyout/light-dismiss surface，而 Mica 更适合长驻基础层。
  [Tauri `Effect`](https://docs.rs/tauri/latest/tauri/window/enum.Effect.html) ·
  [Microsoft Acrylic](https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic) ·
  [Microsoft Mica](https://learn.microsoft.com/en-us/windows/apps/design/style/mica)
- v1 不能把材质成功作为功能前置条件：Windows 先尝试 Acrylic，必要时试
  Mica；macOS 尝试 `Popover`/`HudWindow`；任何 API 失败、系统减少透明度、
  远程桌面或对比度不合格时都立即使用不透明语义背景。

## 当前无法由文档证明

以下项目必须保持为 implementation smoke gate：

1. 同一 universal artifact 的两个 slice 及全部 native dependencies 都以
   macOS 15 为 deployment target，且能在 macOS 15 Intel/Apple Silicon 启动。
2. unsigned preview 的首次安装路径、NSIS WebView2 bootstrapper、卸载、覆盖
   安装和 updater 原子性。
3. tray 定位、失焦隐藏、通知、autostart、dialog、Keychain/Credential Manager
   在最低 OS、安装态、多显示器/DPI、睡眠恢复下的组合行为。
4. Acrylic/Mica/vibrancy 的实际渲染、Reduce Transparency/高对比度 fallback
   和资源占用；API 可用不等于视觉或性能达标。
5. bundled SQLite、rustls/CA roots、SMTP providers、系统代理/防火墙与真实
   上游 HTTPS 在最终 lockfile 和 release profile 下的兼容性。
6. 启动、空闲 CPU/内存、UI bundle、更新下载与磁盘预算。没有成品 release
   build 时不能把架构预算写成已验证事实。
