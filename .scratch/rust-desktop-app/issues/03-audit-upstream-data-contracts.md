Status: open
Type: wayfinder:research
Parent: ../map.md
Blocked by: none
Assignee: unassigned

# 审核 Codex 与重置雷达数据契约

## Question

结合当前 Node 原型、已有研究与可复查的一手来源，形成 Rust 适配器所需的最小稳定契约：`auth.json` 结构与只读策略、Codex 周额度窗口、额度 epoch、Codex Resets `watch` 预测、错误与过期语义、轮询和手动刷新边界。把缺少官方保证的部分明确标为非公开/第三方依赖，并在 `docs/research/upstream-contracts.md` 记录结论。

## Comments
