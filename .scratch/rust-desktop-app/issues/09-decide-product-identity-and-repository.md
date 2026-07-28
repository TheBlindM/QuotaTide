Status: in_progress
Type: wayfinder:grilling
Parent: ../map.md
Blocked by: ./08-decide-distribution-policy.md
Assignee: codex

# 确定产品身份与开源仓库

## Question

决定公开产品名、中英文显示名、GitHub `owner/repo`、bundle identifier、
Windows publisher/display metadata、可执行文件与数据目录名称、版权持有人、
SECURITY 联系方式和 updater endpoint。名称必须适合开源发布，不能暗示这是
OpenAI 官方产品，也不能在首次公开 Release 后随意改变持久身份。

## Comments

- 2026-07-28：开始确认产品显示名与持久技术身份。当前仓库没有 Git remote，
  因此 `owner/repo`、updater endpoint 和公开安全联系方式都需要在发布前
  明确。
- 2026-07-28：用户确认公开产品名为 `QuotaTide`。中文界面使用“Codex
  额度助手”作为描述性副标题，并在 README/关于页明确它是独立社区项目，
  不隶属于、不受 OpenAI 背书。初步公开检索未发现明显同类产品撞名，但这
  不是商标法律审查。
- 2026-07-28：公开 GitHub `owner/repo` 暂时不能确认，用户要求继续其他
  身份决策。不得创建远程仓库或把 `benteli/quota-tide` 当成已决定地址；
  updater endpoint 保持 build-time placeholder，首次公开 Release 前必须
  回填并锁定。
