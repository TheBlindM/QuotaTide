# Rust 跨平台桌面技术栈研究

研究日期：2026-07-28  
范围：面向 macOS 与 Windows 的单账号 Codex 额度托盘应用。核心能力必须使用 Rust；界面可以使用 WebView。本文只采用项目官方文档、官方仓库和官方 API 文档。

## 结论

**v1 采用 Tauri 2，界面采用本地打包的轻量 Web 前端；Slint 保留为纯 Rust UI 备选；iced 与 egui 不进入最终候选。**

推荐的职责边界：

- Rust 拥有 `auth.json` 只读解析、额度采集、七日策略模板、结转额度、今日实际上限、SQLite、调度、提醒事件去重、SMTP、系统凭证库适配和系统通知。
- WebView 只负责展示与收集配置，通过窄而有类型的 Tauri commands/events 与 Rust 交互；不直接读取 Token、数据库或系统凭证。
- UI 资产全部随应用打包，不加载远程脚本；配置严格 CSP 和最小 capabilities。
- 托盘、窗口显示/隐藏、托盘相对定位、通知、原生文件选择器、开机启动和更新使用 Tauri 2 的官方能力；系统凭证库仍应由独立 Rust adapter 统一 macOS Keychain 与 Windows Credential Manager，而不是把 SMTP 密码交给 WebView。

这不是因为 Tauri 的界面层“更 Rust”，而是因为本产品最难、风险最高的部分是 **跨平台托盘小窗口、原生毛玻璃、通知、文件选择、安装与更新**。Tauri 已为这些需求提供同一生态中的官方 API；纯 Rust UI 方案需要额外拼装窗口、托盘、定位和平台材质。

## 决策矩阵

| 维度 | Tauri 2 | Slint | iced | egui/eframe |
| --- | --- | --- | --- | --- |
| Rust 核心占比 | 业务和系统层可全部为 Rust；表现层为 HTML/CSS/TS | 业务层 Rust，`.slint` DSL 编译为 Rust/native code | 应用与 UI 均为 Rust | 应用与 UI 均为 Rust |
| 渲染 | macOS WKWebView、Windows WebView2；不随包捆绑浏览器运行时 | winit + FemtoVG/Skia/software 等自绘渲染器 | wgpu 或 tiny-skia | wgpu 或 glow，经 winit/eframe |
| 托盘小窗口 | 一等公民：tray API、点击坐标、显示/聚焦窗口、官方 Positioner | 1.17 已提供原生 `SystemTrayIcon`；没有等价的 tray-relative positioner，且 macOS 有菜单时不触发左击 callback | 官方功能表未提供托盘；需另接 | eframe 官方功能不含托盘；需另接 |
| 毛玻璃界面 | 窗口 effects 直接提供 macOS 材质及 Windows Mica/Acrylic，CSS 负责内容层次 | 可取得 raw window handle，但毛玻璃需要平台代码/第三方 crate；官方讨论中的做法也是 native hack | 需要 raw-window-handle + 平台实现 | 需要 winit/raw handle + 平台实现；且官方明确“不追求 native-looking” |
| 系统集成 | 官方 notification、dialog、autostart、positioner、SQL、store、updater 等插件 | 主要是 UI toolkit，系统集成需自行选 crate 和维护 adapters | 同左 | 同左 |
| 测试 | Rust 单测；mock runtime 集成测试；前端浏览器测试；WebDriverIO 跨 macOS/Windows E2E | Rust 单测；`system-testing` 可远程检查和控制 UI，但托盘/定位等系统壳仍需自建双平台 smoke 测试 | `iced_test` 提供 headless interaction 与 snapshot，但项目自称 experimental | UI 函数可测试，但官方框架没有与 Tauri 同等级的托盘/系统集成 E2E 路线 |
| 包体/资源依据 | 使用系统 WebView，官方提供 release profile 和移除未使用 commands 的缩减手段 | 渲染器随应用；官方称 Skia 相比其他 renderer 磁盘占用重，software 轻量 | 自带 wgpu/tiny-skia 渲染路径 | eframe 官方说明依赖较多，含 winit 与图形栈 |
| 开源许可 | MIT 或 Apache-2.0，与目标 MIT 项目直接兼容 | 可用 royalty-free desktop license，但要求 Slint attribution；若选 GPLv3，分发的完整二进制须 GPLv3 | MIT | MIT 或 Apache-2.0 |
| 主要风险 | 两端 WebView 行为差异；IPC/CSP 权限面；透明窗口在 macOS 的 App Store/private API 边界需原型确认 | 托盘能力刚在 1.17 加入；定位、材质、通知、更新等仍需拼装；许可归属和 attribution；桌面能力仍在完善 | 官方标注 experimental；缺少本产品所需桌面壳能力 | 官方明确界面不追求原生外观、API 仍会 breaking，和“Apple 风格毛玻璃”目标相冲 |
| 决策 | **推荐** | **备选** | 淘汰 | 淘汰 |

