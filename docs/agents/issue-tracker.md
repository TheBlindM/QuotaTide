# Issue tracker: Local Markdown

这个 repo 的 issues 和 specs（spec 也常称为 PRD）作为 Markdown 文件存放在 `.scratch/` 中。

## Conventions

- 每个 feature 一个目录：`.scratch/<feature-slug>/`
- Spec 是 `.scratch/<feature-slug>/spec.md`
- Implementation issues 每个 ticket 一个文件，路径为 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`，从 `01` 开始编号；绝不要写成一个 combined tickets file
- Triage state 记录为每个 issue file 顶部附近的 `Status:` 行（role 字符串见 `triage-labels.md`）
- Comments 和 conversation history 追加到文件底部的 `## Comments` heading 下

## When a skill says "publish to the issue tracker"

在 `.scratch/<feature-slug>/` 下创建新文件（必要时创建目录）。

## When a skill says "fetch the relevant ticket"

读取引用路径处的文件。用户通常会直接传入路径或 issue number。

## Wayfinding operations

- Map 保存为 `.scratch/<feature-slug>/map.md`，并使用 `Type: wayfinder:map`
- Child tickets 保存到 `.scratch/<feature-slug>/issues/<NN>-<slug>.md`
- Ticket 顶部使用 `Status`、`Type`、`Parent`、`Blocked by` 和 `Assignee` 字段
- `Type` 使用 `wayfinder:research`、`wayfinder:prototype`、`wayfinder:grilling` 或 `wayfinder:task`
- Local Markdown 没有原生依赖关系；`Blocked by` 填相对 ticket 路径，多个路径以逗号分隔，无依赖时写 `none`
- Claim ticket 时填写 `Assignee`；未领取时写 `unassigned`
- Frontier 是所有 `Status: open`、`Assignee: unassigned`，且 `Blocked by` 中每个 ticket 均已关闭的 child tickets
- Resolution 追加到 `## Comments`，随后将 `Status` 改为 `closed`，并在 map 的 `Decisions so far` 中添加一行链接与结论摘要
