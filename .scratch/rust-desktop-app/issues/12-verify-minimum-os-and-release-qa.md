Status: closed
Type: wayfinder:research
Parent: ../map.md
Blocked by: ./02-verify-platform-integrations.md, ./04-prototype-tray-window.md, ./07-decide-config-state-security.md, ./08-decide-distribution-policy.md, ./09-decide-product-identity-and-repository.md, ./10-design-app-icon-and-tray-assets.md, ./11-decide-localization-and-accessibility.md
Assignee: codex

# 验证最低系统版本与发布 QA 矩阵

## Question

对照 Tauri、WebView2、macOS/Windows 平台与所选依赖的一手资料，确定 v1
暂定最低 macOS/Windows 版本和发布 QA 矩阵，并明确哪些结论必须在实现阶段
用真实构建 smoke 才能最终确认。矩阵必须覆盖
CPU、安装/卸载、托盘/窗口、毛玻璃降级、通知、开机启动、文件选择、凭证库、
SMTP、SQLite 恢复、更新、语言/可访问性和资源预算，形成 build-ready release
gate 文档。

## Comments

- 2026-07-28：开始核对最低系统版本与 build-ready release gate。先锁定候选
  OS/CPU 基线与一手来源，再把既有产品、架构、安全、发布、本地化决策展开为
  可执行的双平台 QA 矩阵；未有真实 Rust/Tauri 构建证据的项目必须明确标记为
  implementation smoke gate，不能伪装成已验证。
- 2026-07-28：研究完成。v1 正式候选 floor 为 macOS 15 Sequoia universal
  （Apple Silicon + Intel）与 Windows 11 25H2 x64；macOS 14、Windows 10
  22H2 和 Windows 11 24H2 只做不承诺的扩大兼容 smoke。发布门禁已按
  `AUTO/BUILD/SMOKE/MANUAL/LIVE/SECURITY` 展开 CPU、安装、平台集成、数据、
  SMTP、更新、本地化/可访问性与资源预算，并明确全部真实构建证据仍待实施。
  结论见 `docs/research/minimum-os-and-release-qa.md`，一手来源见
  `docs/research/minimum-os-source-notes.md`。
