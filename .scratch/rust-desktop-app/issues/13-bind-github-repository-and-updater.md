Status: open
Type: wayfinder:task
Parent: ../map.md
Blocked by: ./09-decide-product-identity-and-repository.md, ./26-package-update-and-open-source.md, ./27-run-supported-release-qa.md
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
- 2026-07-28：实施拆票时确认本票是最终外部门禁。只有安装/更新候选与正式
  支持矩阵 QA 完成，并且用户提供 `owner/repo` 后才能领取；不得用 placeholder
  或由代理擅自创建远程仓库。
