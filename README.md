# QuotaTide

QuotaTide 是一个面向 macOS 与 Windows 的独立开源托盘应用，用于在本机监控
单个 Codex 账号的当前七日额度窗口。

作者：TheBlind

License：MIT
应用标识：`dev.theblind.quotatide`

## 当前状态

项目正在从旧 Node/Docker 原型重构为 Rust/Tauri 桌面应用。目前只完成
**Ticket 14 的应用骨架**：

- Rust workspace 与框架无关的 `quota-core`；
- Tauri 2 隐藏窗口和单一 Tide Dial 托盘入口；
- Vite + Preact + TypeScript UI；
- 类型化的公开构建信息 command；
- macOS/Windows bundle CI 骨架。

当前版本尚未读取 `auth.json`、请求真实额度、保存 SQLite 账本、发送通知或
邮件，也没有完成发布 smoke。旧 Node 服务仍暂时保留供后续行为迁移测试使用，
但新桌面应用不启动它、不读取它的数据库，也不依赖 Docker。

## 开发要求

- Rust 1.88.0（由 `rust-toolchain.toml` 固定）
- Node.js 22.13 或更高
- macOS：Xcode Command Line Tools
- Windows：Microsoft C++ Build Tools 与 WebView2
- Tauri CLI 2.11.4
- cargo-deny 0.20.2

安装工具与前端依赖：

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo install tauri-cli --version 2.11.4 --locked
cargo install cargo-deny --version 0.20.2 --locked
npm --prefix ui ci
```

运行测试和检查：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny check
npm --prefix ui run check
node scripts/check-desktop-versions.mjs
git diff --exit-code -- ui/src/bindings
```

生成 Tide Dial 平台图标并启动桌面开发模式：

```bash
npm run icons
cargo tauri dev
```

## 产品规范

- [实施规范](.scratch/rust-desktop-app/spec.md)
- [最低系统版本与发布 QA](docs/research/minimum-os-and-release-qa.md)
- [架构决策](docs/research/application-architecture.md)
- [本地安全模型](docs/research/config-state-security.md)

QuotaTide 与 OpenAI 没有官方关系。Codex 和 OpenAI 是其各自所有者的商标。
