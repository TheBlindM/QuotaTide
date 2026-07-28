Status: open
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./01-choose-rust-desktop-stack.md, ./02-verify-platform-integrations.md, ./03-audit-upstream-data-contracts.md, ./04-prototype-tray-window.md
Assignee: unassigned

# 决定桌面应用架构与模块边界

## Question

依据技术栈、平台能力、上游契约和 UI 原型，决定桌面壳、额度领域核心、策略引擎、采集调度、持久化、通知、SMTP、凭证库与界面状态之间的模块边界和公开 seam。结论应能指导测试策略，并判断哪些行为从 Node 原型移植、重写或放弃。

## Comments
