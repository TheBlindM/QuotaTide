Status: in_progress
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./01-choose-rust-desktop-stack.md, ./02-verify-platform-integrations.md, ./03-audit-upstream-data-contracts.md, ./04-prototype-tray-window.md
Assignee: codex

# 决定桌面应用架构与模块边界

## Question

依据技术栈、平台能力、上游契约和 UI 原型，决定桌面壳、额度领域核心、策略引擎、采集调度、持久化、通知、SMTP、凭证库与界面状态之间的模块边界和公开 seam。结论应能指导测试策略，并判断哪些行为从 Node 原型移植、重写或放弃。

## Comments

- 2026-07-28：开始架构收敛。使用 deep module 词汇决定 external seams、
  internal seams、adapter 和 interface test surface；Rust 核心必须独立于
  Tauri/WebView，可用 fake clock、in-memory upstream、SQLite test database
  与 recording delivery adapters 验证完整刷新事务。
