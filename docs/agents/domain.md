# Domain Docs

Engineering skills 探索 codebase 时，应如何消费这个 repo 的 domain documentation。

## Before exploring, read these

- repo 根目录的 **`CONTEXT.md`**，或
- repo 根目录的 **`CONTEXT-MAP.md`**（如果存在）— 它指向每个 context 的一个 `CONTEXT.md`。读取与当前话题相关的每个文件。
- **`docs/adr/`** — 读取与你即将处理区域相关的 ADRs。在 multi-context repos 中，也检查 `src/<context>/docs/adr/` 中的 context-scoped decisions。

如果这些文件不存在，**静默继续**。不要标记缺失；不要提前建议创建。producer skill（`/grill-with-docs`）会在 terms 或 decisions 实际被解决时懒创建它们。

## File structure

本仓库使用 single-context：

```text
/
├── CONTEXT.md
├── docs/adr/
└── src/
```

## Use the glossary's vocabulary

当你的输出命名某个 domain concept 时（issue title、refactor proposal、hypothesis、test name），使用 `CONTEXT.md` 中定义的 term。不要漂移到 glossary 明确避免的 synonyms。

如果需要的概念还不在 glossary 中，要么重新考虑是否正在发明项目没有使用的语言，要么为 `/grill-with-docs` 记录这个缺口。

## Flag ADR conflicts

如果输出与现有 ADR 矛盾，明确指出，而不是静默覆盖。
