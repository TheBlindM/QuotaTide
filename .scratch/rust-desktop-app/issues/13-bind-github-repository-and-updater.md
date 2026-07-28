Status: open
Type: wayfinder:task
Parent: ../map.md
Blocked by: ./09-decide-product-identity-and-repository.md
Assignee: unassigned

# 绑定 GitHub 仓库与 updater endpoint

## Question

在用户能够确认公开远程身份后，创建或选择 QuotaTide 的 GitHub
`owner/repo`，添加正确 Git remote，并把 updater endpoint、Cargo/package
metadata、README、SECURITY、about links、release Environment 与 provenance
subject 一次性绑定到同一仓库。启用 Private Vulnerability Reporting，并证明
公开 `/releases/latest/download/latest.json` 地址可达。未完成前 release build
必须因 placeholder gate 失败。

## Comments

- 2026-07-28：用户暂时不能确认 GitHub 仓库，明确要求先继续本地工作。建议
  slug 为 `quota-tide`，但 owner、slug 和远程创建均未获授权，不得擅自执行。
