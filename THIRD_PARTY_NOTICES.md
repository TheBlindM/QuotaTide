# Third-party notices / 第三方声明

QuotaTide is built with open-source software. Exact dependency versions are
locked in `Cargo.lock`, `ui/package-lock.json`, and `package-lock.json`; the
license policy is enforced by `deny.toml` and `cargo deny check`.

QuotaTide 使用开源软件构建。精确依赖版本记录在 `Cargo.lock`、
`ui/package-lock.json` 和 `package-lock.json`；`deny.toml` 与
`cargo deny check` 用于执行许可证策略。

Principal components include:

| Component | Purpose | License |
|---|---|---|
| Rust and Cargo ecosystem crates | Core application and platform adapters | See each crate metadata and `Cargo.lock` |
| Tauri | Desktop runtime and bundling | Apache-2.0 / MIT |
| Preact | User interface | MIT |
| Vite | Frontend build tooling | MIT |
| SQLite / rusqlite bundled SQLite | Local storage | Public domain / MIT wrapper |

This notice does not replace the license text shipped by each dependency.
Binary release preparation must run the dependency/license gate before a draft
is created. Copyright belongs to the respective authors.

本文件不能替代各依赖自身的许可证文本。二进制发布在创建 draft 前必须通过依赖
与许可证门禁；各组件版权归其作者所有。
