# Contributing / 参与贡献

## English

1. Open an issue describing the user-visible problem and supported platform.
2. Keep the core domain logic in `crates/quotatide-core`; keep Tauri and
   operating-system adapters at the edge.
3. Never add real `auth.json`, tokens, email credentials, local databases,
   diagnostics, updater private keys, or signing passwords.
4. Add focused Rust or UI tests and run the commands in `README.md`.
5. Preserve Simplified Chinese and English copy, keyboard operation, 200% zoom,
   reduced-motion support, and high-contrast behavior.
6. Pull requests and forks never receive release secrets. Changes to release
   workflows, updater keys, identity, or lockfiles require CODEOWNERS review.

The project accepts MIT-licensed contributions under the repository license.
The author identity is TheBlind; contributors retain attribution through Git
history.

## 简体中文

1. 先创建 issue，说明用户可见的问题和受影响的支持平台。
2. 核心领域逻辑放在 `crates/quotatide-core`，Tauri 与操作系统适配器留在边界。
3. 禁止提交真实 `auth.json`、令牌、邮件凭证、本地数据库、诊断包、updater
   私钥或签名密码。
4. 补充聚焦的 Rust 或界面测试，并运行 `README.md` 中的检查命令。
5. 保持简体中文/英文、键盘操作、200% 缩放、减少动画和高对比度可用。
6. pull request 与 fork 永远不能读取发布秘密；发布工作流、updater key、
   项目标识或 lockfile 的变更需要 CODEOWNERS 审核。

贡献内容按仓库 MIT License 提交。项目作者署名为 TheBlind；贡献者通过 Git
历史保留署名。