## Tauri 2 评估

### Rust 核心与 WebView 边界

Tauri 官方架构把应用描述为 Rust 工具加 WebView 中的 HTML；WebView 通过消息传递调用 Rust API。它直接使用 TAO 管理窗口、WRY 抽象 WebView，并使用操作系统 WebView而不是捆绑浏览器运行时。[Tauri Architecture](https://v2.tauri.app/concept/architecture/)

在本项目中，这允许“核心全部 Rust、界面只做表现”：

1. Rust scheduler 每小时读取当前 `auth.json`，完成网络、持久化、策略和提醒事件。
2. UI 只调用 query/update commands，不持有访问令牌或 SMTP 密码。
3. commands 输入输出使用 serializable DTO；Tauri 的 runtime authority 可以按 window、command、scope 限制权限。[Runtime Authority](https://v2.tauri.app/security/runtime-authority/)
4. 前端不访问互联网；Codex、Codex Resets、SMTP 和 updater 网络请求都由 Rust 发起。官方建议不从 CDN 加载远程脚本，并提供 CSP 来限制 WebView 资源。[Content Security Policy](https://v2.tauri.app/security/csp/)

代价是前端仍有 HTML/CSS/TypeScript，不是“纯 Rust UI”。这是已确认的产品边界，而非缺陷。

### 渲染与维护风险

Tauri 在 Windows 使用基于 Chromium 的 WebView2，在 macOS 使用 WKWebView；它不捆绑 WebView，因此实际 Web 能力受系统 provider/version 影响。[Webview Versions](https://v2.tauri.app/reference/webview-versions/)

这带来两个明确维护要求：

- UI 只使用 macOS 与目标 Windows 版本共同支持的稳定 Web 标准，并在两个平台做视觉回归。
- 不把每小时调度放进隐藏 WebView。Tauri 文档说明浏览器可能 throttle 或卸载隐藏 view；调度必须在 Rust runtime 中运行。[Webview API](https://v2.tauri.app/reference/javascript/api/namespacewebview/)

### 托盘小窗口

Tauri 2 的官方 tray API 支持 Rust 侧创建托盘、接收 click/double-click 等事件，并且事件包含托盘图标的位置和矩形；官方示例直接在左击时显示、聚焦既有窗口。[System Tray](https://v2.tauri.app/learn/system-tray/)

官方 Positioner 插件支持 macOS 与 Windows，并专门说明了 `tray-icon` feature 与 `on_tray_event` 集成，用于托盘相对定位。[Positioner](https://v2.tauri.app/plugin/positioner/)

因此 v1 的窗口壳可以是：

- 启动时 `visible: false`、固定尺寸、不可 resize、无传统主窗口。
- 托盘左击：按 tray rect 计算/修正位置，show + focus。
- focus lost：隐藏而非退出。
- macOS 设 accessory activation policy；Windows 设 skip taskbar。具体 API/边角行为必须在后续双平台原型 ticket 中验证。

### Apple 风格毛玻璃

Tauri 的 window effects API公开了 macOS 的 `Popover`、`Menu`、`HudWindow`、`Sidebar` 等材质，以及 Windows 的 `Mica`、`MicaDark/Light`、`Acrylic` 和 `Blur`；`set_effects` 要求透明窗口，Linux 不支持。[Effect API](https://docs.rs/tauri/latest/tauri/window/enum.Effect.html) [Window::set_effects](https://docs.rs/tauri/latest/tauri/window/struct.Window.html#method.set_effects)

推荐映射不是强行让 Windows 模仿 macOS：

- macOS：优先 `Popover` 或 `HudWindow`，使用系统材质、圆角与浅色/深色适配。
- Windows 11：优先 `Mica`；Windows 10 兼容路径评估 `Acrylic`/`Blur`，失败时降级为不透明背景。
- CSS 的 `backdrop-filter` 只负责 Web 内容层，不作为原生窗口毛玻璃的唯一实现。

需要警惕：Tauri Webview API 明确指出 macOS 的 WebView transparency 需要 `macos-private-api`，启用会妨碍进入 App Store。[Webview API](https://v2.tauri.app/reference/javascript/api/namespacewebview/) 因此必须在后续原型中验证能否只通过 window effects 达到目标；若必须启用该私有 API，应把“独立签名分发”和“Mac App Store”视为不同发布目标，不能在这里假定二者兼容。

### 插件与系统集成

Tauri 官方生态直接覆盖本项目的大部分壳层能力：

- 原生通知支持 macOS 与 Windows；Windows 开发态有限制且正式通知只对已安装应用生效。[Notifications](https://v2.tauri.app/plugin/notification/)
- 原生文件选择器在 macOS 与 Windows 返回文件系统路径，适合选择 `auth.json`。[Dialog](https://v2.tauri.app/plugin/dialog/)
- 开机启动官方支持 macOS 与 Windows。[Autostart](https://v2.tauri.app/plugin/autostart/)
- 官方还提供 SQL、Store、Stronghold、Updater 等插件入口。[Tauri Architecture — Plugins](https://v2.tauri.app/concept/architecture/#plugins)

但不应因为“有插件”就把所有领域逻辑塞进 plugin guest bindings。SQL、SMTP、系统凭证库和网络 adapter 应从 Rust 内部调用；WebView 不获得通用文件系统、shell 或 HTTP 权限。

### 包体与资源

官方依据只足以得出相对结论：Tauri 复用系统 WebView、不捆绑浏览器 runtime；官方提供 LTO、`opt-level = "s"`、strip、`panic = "abort"`，以及移除 capability 中未使用 commands 的构建选项。[App Size](https://v2.tauri.app/concept/size/)

本文不引用营销页的最小 demo 数字，也不预测本产品安装包大小。真实预算必须由同一功能集的 macOS universal app 与 Windows installer release artifacts 实测；邮件、SQLite、更新、图标和签名都会改变结果。

### 测试能力

Tauri 官方支持：

- Rust 核心正常使用 `cargo test`。
- mock runtime 做单元/集成测试，且不启动 native WebView。
- WebDriverIO Tauri service 在 Windows、Linux 和 macOS 工作；其 embedded provider 是 macOS E2E 的官方推荐路径。
- 前端可在纯浏览器模式 mock `invoke()`，快速测表现层。[Tests](https://v2.tauri.app/develop/tests/) [WebDriver](https://v2.tauri.app/develop/tests/webdriver/)

建议测试金字塔：

1. 大量 Rust domain tests：额度窗口、七日策略模板、结转、提醒事件去重、时间边界。
2. adapter contract tests：auth JSON、upstream JSON、SQLite、SMTP。
3. 少量 frontend component tests：显示状态与配置验证。
4. macOS + Windows 的 smoke E2E：启动、托盘点击、窗口定位、选择文件、系统通知权限。
5. 原生毛玻璃与托盘位置仍需人工/截图 QA；WebDriver 不能证明系统材质正确。

## Slint 评估

Slint 用 `.slint` 声明式 DSL 描述 UI，编译器可生成 Rust，业务逻辑可完全使用 Rust。默认桌面路径基于 winit；官方支持 FemtoVG、Skia 与 software 等 renderer，macOS/Windows 都在 winit backend 支持范围。[Slint Rust API](https://docs.slint.dev/latest/docs/rust/slint/) [Winit Backend](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backend_winit/)

它作为备选的优点：

- 不依赖系统 WebView，渲染更一致。
- UI/业务都能留在 Rust 工具链附近；`.slint` 会 ahead-of-time 编译。
- Slint 1.17 已提供 `SystemTrayIcon`，在 macOS 使用 `NSStatusItem`、在 Windows 使用 `Shell_NotifyIcon`，因此无需再引入独立 tray crate。[SystemTrayIcon](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/) [Slint 1.17 Release](https://slint.dev/blog/slint-1.17-released)
- renderer 可选，包含 GPU 与 software 路径；但官方也明确称 Skia 相对其他 renderer 有较重磁盘占用，因此不能泛称 Slint 一定更轻。[Backends & Renderers](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/)
- `WindowHandle` 可以暴露 raw AppKit/Win32 handles，理论上能接平台材质。[WindowHandle](https://docs.slint.dev/latest/docs/rust/slint/struct.WindowHandle)
- `system-testing` feature 可让测试工具远程检查和控制 UI；它是开发/测试能力，不建议进入生产构建。[Slint Cargo Features](https://docs.slint.dev/latest/docs/rust/slint/docs/cargo_features/)

没有成为首选的原因：

- `SystemTrayIcon` 是 2026 年 6 月随 1.17 新增的桌面能力，维护历史短于 Tauri tray。它没有等价的 tray-relative positioner；而且在 macOS 附带菜单时，左击会打开菜单而不触发 `clicked`，需要为“左击弹出窗口、右击打开菜单”的目标另做交互取舍。[SystemTrayIcon](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/)
- 官方公开窗口 API提供 always-on-top、hide 等常规能力，但未提供跨平台毛玻璃抽象。[Slint Window](https://docs.slint.dev/latest/docs/slint/reference/window/window/)
- 官方社区中 macOS blur 示例本身需要 Cocoa/Objective-C 和 raw window handles，讨论还指出 Windows 需要不同 workaround。这是“可以做”，不是“Slint 提供跨平台能力”。[Slint discussion #5710](https://github.com/slint-ui/slint/discussions/5710)
- 通知、开机启动、文件选择、更新、凭证库、tray-relative positioning 和 installer 工作流都需另选 crates 并自行组合 event loops/adapters，增加 v1 风险。
- Slint 1.17 的官方发布说明仍把新增托盘等能力表述为迈向 “desktop-ready” 的阶段性进展，说明桌面通用能力仍在持续补齐。[Slint 1.17 Release](https://slint.dev/blog/slint-1.17-released)
- 许可需要显式选择：MIT 源码项目可以使用 royalty-free desktop license，但二进制/网站需按条款展示 Slint attribution；若使用 GPLv3 路径，完整分发二进制须 GPLv3。[Slint License Terms](https://slint.dev/terms-and-conditions)

因此 Slint 是在以下条件下的合理 fallback：Tauri 的双平台毛玻璃原型无法接受、WebView 兼容性造成实质 UI 阻碍，且团队愿意承担托盘/窗口/系统集成和 attribution 的额外维护。

## iced 与 egui/eframe 的淘汰判断

### iced

iced 的官方仓库列出跨平台 Rust UI、async、wgpu/tiny-skia renderer，并提供 `iced_test` 进行 headless interaction 与 snapshot，技术上有吸引力。[iced repository](https://github.com/iced-rs/iced) [iced_test](https://docs.iced.rs/iced_test/)

但官方同时把 iced 标为 **experimental software**。其公开特性集中在 UI/runtime/renderer，没有本产品最关键的 tray、tray-relative popover、原生毛玻璃、通知、更新和发布壳。采用 iced 意味着我们仍需自己组装这些平台能力，却没有 Slint 的稳定 1.x 声明。故不进入 v1 最终候选。

### egui/eframe

egui 是纯 Rust immediate-mode UI；eframe 支持 macOS 与 Windows并使用 winit + wgpu/glow。它适合工具、调试器和游戏引擎 UI。[egui repository](https://github.com/emilk/egui)

官方 README 直接说明：

- “Native looking interface” 是 non-goal；
- 接口仍在变化，升级会有 breaking changes；
- eframe 依赖较多，包括 winit、图形和 clipboard crates；
- immediate mode 每帧重新布局，复杂 UI 需自行注意 CPU/layout trade-off。

这些特性与“Apple 风格毛玻璃的最终用户托盘应用”目标正面冲突。即使平台材质可以通过新暴露的 winit window access 自行实现，托盘/定位/通知/更新仍需拼装，因此淘汰。

## 需要后续 ticket 验证的假设

技术栈决策已足以继续，但以下不是文档研究可以证明的事实：

1. 在不启用 macOS private WebView API 的情况下，Tauri window effects + 本地 Web UI 是否能达到可接受的原生毛玻璃。
2. macOS 刘海、多显示器、缩放，以及 Windows 多显示器/DPI 下的 tray-relative positioning。
3. 失焦隐藏、右键菜单、首次通知授权、Windows installed-app notification 的完整 lifecycle。
4. 目标 Windows 最低版本对应的 Mica/Acrylic 降级矩阵。
5. release 构建的真实常驻内存、冷启动时间、CPU idle 和安装包大小。

这些应由托盘窗口原型与平台能力 ticket 在真实 macOS/Windows 上给出可运行证据，不应在 spec 中写成已保证。

## 最终推荐

采用：

- **Tauri 2**
- **Rust domain/application/adapters**
- **本地打包的轻量 Web UI**（框架在 UI 原型后决定，避免先引入重型 SPA）
- **官方 tray + positioner + notification + dialog + autostart + updater**
- **Rust-owned scheduler、SQLite、SMTP 和凭证库 adapters**

保留：

- **Slint** 作为 Tauri 毛玻璃/WebView 原型失败时的唯一纯 Rust UI fallback。

淘汰：

- **iced**：实验状态与桌面壳缺口使 v1 集成风险过高。
- **egui/eframe**：官方明确不追求原生外观，和产品视觉目标不匹配。
