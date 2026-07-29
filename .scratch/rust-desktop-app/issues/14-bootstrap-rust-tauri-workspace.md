# 14 — 搭建 Rust/Tauri 可运行骨架

**What to build:** 建立可持续开发的 QuotaTide 桌面应用骨架，让同一份代码在
macOS 与 Windows 上构建、启动、显示托盘占位入口，并用一个类型化 command
证明 Rust 核心、Tauri 壳和 Preact 界面已经贯通。

**Blocked by:** None — can start immediately

**Status:** closed

- [x] 建立 Rust workspace、框架无关的 quota core、Tauri 组合层和
  Vite/Preact/TypeScript UI，依赖方向符合已确认架构。
- [x] 应用名称、作者、identifier、版本来源、MIT License 和 Tide Dial
  应用/托盘资产使用已确认的 QuotaTide 身份。
- [x] 应用启动后只创建一个隐藏窗口和一个托盘入口，不启动 HTTP server，
  不依赖 Docker，也不读取旧 Node 运行数据。
- [x] 一个只返回无秘密构建信息的 Rust command 能从 UI 调用，Rust DTO 可生成
  TypeScript 类型且 CI 能检测合约漂移。
- [x] Tauri capability 与 CSP 使用最小 allowlist，UI 不获得通用 filesystem、
  shell、HTTP、SQL、notification、updater 或系统凭证权限。
- [x] Rust format、clippy、测试、前端 lint、类型检查、测试和 production build
  均可在本地通过。
- [x] CI 至少覆盖 Linux 的纯核心/UI 检查、macOS bundle 构建和 Windows x64
  bundle 构建；fork/PR job 不持有发布秘密。
- [x] 固定 Rust toolchain 与直接依赖版本，启用依赖许可检查并记录当前 MSRV。
- [x] 添加最小开发说明，明确当前只是骨架、未完成真实额度监控或发布 smoke。

## Comments

- Resolution: commits `c0c414c` and `c9e3fea` establish the Rust 1.88 workspace,
  `quotatide-core`, the Tauri/Preact shell, typed `BuildInfo` IPC contract,
  platform-specific Tide Dial tray assets, minimum capability/CSP, CI bundle
  smoke, MIT identity, dependency policy, and release-identity guard.
- Verification: Rust fmt/clippy/tests, UI lint/typecheck/test/build, version and
  identity checks, offline `cargo deny`, native macOS application bundle build,
  bundle metadata inspection, and hidden tray-process launch smoke all passed.
- Review: Standards found three identity deviations and Spec found two asset/
  copyright deviations; all were fixed and both targeted re-reviews passed.
